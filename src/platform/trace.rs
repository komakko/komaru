use std::sync::OnceLock;

use crate::core::ingest::Payload;
use crate::identity;

const PREVIEW_CHARS: usize = 160;

pub fn drop_event(describe: impl FnOnce() -> String) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os(identity::TRACE_DROP_VARIABLE).is_some()) {
        eprintln!("drop: {}", describe());
    }
}

pub fn summarise(payloads: &[Payload]) -> String {
    let preview = |text: &str| text.chars().take(PREVIEW_CHARS).collect::<String>();
    let items: Vec<String> = payloads
        .iter()
        .map(|payload| match payload {
            Payload::Files(paths) => format!("files {paths:?}"),
            Payload::Pixels(pixels) => format!("pixels {}x{}", pixels.width, pixels.height),
            Payload::Bytes(bytes) => format!("bytes {}", bytes.len()),
            Payload::Html(fragment) => format!(
                "html {:?} base {:?}",
                preview(&fragment.markup),
                fragment.base_url
            ),
            Payload::Text(text) => format!("text {:?}", preview(text)),
        })
        .collect();
    items.join(" | ")
}
