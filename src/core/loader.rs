use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::core::animation::{Animation, decode_frames};
use crate::core::decode::{DecodedImage, decode};
use crate::core::encode::Format;
use crate::core::info::ImageInfo;
use crate::core::ingest::{self, Acquired, Candidates, Origin, Payload, Reference};
use crate::core::limits;
use crate::core::mips::build_levels_below;
use crate::error::{DecodeError, FetchError, IngestError};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

pub type SharedRgba = Arc<dyn AsRef<[u8]> + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadId {
    ticket: u64,
    revision: u32,
}

impl LoadId {
    pub fn is_same_load(self, other: Self) -> bool {
        self.ticket == other.ticket
    }

    pub fn revised(self) -> Self {
        Self {
            revision: self.revision + 1,
            ..self
        }
    }
}

pub struct Loaded {
    pub id: LoadId,
    pub origin: Origin,
    pub file_size: Option<u64>,
    pub lossy_format: Option<Format>,
    pub info: ImageInfo,
    pub animation_source: Option<Arc<[u8]>>,
    pub base: DecodedImage,
}

pub struct Animated {
    pub load: LoadId,
    pub outcome: Result<Animation, DecodeError>,
}

pub struct Mips {
    pub load: LoadId,
    pub levels: Vec<DecodedImage>,
}

pub type LoadResult = Result<Loaded, IngestError>;

pub enum Activity {
    ReadingClipboard,
    ReceivingDrop,
    ReadingFile {
        title: String,
    },
    Downloading {
        title: String,
        received: u64,
        total: Option<u64>,
    },
    Decoding {
        title: String,
    },
}

impl Activity {
    pub fn is_cancellable(&self) -> bool {
        matches!(
            self,
            Self::ReadingFile { .. } | Self::Downloading { .. } | Self::Decoding { .. }
        )
    }
}

pub enum LoaderEvent {
    Busy(Activity),
    Finished(LoadResult),
    Mips(Mips),
    Animation(Animated),
    Idle,
}

enum Work {
    Ingest(Vec<Payload>),
    Fetched(Origin, Result<Vec<u8>, FetchError>, Candidates),
    ReceivingDrop,
    Fail(IngestError),
    Mips(MipRequest),
    Frames(FrameRequest),
    Cancel,
}

struct MipRequest {
    load: LoadId,
    size: [usize; 2],
    pixels: SharedRgba,
}

struct FrameRequest {
    load: LoadId,
    bytes: Arc<[u8]>,
}

struct Message {
    ticket: u64,
    work: Work,
}

#[derive(Clone, Default)]
struct Tickets(Arc<AtomicU64>);

impl Tickets {
    fn issue(&self) -> u64 {
        self.0.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn latest(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    fn is_current(&self, ticket: u64) -> bool {
        self.latest() == ticket
    }
}

#[derive(Clone)]
struct Notifier {
    events: Sender<LoaderEvent>,
    wake_ui: Arc<dyn Fn() + Send + Sync>,
}

impl Notifier {
    fn send(&self, event: LoaderEvent) -> bool {
        let delivered = self.events.send(event).is_ok();
        if delivered {
            (self.wake_ui)();
        }
        delivered
    }

    fn busy(&self, activity: Activity) {
        self.send(LoaderEvent::Busy(activity));
    }
}

#[derive(Clone)]
pub struct LoadRequester {
    sender: Sender<Message>,
    tickets: Tickets,
}

impl LoadRequester {
    pub fn ingest(&self, payloads: Vec<Payload>) {
        self.send(self.tickets.issue(), Work::Ingest(payloads));
    }

    pub fn receiving_drop(&self) {
        self.send(self.tickets.issue(), Work::ReceivingDrop);
    }

    pub fn fail(&self, error: IngestError) {
        self.send(self.tickets.issue(), Work::Fail(error));
    }

    pub fn cancel(&self) {
        self.send(self.tickets.issue(), Work::Cancel);
    }

    pub fn build_mips(&self, load: LoadId, size: [usize; 2], pixels: SharedRgba) {
        let request = MipRequest { load, size, pixels };
        self.send(load.ticket, Work::Mips(request));
    }

    pub fn decode_animation(&self, load: LoadId, bytes: Arc<[u8]>) {
        self.send(load.ticket, Work::Frames(FrameRequest { load, bytes }));
    }

    fn send(&self, ticket: u64, work: Work) {
        let _ = self.sender.send(Message { ticket, work });
    }
}

pub struct Loader {
    requester: LoadRequester,
    events: Receiver<LoaderEvent>,
}

impl Loader {
    pub fn spawn(wake_ui: impl Fn() + Send + Sync + 'static) -> Self {
        let (sender, inbox) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        let requester = LoadRequester {
            sender,
            tickets: Tickets::default(),
        };
        let worker = Worker {
            inbox,
            notifier: Notifier {
                events: event_sender,
                wake_ui: Arc::new(wake_ui),
            },
            requester: requester.clone(),
        };
        thread::spawn(move || worker.run());
        Self { requester, events }
    }

    pub fn requester(&self) -> LoadRequester {
        self.requester.clone()
    }

    pub fn try_next_event(&self) -> Option<LoaderEvent> {
        self.events.try_recv().ok()
    }
}

struct Worker {
    inbox: Receiver<Message>,
    notifier: Notifier,
    requester: LoadRequester,
}

impl Worker {
    fn run(self) {
        while let Ok(message) = self.inbox.recv() {
            if !self.requester.tickets.is_current(message.ticket) {
                continue;
            }
            let Some(event) = self.process(message) else {
                continue;
            };
            if !self.notifier.send(event) {
                break;
            }
        }
    }

    fn process(&self, message: Message) -> Option<LoaderEvent> {
        let ticket = message.ticket;
        let event = match message.work {
            Work::Ingest(payloads) => {
                let candidates = Candidates::from_payloads(payloads);
                LoaderEvent::Finished(self.advance(ticket, candidates)?)
            }
            Work::Fetched(origin, fetched, candidates) => {
                LoaderEvent::Finished(self.finish_fetch(ticket, origin, fetched, candidates)?)
            }
            Work::ReceivingDrop => {
                self.notifier.busy(Activity::ReceivingDrop);
                return None;
            }
            Work::Fail(error) => LoaderEvent::Finished(Err(error)),
            Work::Mips(request) => LoaderEvent::Mips(self.build_mips(request)?),
            Work::Frames(request) => LoaderEvent::Animation(self.decode_animation(request)?),
            Work::Cancel => LoaderEvent::Idle,
        };
        self.requester.tickets.is_current(ticket).then_some(event)
    }

    fn build_mips(&self, request: MipRequest) -> Option<Mips> {
        let tickets = &self.requester.tickets;
        let pixels: &[u8] = (*request.pixels).as_ref();
        let levels = build_levels_below(request.size, pixels, || {
            tickets.is_current(request.load.ticket)
        })?;
        (!levels.is_empty()).then_some(Mips {
            load: request.load,
            levels,
        })
    }

    fn decode_animation(&self, request: FrameRequest) -> Option<Animated> {
        let tickets = &self.requester.tickets;
        let outcome = decode_frames(&request.bytes, limits::decoded(), || {
            tickets.is_current(request.load.ticket)
        })
        .transpose()?;
        Some(Animated {
            load: request.load,
            outcome,
        })
    }

    fn loaded(&self, origin: Origin, decoded: Decoded) -> Loaded {
        Loaded {
            id: LoadId {
                ticket: self.requester.tickets.latest(),
                revision: 0,
            },
            origin,
            file_size: decoded.file_size,
            lossy_format: decoded.lossy_format,
            info: decoded.info,
            animation_source: decoded.animation_source,
            base: decoded.base,
        }
    }

    fn advance(&self, ticket: u64, mut candidates: Candidates) -> Option<LoadResult> {
        while let Some(reference) = candidates.take_next() {
            if !self.requester.tickets.is_current(ticket) {
                return None;
            }
            let loaded = match reference {
                Reference::Url(url) => {
                    self.start_fetch(ticket, url, candidates);
                    return None;
                }
                Reference::Path(path) => self.load_file(path),
                Reference::Inline(acquired) => self.show(Origin::Unnamed, acquired),
            };
            match loaded {
                Ok(loaded) => return Some(Ok(loaded)),
                Err(error) => candidates.record_failure(error),
            }
        }
        Some(Err(candidates.into_failure()))
    }

    fn finish_fetch(
        &self,
        ticket: u64,
        origin: Origin,
        fetched: Result<Vec<u8>, FetchError>,
        mut candidates: Candidates,
    ) -> Option<LoadResult> {
        let loaded = fetched
            .map_err(IngestError::from)
            .and_then(|bytes| self.show(origin, Acquired::Encoded(bytes)));
        match loaded {
            Ok(loaded) => Some(Ok(loaded)),
            Err(error) => {
                candidates.record_failure(error);
                self.advance(ticket, candidates)
            }
        }
    }

    fn load_file(&self, path: std::path::PathBuf) -> Result<Loaded, IngestError> {
        let origin = Origin::File(path.clone());
        self.notifier.busy(Activity::ReadingFile {
            title: origin.title(),
        });
        let acquired = ingest::read_file(&path)?;
        self.show(origin, acquired)
    }

    fn show(&self, origin: Origin, acquired: Acquired) -> Result<Loaded, IngestError> {
        self.notifier.busy(Activity::Decoding {
            title: origin.title(),
        });
        Ok(self.loaded(origin, decode_base(&acquired)?))
    }

    fn start_fetch(&self, ticket: u64, url: String, candidates: Candidates) {
        let origin = Origin::Url(url.clone());
        let title = origin.title();
        self.notifier.busy(Activity::Downloading {
            title: title.clone(),
            received: 0,
            total: None,
        });
        let requester = self.requester.clone();
        let notifier = self.notifier.clone();
        thread::spawn(move || {
            let mut last_report = Instant::now();
            let fetched = ingest::fetch(
                &url,
                || !requester.tickets.is_current(ticket),
                |received, total| {
                    if last_report.elapsed() >= PROGRESS_INTERVAL
                        && requester.tickets.is_current(ticket)
                    {
                        last_report = Instant::now();
                        notifier.busy(Activity::Downloading {
                            title: title.clone(),
                            received,
                            total,
                        });
                    }
                },
            );
            requester.send(ticket, Work::Fetched(origin, fetched, candidates));
        });
    }
}

struct Decoded {
    base: DecodedImage,
    info: ImageInfo,
    animation_source: Option<Arc<[u8]>>,
    file_size: Option<u64>,
    lossy_format: Option<Format>,
}

fn decode_base(acquired: &Acquired) -> Result<Decoded, DecodeError> {
    let (file_size, lossy_format) = match acquired {
        Acquired::Encoded(bytes) => (Some(bytes.len() as u64), lossy_format_of(bytes)),
        Acquired::Pixels(_) => (None, None),
    };
    decode(acquired).map(|decoded| Decoded {
        base: decoded.image,
        info: decoded.info,
        animation_source: match acquired {
            Acquired::Encoded(bytes) if decoded.animated => Some(Arc::from(bytes.as_slice())),
            _ => None,
        },
        file_size,
        lossy_format,
    })
}

fn lossy_format_of(bytes: &[u8]) -> Option<Format> {
    match image::guess_format(bytes).ok()? {
        image::ImageFormat::Jpeg => Some(Format::Jpeg),
        image::ImageFormat::WebP => Some(Format::WebP),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ingest::HtmlFragment;
    use image::{ImageFormat, RgbaImage};
    use std::io::{Cursor, Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        RgbaImage::new(4, 3)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn large_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        RgbaImage::new(300, 200)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn request_mips(loader: &Loader, loaded: &Loaded) {
        loader.requester().build_mips(
            loaded.id,
            [loaded.base.width, loaded.base.height],
            Arc::new(loaded.base.premultiplied_rgba.clone()),
        );
    }

    fn next_event(loader: &Loader) -> LoaderEvent {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(event) = loader.try_next_event() {
                return event;
            }
            assert!(Instant::now() < deadline, "no event arrived");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn next_mips(loader: &Loader) -> Mips {
        loop {
            if let LoaderEvent::Mips(mips) = next_event(loader) {
                return mips;
            }
        }
    }

    fn next_outcome(loader: &Loader) -> LoadResult {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(LoaderEvent::Finished(outcome)) = loader.try_next_event() {
                return outcome;
            }
            assert!(Instant::now() < deadline, "no outcome arrived");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn serve_png() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let body = png();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        });
        format!("http://{address}/no-extension")
    }

    fn serve_status(status: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let response =
                format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream.write_all(response.as_bytes()).unwrap();
        });
        format!("http://{address}/missing.png")
    }

    fn temp_png(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, png()).unwrap();
        path
    }

    #[test]
    fn loads_files_and_keeps_the_first_of_several() {
        let loader = Loader::spawn(|| {});
        let path = temp_png("komaru_loader_file.png");
        loader.requester().ingest(vec![Payload::Files(vec![
            path.clone(),
            PathBuf::from("/ignored"),
        ])]);
        let loaded = next_outcome(&loader).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(loaded.origin, Origin::File(path));
        assert_eq!(loaded.file_size, Some(png().len() as u64));
    }

    #[test]
    fn requested_lower_levels_arrive_for_the_same_load() {
        let loader = Loader::spawn(|| {});
        loader.requester().ingest(vec![Payload::Bytes(large_png())]);
        let loaded = next_outcome(&loader).unwrap();
        assert_eq!((loaded.base.width, loaded.base.height), (300, 200));
        request_mips(&loader, &loaded);
        let mips = next_mips(&loader);
        assert_eq!(mips.load, loaded.id);
        let sizes: Vec<_> = mips.levels.iter().map(|l| (l.width, l.height)).collect();
        assert_eq!(sizes, [(150, 100), (75, 50), (38, 25)]);
    }

    #[test]
    fn lower_levels_for_a_superseded_load_are_never_built() {
        let loader = Loader::spawn(|| {});
        let requester = loader.requester();
        requester.ingest(vec![Payload::Bytes(large_png())]);
        let stale = next_outcome(&loader).unwrap();
        requester.ingest(vec![Payload::Text("hello".to_owned())]);
        request_mips(&loader, &stale);
        assert!(next_outcome(&loader).is_err());
        requester.ingest(vec![Payload::Text("again".to_owned())]);
        loop {
            match next_event(&loader) {
                LoaderEvent::Finished(_) => break,
                LoaderEvent::Mips(_) => panic!("levels were built for a superseded load"),
                LoaderEvent::Busy(_) | LoaderEvent::Animation(_) | LoaderEvent::Idle => {}
            }
        }
    }

    fn gif() -> Vec<u8> {
        use image::codecs::gif::GifEncoder;
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            for shade in [0, 90] {
                let buffer = RgbaImage::from_pixel(4, 3, image::Rgba([shade, 0, 0, 255]));
                encoder.encode_frame(image::Frame::new(buffer)).unwrap();
            }
        }
        bytes
    }

    #[test]
    fn animated_files_offer_their_frames_after_the_first_one_shows() {
        let loader = Loader::spawn(|| {});
        let requester = loader.requester();
        requester.ingest(vec![Payload::Bytes(png())]);
        assert!(next_outcome(&loader).unwrap().animation_source.is_none());
        requester.ingest(vec![Payload::Bytes(gif())]);
        let loaded = next_outcome(&loader).unwrap();
        let source = loaded.animation_source.expect("a gif may be animated");
        requester.decode_animation(loaded.id, source);
        loop {
            if let LoaderEvent::Animation(animated) = next_event(&loader) {
                assert_eq!(animated.load, loaded.id);
                assert_eq!(animated.outcome.unwrap().frames.len(), 2);
                break;
            }
        }
    }

    #[test]
    fn cancelling_a_stuck_download_goes_idle_without_an_outcome() {
        let stuck = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = stuck.local_addr().unwrap();
        let loader = Loader::spawn(|| {});
        let requester = loader.requester();
        requester.ingest(vec![Payload::Text(format!("http://{address}/slow.png"))]);
        requester.cancel();
        loop {
            match next_event(&loader) {
                LoaderEvent::Idle => break,
                LoaderEvent::Finished(_) => panic!("a cancelled load finished"),
                LoaderEvent::Busy(_) | LoaderEvent::Mips(_) | LoaderEvent::Animation(_) => {}
            }
        }
    }

    #[test]
    fn only_slow_activities_can_be_cancelled() {
        assert!(
            Activity::ReadingFile {
                title: String::new()
            }
            .is_cancellable()
        );
        assert!(
            Activity::Decoding {
                title: String::new()
            }
            .is_cancellable()
        );
        assert!(
            Activity::Downloading {
                title: String::new(),
                received: 0,
                total: None
            }
            .is_cancellable()
        );
        assert!(!Activity::ReadingClipboard.is_cancellable());
        assert!(!Activity::ReceivingDrop.is_cancellable());
    }

    #[test]
    fn small_images_get_no_lower_levels() {
        let loader = Loader::spawn(|| {});
        loader.requester().ingest(vec![Payload::Bytes(png())]);
        let loaded = next_outcome(&loader).unwrap();
        request_mips(&loader, &loaded);
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            assert!(!matches!(
                loader.try_next_event(),
                Some(LoaderEvent::Mips(_))
            ));
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn loads_urls_by_content_not_extension() {
        let loader = Loader::spawn(|| {});
        let url = serve_png();
        loader.requester().ingest(vec![Payload::Text(format!(
            "{url}\nhttps://ignored.test/x.png"
        ))]);
        let outcome = next_outcome(&loader);
        let loaded = outcome.unwrap();
        assert_eq!(loaded.origin, Origin::Url(url));
        assert_eq!(loaded.base.width, 4);
    }

    #[test]
    fn newest_request_wins() {
        let loader = Loader::spawn(|| {});
        let requester = loader.requester();
        let path = temp_png("komaru_loader_latest.png");
        for _ in 0..20 {
            requester.ingest(vec![Payload::Text("hello".to_owned())]);
        }
        requester.ingest(vec![Payload::Files(vec![path.clone()])]);
        let outcome = next_outcome(&loader);
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            outcome,
            Ok(Loaded {
                origin: Origin::File(_),
                ..
            })
        ));
    }

    #[test]
    fn a_newer_request_abandons_a_stuck_fetch() {
        let stuck = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = stuck.local_addr().unwrap();
        let loader = Loader::spawn(|| {});
        let requester = loader.requester();
        requester.ingest(vec![Payload::Text(format!("http://{address}/slow.png"))]);
        let path = temp_png("komaru_loader_abandon.png");
        let started = Instant::now();
        requester.ingest(vec![Payload::Files(vec![path.clone()])]);
        let outcome = next_outcome(&loader);
        std::fs::remove_file(&path).unwrap();
        assert!(outcome.is_ok());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn reports_unrecognised_payloads_and_platform_failures() {
        let loader = Loader::spawn(|| {});
        loader
            .requester()
            .ingest(vec![Payload::Text("just words".to_owned())]);
        assert!(matches!(
            next_outcome(&loader),
            Err(IngestError::NothingRecognised)
        ));
        loader.requester().fail(IngestError::Platform(
            crate::error::PlatformError::Unsupported("nope"),
        ));
        assert!(matches!(
            next_outcome(&loader),
            Err(IngestError::Platform(_))
        ));
    }

    #[test]
    fn rejects_files_that_are_not_images() {
        let loader = Loader::spawn(|| {});
        let path = std::env::temp_dir().join("komaru_loader_text.png");
        std::fs::write(&path, b"plain text pretending to be a png").unwrap();
        loader
            .requester()
            .ingest(vec![Payload::Files(vec![path.clone()])]);
        let outcome = next_outcome(&loader);
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            outcome,
            Err(IngestError::Decode(DecodeError::UnknownFormat))
        ));
    }

    #[test]
    fn falls_back_to_the_next_representation_when_a_link_fails() {
        let loader = Loader::spawn(|| {});
        let forbidden = serve_status("403 Forbidden");
        let working = serve_png();
        loader.requester().ingest(vec![
            Payload::Html(HtmlFragment::without_base(format!(
                r#"<img src="{forbidden}">"#
            ))),
            Payload::Files(vec![PathBuf::from("/definitely/not/here.png")]),
            Payload::Text(working.clone()),
        ]);
        let loaded = next_outcome(&loader).unwrap();
        assert_eq!(loaded.origin, Origin::Url(working));
    }

    #[test]
    fn reports_the_first_failure_when_every_candidate_fails() {
        let loader = Loader::spawn(|| {});
        let missing = serve_status("404 Not Found");
        let forbidden = serve_status("403 Forbidden");
        loader
            .requester()
            .ingest(vec![Payload::Text(missing), Payload::Text(forbidden)]);
        let error = next_outcome(&loader).err().unwrap();
        assert!(matches!(error, IngestError::Fetch(_)));
        assert!(error.to_string().contains("404"));
    }

    #[test]
    fn a_working_first_candidate_is_never_followed_by_the_rest() {
        let loader = Loader::spawn(|| {});
        let working = serve_png();
        let unreachable = serve_status("404 Not Found");
        loader.requester().ingest(vec![
            Payload::Text(working.clone()),
            Payload::Text(unreachable),
        ]);
        let loaded = next_outcome(&loader).unwrap();
        assert_eq!(loaded.origin, Origin::Url(working));
    }

    #[test]
    fn announces_a_receiving_drop_until_the_payloads_arrive() {
        let loader = Loader::spawn(|| {});
        loader.requester().receiving_drop();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(LoaderEvent::Busy(Activity::ReceivingDrop)) = loader.try_next_event() {
                break;
            }
            assert!(Instant::now() < deadline, "no activity arrived");
            thread::sleep(Duration::from_millis(5));
        }
    }
}
