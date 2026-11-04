use std::cell::Cell;
use std::ffi::{OsString, c_void};
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;

use windows::Win32::Foundation::{HGLOBAL, HWND, POINTL, S_OK};
use windows::Win32::System::Com::{
    DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IDataObject, IStream, STGMEDIUM, TYMED_HGLOBAL,
    TYMED_ISTREAM,
};
use windows::Win32::System::DataExchange::GetClipboardFormatNameW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_DIB, CF_DIBV5, CF_HDROP, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK,
    DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium,
    RevokeDragDrop,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows_core::{Ref, implement};

use crate::core::ingest::{Payload, looks_like_image};
use crate::core::limits;
use crate::error::PlatformError;
use crate::platform::trace::{drop_event, summarise};
use crate::platform::win_formats::{bmp_from_dib, html_fragment, utf16_text};
use crate::platform::{DropEvent, DropSink};

const FIRST_REGISTERED_FORMAT: u16 = 0xC000;
const FORMAT_NAME_CAPACITY: usize = 128;
const WHOLE_OBJECT: i32 = -1;
const FIRST_FILE: i32 = 0;
const IGNORE_FILE_CONTENTS: &str = "chromium/x-ignore-file-contents";
const STREAM_CHUNK: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Files,
    RawImage,
    Dib,
    Html,
    Utf16,
}

struct Candidate {
    id: u16,
    index: i32,
    source: Source,
    rank: u8,
    preference: u8,
}

#[implement(IDropTarget)]
struct DropTarget {
    sink: DropSink,
    accepts: Cell<bool>,
}

pub fn install(window: isize, sink: DropSink) -> Result<(), PlatformError> {
    let window = HWND(window as *mut c_void);
    let target: IDropTarget = DropTarget {
        sink,
        accepts: Cell::new(false),
    }
    .into();
    unsafe {
        let _ = RevokeDragDrop(window);
        RegisterDragDrop(window, &target)
    }
    .map_err(win32_error)
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let accepts = data
            .as_ref()
            .is_some_and(|data| !candidates(data).is_empty());
        drop_event(|| {
            let ids = data.as_ref().map(offered_ids).unwrap_or_default();
            let names: Vec<String> = ids
                .iter()
                .map(|&id| registered_format_name(id).unwrap_or_else(|| format!("#{id}")))
                .collect();
            format!("windows offered {names:?}, accepted {accepts}")
        });
        self.accepts.set(accepts);
        (self.sink)(DropEvent::Hover(accepts));
        self.answer(effect);
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        self.answer(effect);
        Ok(())
    }

    fn DragLeave(&self) -> windows_core::Result<()> {
        self.accepts.set(false);
        (self.sink)(DropEvent::Hover(false));
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        self.answer(effect);
        (self.sink)(DropEvent::Hover(false));
        if self.accepts.replace(false)
            && let Some(data) = data.as_ref()
        {
            let collected = collect(data);
            drop_event(|| match &collected {
                Ok(payloads) => format!("windows received {}", summarise(payloads)),
                Err(error) => format!("windows failed: {error}"),
            });
            (self.sink)(DropEvent::Delivered(collected));
        }
        Ok(())
    }
}

impl DropTarget_Impl {
    fn answer(&self, effect: *mut DROPEFFECT) {
        let allowed = unsafe { *effect };
        let chosen = if !self.accepts.get() {
            DROPEFFECT_NONE
        } else if allowed.0 & DROPEFFECT_COPY.0 != 0 {
            DROPEFFECT_COPY
        } else if allowed.0 & DROPEFFECT_LINK.0 != 0 {
            DROPEFFECT_LINK
        } else {
            DROPEFFECT_NONE
        };
        unsafe { *effect = chosen };
    }
}

fn collect(data: &IDataObject) -> Result<Vec<Payload>, PlatformError> {
    let mut payloads = Vec::new();
    let mut failure = None;
    let mut have_image = false;
    for candidate in candidates(data) {
        let is_image = matches!(candidate.source, Source::RawImage | Source::Dib);
        if is_image && have_image {
            continue;
        }
        match payload_from(data, &candidate) {
            Ok(Some(payload)) => {
                have_image |= is_image;
                payloads.push(payload);
                if candidate.source == Source::Files {
                    break;
                }
            }
            Ok(None) => {}
            Err(error) => failure = Some(error),
        }
    }
    match (payloads.is_empty(), failure) {
        (true, Some(error)) => Err(error),
        _ => Ok(payloads),
    }
}

fn payload_from(
    data: &IDataObject,
    candidate: &Candidate,
) -> Result<Option<Payload>, PlatformError> {
    let (id, index) = (candidate.id, candidate.index);
    let payload = match candidate.source {
        Source::Files => {
            let paths = with_medium(data, id, index, |medium| unsafe {
                read_paths(medium.u.hGlobal)
            })?;
            (!paths.is_empty()).then_some(Payload::Files(paths))
        }
        Source::RawImage => {
            let bytes = fetch(data, id, index)?;
            looks_like_image(&bytes).then_some(Payload::Bytes(bytes))
        }
        Source::Dib => bmp_from_dib(&fetch(data, id, index)?).map(Payload::Bytes),
        Source::Html => html_fragment(&fetch(data, id, index)?).map(Payload::Html),
        Source::Utf16 => {
            let text = utf16_text(&fetch(data, id, index)?);
            (!text.trim().is_empty()).then_some(Payload::Text(text))
        }
    };
    Ok(payload)
}

fn candidates(data: &IDataObject) -> Vec<Candidate> {
    let ids = offered_ids(data);
    let file_contents_unreliable = ids
        .iter()
        .any(|&id| registered_format_name(id).is_some_and(|name| name == IGNORE_FILE_CONTENTS));
    let mut found: Vec<Candidate> = ids
        .into_iter()
        .filter_map(classify)
        .filter(|candidate| !(file_contents_unreliable && candidate.index == FIRST_FILE))
        .collect();
    found.sort_by_key(|candidate| (candidate.rank, candidate.preference));
    found
}

fn offered_ids(data: &IDataObject) -> Vec<u16> {
    let Ok(formats) = (unsafe { data.EnumFormatEtc(DATADIR_GET.0 as u32) }) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    let mut slot = [FORMATETC::default()];
    while unsafe { formats.Next(&mut slot, None) } == S_OK {
        ids.push(slot[0].cfFormat);
    }
    ids
}

fn classify(id: u16) -> Option<Candidate> {
    let candidate = |source, rank, preference| Candidate {
        id,
        index: WHOLE_OBJECT,
        source,
        rank,
        preference,
    };
    match id {
        id if id == CF_HDROP.0 => Some(candidate(Source::Files, 0, 0)),
        id if id == CF_DIBV5.0 => Some(candidate(Source::Dib, 1, 8)),
        id if id == CF_DIB.0 => Some(candidate(Source::Dib, 1, 9)),
        id if id == CF_UNICODETEXT.0 => Some(candidate(Source::Utf16, 4, 0)),
        _ => {
            let name = registered_format_name(id)?.to_ascii_lowercase();
            match name.as_str() {
                "filecontents" => Some(Candidate {
                    index: FIRST_FILE,
                    ..candidate(Source::RawImage, 1, 0)
                }),
                "png" | "image/png" => Some(candidate(Source::RawImage, 1, 1)),
                "jfif" | "image/jpeg" => Some(candidate(Source::RawImage, 1, 2)),
                "image/webp" => Some(candidate(Source::RawImage, 1, 3)),
                "gif" | "image/gif" => Some(candidate(Source::RawImage, 1, 4)),
                "image/avif" => Some(candidate(Source::RawImage, 1, 5)),
                "image/bmp" => Some(candidate(Source::RawImage, 1, 6)),
                "image/svg+xml" => None,
                other if other.starts_with("image/") => Some(candidate(Source::RawImage, 1, 7)),
                "html format" => Some(candidate(Source::Html, 2, 0)),
                "uniformresourcelocatorw" => Some(candidate(Source::Utf16, 3, 0)),
                "text/x-moz-url" => Some(candidate(Source::Utf16, 3, 1)),
                _ => None,
            }
        }
    }
}

fn registered_format_name(id: u16) -> Option<String> {
    if id < FIRST_REGISTERED_FORMAT {
        return None;
    }
    let mut buffer = [0u16; FORMAT_NAME_CAPACITY];
    let length = usize::try_from(unsafe { GetClipboardFormatNameW(u32::from(id), &mut buffer) })
        .ok()
        .filter(|&length| length > 0)?;
    Some(String::from_utf16_lossy(&buffer[..length]))
}

fn with_medium<T>(
    data: &IDataObject,
    id: u16,
    index: i32,
    read: impl FnOnce(&STGMEDIUM) -> T,
) -> Result<T, PlatformError> {
    let request = FORMATETC {
        cfFormat: id,
        ptd: ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: index,
        tymed: (TYMED_HGLOBAL.0 | TYMED_ISTREAM.0) as u32,
    };
    let mut medium = unsafe { data.GetData(&request) }.map_err(win32_error)?;
    let value = read(&medium);
    unsafe { ReleaseStgMedium(&mut medium) };
    Ok(value)
}

fn fetch(data: &IDataObject, id: u16, index: i32) -> Result<Vec<u8>, PlatformError> {
    with_medium(data, id, index, |medium| unsafe { read_medium(medium) })?
}

unsafe fn read_medium(medium: &STGMEDIUM) -> Result<Vec<u8>, PlatformError> {
    match medium.tymed {
        tymed if tymed == TYMED_HGLOBAL.0 as u32 => unsafe { read_global(medium.u.hGlobal) },
        tymed if tymed == TYMED_ISTREAM.0 as u32 => {
            let stream: &Option<IStream> = unsafe { &medium.u.pstm };
            stream.as_ref().map_or(Ok(Vec::new()), read_stream)
        }
        _ => Ok(Vec::new()),
    }
}

unsafe fn read_global(global: HGLOBAL) -> Result<Vec<u8>, PlatformError> {
    let size = unsafe { GlobalSize(global) };
    ensure_within_limit(size as u64)?;
    let source = unsafe { GlobalLock(global) };
    if source.is_null() {
        return Ok(Vec::new());
    }
    let bytes = unsafe { std::slice::from_raw_parts(source.cast::<u8>(), size) }.to_vec();
    let _ = unsafe { GlobalUnlock(global) };
    Ok(bytes)
}

fn read_stream(stream: &IStream) -> Result<Vec<u8>, PlatformError> {
    let mut data = Vec::new();
    let mut chunk = vec![0u8; STREAM_CHUNK];
    loop {
        let mut read = 0u32;
        unsafe {
            stream.Read(
                chunk.as_mut_ptr().cast(),
                STREAM_CHUNK as u32,
                Some(&mut read),
            )
        }
        .ok()
        .map_err(win32_error)?;
        if read == 0 {
            return Ok(data);
        }
        ensure_within_limit((data.len() + read as usize) as u64)?;
        data.extend_from_slice(&chunk[..read as usize]);
    }
}

unsafe fn read_paths(global: HGLOBAL) -> Vec<PathBuf> {
    let drop = HDROP(global.0);
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    (0..count)
        .map(|index| {
            let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
            let mut buffer = vec![0u16; length + 1];
            let copied = unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) } as usize;
            PathBuf::from(OsString::from_wide(&buffer[..copied]))
        })
        .collect()
}

fn ensure_within_limit(size: u64) -> Result<(), PlatformError> {
    if size > limits::encoded() {
        return Err(PlatformError::DragDataTooLarge {
            limit: limits::encoded(),
        });
    }
    Ok(())
}

fn win32_error(error: windows_core::Error) -> PlatformError {
    PlatformError::Win32(io::Error::from(error))
}
