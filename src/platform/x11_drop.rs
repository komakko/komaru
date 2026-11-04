use std::thread;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt, CreateWindowAux, EventMask, PropMode,
    Property, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, NONE};

use crate::core::ingest::{HtmlFragment, Payload};
use crate::core::limits;
use crate::error::PlatformError;
use crate::platform::trace::{drop_event, summarise};
use crate::platform::{DropEvent, DropSink};

const PROTOCOL_VERSION: u32 = 5;
const TYPE_LIST_WORDS: u32 = 1024;
const STALL_TIMEOUT: Duration = Duration::from_secs(3);
const ACCEPTED: u32 = 1;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        XdndAware,
        XdndProxy,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndSelection,
        XdndTypeList,
        XdndActionCopy,
        INCR,
        KOMARU_DROP,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    UriList,
    Image,
    Html,
    MozUrl,
    Text,
}

struct Offer {
    atom: Atom,
    kind: Kind,
    preference: u8,
}

struct Session {
    source: Window,
    target: Window,
    version: u32,
    offers: Vec<Offer>,
}

struct DropTarget {
    connection: RustConnection,
    atoms: Atoms,
    window: Window,
    sink: DropSink,
}

pub fn install(target_window: Window, sink: DropSink) -> Result<(), PlatformError> {
    let target = DropTarget::connect(target_window, sink)?;
    thread::spawn(move || target.run());
    Ok(())
}

impl DropTarget {
    fn connect(target_window: Window, sink: DropSink) -> Result<Self, PlatformError> {
        let (connection, screen_number) = RustConnection::connect(None)?;
        let root = connection.setup().roots[screen_number].root;
        let atoms = Atoms::new(&connection)?.reply()?;
        let window = connection.generate_id()?;
        connection.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            COPY_FROM_PARENT,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?;
        connection.change_property32(
            PropMode::REPLACE,
            window,
            atoms.XdndAware,
            AtomEnum::ATOM,
            &[PROTOCOL_VERSION],
        )?;
        connection.change_property32(
            PropMode::REPLACE,
            window,
            atoms.XdndProxy,
            AtomEnum::WINDOW,
            &[window],
        )?;
        connection
            .change_property32(
                PropMode::REPLACE,
                target_window,
                atoms.XdndProxy,
                AtomEnum::WINDOW,
                &[window],
            )?
            .check()?;
        connection.flush()?;
        Ok(Self {
            connection,
            atoms,
            window,
            sink,
        })
    }

    fn run(self) {
        let mut session = None;
        let mut hovering = false;
        while let Ok(event) = self.connection.wait_for_event() {
            if let Event::ClientMessage(message) = event
                && message.format == 32
            {
                session = self.handle(&message, session);
                let accepting = session
                    .as_ref()
                    .is_some_and(|session| !session.offers.is_empty());
                if accepting != hovering {
                    hovering = accepting;
                    (self.sink)(DropEvent::Hover(accepting));
                }
            }
        }
    }

    fn handle(&self, message: &ClientMessageEvent, session: Option<Session>) -> Option<Session> {
        let data = message.data.as_data32();
        let kind = message.type_;
        if kind == self.atoms.XdndEnter {
            return self.begin(message);
        }
        let session = session.filter(|session| session.source == data[0])?;
        if kind == self.atoms.XdndPosition {
            self.reply_status(&session);
            Some(session)
        } else if kind == self.atoms.XdndDrop {
            self.finish(&session, data[2]);
            None
        } else if kind == self.atoms.XdndLeave {
            None
        } else {
            Some(session)
        }
    }

    fn begin(&self, message: &ClientMessageEvent) -> Option<Session> {
        let data = message.data.as_data32();
        let types = if data[1] & 1 != 0 {
            self.type_list(data[0])?
        } else {
            data[2..5]
                .iter()
                .copied()
                .filter(|&atom| atom != NONE)
                .collect()
        };
        Some(Session {
            source: data[0],
            target: message.window,
            version: data[1] >> 24,
            offers: self.offers(&types)?,
        })
    }

    fn type_list(&self, source: Window) -> Option<Vec<Atom>> {
        let reply = self
            .connection
            .get_property(
                false,
                source,
                self.atoms.XdndTypeList,
                AtomEnum::ATOM,
                0,
                TYPE_LIST_WORDS,
            )
            .ok()?
            .reply()
            .ok()?;
        Some(reply.value32()?.collect())
    }

    fn offers(&self, types: &[Atom]) -> Option<Vec<Offer>> {
        let cookies = types
            .iter()
            .map(|&atom| self.connection.get_atom_name(atom))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let mut offers = Vec::new();
        let mut names = Vec::new();
        for (&atom, cookie) in types.iter().zip(cookies) {
            let name = String::from_utf8_lossy(&cookie.reply().ok()?.name).into_owned();
            names.push(name.clone());
            if let Some((kind, preference)) = classify(&name) {
                offers.push(Offer {
                    atom,
                    kind,
                    preference,
                });
            }
        }
        drop_event(|| format!("x11 offered {names:?}, usable {}", offers.len()));
        Some(offers)
    }

    fn reply_status(&self, session: &Session) {
        let accepted = !session.offers.is_empty();
        let (flags, action) = if accepted {
            (ACCEPTED, self.atoms.XdndActionCopy)
        } else {
            (0, NONE)
        };
        self.send(
            session.source,
            self.atoms.XdndStatus,
            [session.target, flags, 0, 0, action],
        );
    }

    fn finish(&self, session: &Session, timestamp: u32) {
        (self.sink)(DropEvent::Receiving);
        let result = self.collect(session, timestamp);
        drop_event(|| match &result {
            Ok(payloads) => format!("x11 received {}", summarise(payloads)),
            Err(error) => format!("x11 failed: {error}"),
        });
        if session.version >= 2 {
            let accepted = result.as_ref().is_ok_and(|payloads| !payloads.is_empty());
            let (flags, action) = if accepted {
                (ACCEPTED, self.atoms.XdndActionCopy)
            } else {
                (0, NONE)
            };
            self.send(
                session.source,
                self.atoms.XdndFinished,
                [session.target, flags, action, 0, 0],
            );
        }
        (self.sink)(DropEvent::Delivered(result));
    }

    fn send(&self, destination: Window, kind: Atom, data: [u32; 5]) {
        let event = ClientMessageEvent::new(32, destination, kind, data);
        let _ = self
            .connection
            .send_event(false, destination, EventMask::NO_EVENT, event);
        let _ = self.connection.flush();
    }

    fn collect(&self, session: &Session, timestamp: u32) -> Result<Vec<Payload>, PlatformError> {
        let mut ranked = Vec::new();
        let mut failure = None;
        for offer in best_per_kind(&session.offers) {
            match self.fetch(offer.atom, timestamp) {
                Ok(bytes) => ranked.extend(payload_from(offer.kind, bytes)),
                Err(error) => failure = Some(error),
            }
            if ranked.iter().any(|(rank, _)| *rank == FILE_RANK) {
                break;
            }
        }
        ranked.sort_by_key(|(rank, _)| *rank);
        match (ranked.is_empty(), failure) {
            (true, Some(error)) => Err(error),
            _ => Ok(ranked.into_iter().map(|(_, payload)| payload).collect()),
        }
    }

    fn fetch(&self, target: Atom, timestamp: u32) -> Result<Vec<u8>, PlatformError> {
        self.connection.convert_selection(
            self.window,
            self.atoms.XdndSelection,
            target,
            self.atoms.KOMARU_DROP,
            timestamp,
        )?;
        self.connection.flush()?;
        let property = self.wait_for(|event| match event {
            Event::SelectionNotify(notify)
                if notify.requestor == self.window && notify.target == target =>
            {
                Some(notify.property)
            }
            _ => None,
        })?;
        if property == NONE {
            return Ok(Vec::new());
        }
        self.read_transfer()
    }

    fn read_transfer(&self) -> Result<Vec<u8>, PlatformError> {
        let header = self.peek_transfer()?;
        if header.type_ == self.atoms.INCR {
            self.connection
                .delete_property(self.window, self.atoms.KOMARU_DROP)?;
            self.connection.flush()?;
            return self.read_incremental();
        }
        ensure_within_limit(u64::from(header.bytes_after))?;
        self.take_transfer(header.bytes_after)
    }

    fn read_incremental(&self) -> Result<Vec<u8>, PlatformError> {
        let mut data = Vec::new();
        loop {
            self.wait_for(|event| match event {
                Event::PropertyNotify(notify)
                    if notify.window == self.window
                        && notify.atom == self.atoms.KOMARU_DROP
                        && notify.state == Property::NEW_VALUE =>
                {
                    Some(())
                }
                _ => None,
            })?;
            let header = self.peek_transfer()?;
            let chunk = self.take_transfer(header.bytes_after)?;
            if chunk.is_empty() {
                return Ok(data);
            }
            ensure_within_limit((data.len() + chunk.len()) as u64)?;
            data.extend_from_slice(&chunk);
        }
    }

    fn peek_transfer(&self) -> Result<x11rb::protocol::xproto::GetPropertyReply, PlatformError> {
        Ok(self
            .connection
            .get_property(
                false,
                self.window,
                self.atoms.KOMARU_DROP,
                AtomEnum::ANY,
                0,
                0,
            )?
            .reply()?)
    }

    fn take_transfer(&self, bytes: u32) -> Result<Vec<u8>, PlatformError> {
        Ok(self
            .connection
            .get_property(
                true,
                self.window,
                self.atoms.KOMARU_DROP,
                AtomEnum::ANY,
                0,
                bytes.div_ceil(4),
            )?
            .reply()?
            .value)
    }

    fn wait_for<T>(&self, mut pick: impl FnMut(Event) -> Option<T>) -> Result<T, PlatformError> {
        let deadline = Instant::now() + STALL_TIMEOUT;
        loop {
            while let Some(event) = self.connection.poll_for_event()? {
                if let Some(found) = pick(event) {
                    return Ok(found);
                }
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(PlatformError::DragSourceStalled)?;
            self.wait_readable(remaining)?;
        }
    }

    fn wait_readable(&self, timeout: Duration) -> Result<(), PlatformError> {
        let timeout = Timespec::try_from(timeout).map_err(|_| PlatformError::DragSourceStalled)?;
        let mut fds = [PollFd::new(self.connection.stream(), PollFlags::IN)];
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(_) | Err(Errno::INTR) => Ok(()),
            Err(_) => Err(PlatformError::DragSourceStalled),
        }
    }
}

const FILE_RANK: u8 = 0;
const IMAGE_RANK: u8 = 1;
const HTML_RANK: u8 = 2;
const LINK_RANK: u8 = 3;
const MOZ_URL_RANK: u8 = 4;
const TEXT_RANK: u8 = 5;

fn ensure_within_limit(size: u64) -> Result<(), PlatformError> {
    if size > limits::encoded() {
        return Err(PlatformError::DragDataTooLarge {
            limit: limits::encoded(),
        });
    }
    Ok(())
}

fn classify(name: &str) -> Option<(Kind, u8)> {
    let name = name.to_ascii_lowercase();
    match name.as_str() {
        "text/uri-list" => Some((Kind::UriList, 0)),
        "text/html" | "text/html;charset=utf-8" => Some((Kind::Html, 0)),
        "text/x-moz-url" => Some((Kind::MozUrl, 0)),
        "text/plain;charset=utf-8" | "utf8_string" => Some((Kind::Text, 0)),
        "text/plain" | "string" => Some((Kind::Text, 1)),
        "image/png" => Some((Kind::Image, 0)),
        "image/jpeg" | "image/jpg" => Some((Kind::Image, 1)),
        "image/webp" => Some((Kind::Image, 2)),
        "image/gif" => Some((Kind::Image, 3)),
        "image/avif" => Some((Kind::Image, 4)),
        "image/bmp" => Some((Kind::Image, 5)),
        "image/svg+xml" => None,
        other if other.starts_with("image/") => Some((Kind::Image, 6)),
        _ => None,
    }
}

fn best_per_kind(offers: &[Offer]) -> Vec<&Offer> {
    let mut best: Vec<&Offer> = Vec::new();
    for offer in offers {
        match best.iter_mut().find(|chosen| chosen.kind == offer.kind) {
            Some(chosen) if offer.preference < chosen.preference => *chosen = offer,
            Some(_) => {}
            None => best.push(offer),
        }
    }
    best.sort_by_key(|offer| offer.kind);
    best
}

fn payload_from(kind: Kind, bytes: Vec<u8>) -> Option<(u8, Payload)> {
    if bytes.is_empty() {
        return None;
    }
    match kind {
        Kind::Image => Some((IMAGE_RANK, Payload::Bytes(bytes))),
        Kind::Html => Some((
            HTML_RANK,
            Payload::Html(HtmlFragment::without_base(decode_text(&bytes))),
        )),
        Kind::MozUrl => Some((MOZ_URL_RANK, Payload::Text(decode_text(&bytes)))),
        Kind::Text => Some((TEXT_RANK, Payload::Text(decode_text(&bytes)))),
        Kind::UriList => {
            let lines = uri_list_lines(&decode_text(&bytes))?;
            let rank = if has_file_scheme(&lines) {
                FILE_RANK
            } else {
                LINK_RANK
            };
            Some((rank, Payload::Text(lines)))
        }
    }
}

fn has_file_scheme(text: &str) -> bool {
    text.get(.."file://".len())
        .is_some_and(|start| start.eq_ignore_ascii_case("file://"))
}

fn uri_list_lines(text: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn decode_text(bytes: &[u8]) -> String {
    let text = match bytes {
        [0xFF, 0xFE, rest @ ..] => decode_utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => decode_utf16(rest, u16::from_be_bytes),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [first, 0, ..] if *first != 0 => decode_utf16(bytes, u16::from_le_bytes),
        [0, second, ..] if *second != 0 => decode_utf16(bytes, u16::from_be_bytes),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    };
    text.trim_end_matches('\0').to_owned()
}

fn decode_utf16(bytes: &[u8], read: fn([u8; 2]) -> u16) -> String {
    let units = bytes.chunks_exact(2).map(|pair| read([pair[0], pair[1]]));
    char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn offer(kind: Kind, preference: u8) -> Offer {
        Offer {
            atom: 0,
            kind,
            preference,
        }
    }

    #[test]
    fn classifies_known_formats_case_insensitively() {
        assert_eq!(classify("text/uri-list"), Some((Kind::UriList, 0)));
        assert_eq!(classify("TEXT/HTML"), Some((Kind::Html, 0)));
        assert_eq!(classify("UTF8_STRING"), Some((Kind::Text, 0)));
        assert_eq!(classify("image/png"), Some((Kind::Image, 0)));
        assert_eq!(classify("image/x-icon"), Some((Kind::Image, 6)));
        assert_eq!(classify("image/svg+xml"), None);
        assert_eq!(classify("application/octet-stream"), None);
        assert_eq!(classify("TARGETS"), None);
    }

    #[test]
    fn keeps_the_most_preferred_offer_of_each_kind_in_kind_order() {
        let offers = vec![
            offer(Kind::Text, 1),
            offer(Kind::Image, 3),
            offer(Kind::Image, 0),
            offer(Kind::Text, 0),
            offer(Kind::UriList, 0),
        ];
        let best: Vec<_> = best_per_kind(&offers)
            .into_iter()
            .map(|chosen| (chosen.kind, chosen.preference))
            .collect();
        assert_eq!(
            best,
            vec![(Kind::UriList, 0), (Kind::Image, 0), (Kind::Text, 0)]
        );
    }

    #[test]
    fn decodes_utf8_and_utf16_text() {
        assert_eq!(decode_text(b"https://a.test/x.png"), "https://a.test/x.png");
        assert_eq!(decode_text(&utf16_le("<img src=\"a\">")), "<img src=\"a\">");
        let mut with_bom = vec![0xFF, 0xFE];
        with_bom.extend(utf16_le("héllo\0"));
        assert_eq!(decode_text(&with_bom), "héllo");
        let big_endian: Vec<u8> = "ab".encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(decode_text(&big_endian), "ab");
        assert_eq!(decode_text(b"\xEF\xBB\xBFabc"), "abc");
    }

    #[test]
    fn uri_lists_drop_comments_and_blank_lines() {
        assert_eq!(
            uri_list_lines("# note\r\n\r\nfile:///a.png\r\nfile:///b.png\r\n").as_deref(),
            Some("file:///a.png\nfile:///b.png")
        );
        assert_eq!(uri_list_lines("# only a comment\n"), None);
    }

    #[test]
    fn file_uri_lists_outrank_images_and_web_links_do_not() {
        let rank = |text: &str| {
            payload_from(Kind::UriList, text.as_bytes().to_vec()).map(|(rank, _)| rank)
        };
        assert_eq!(rank("file:///a.png"), Some(FILE_RANK));
        assert_eq!(rank("FILE:///a.png"), Some(FILE_RANK));
        assert_eq!(rank("https://a.test/a.png"), Some(LINK_RANK));
        assert_eq!(rank("# nothing"), None);
    }

    #[test]
    fn empty_transfers_produce_no_payload() {
        assert!(payload_from(Kind::Image, Vec::new()).is_none());
        assert!(payload_from(Kind::Html, Vec::new()).is_none());
    }
}
