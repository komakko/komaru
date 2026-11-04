use std::collections::VecDeque;

use crate::core::ingest::{Payload, Reference, inspect};
use crate::error::IngestError;

pub struct Candidates {
    remaining: VecDeque<Reference>,
    failure: Option<IngestError>,
}

impl Candidates {
    pub fn from_payloads(payloads: Vec<Payload>) -> Self {
        let mut candidates = Self {
            remaining: VecDeque::new(),
            failure: None,
        };
        for payload in payloads {
            match inspect::reference_from(payload) {
                Ok(Some(reference)) => candidates.push(reference),
                Ok(None) => {}
                Err(error) => candidates.record_failure(error),
            }
        }
        candidates
    }

    pub fn take_next(&mut self) -> Option<Reference> {
        self.remaining.pop_front()
    }

    pub fn record_failure(&mut self, error: IngestError) {
        self.failure.get_or_insert(error);
    }

    pub fn into_failure(self) -> IngestError {
        self.failure.unwrap_or(IngestError::NothingRecognised)
    }

    fn push(&mut self, reference: Reference) {
        let known = self
            .remaining
            .iter()
            .any(|existing| existing.same_target(&reference));
        if !known {
            self.remaining.push_back(reference);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ingest::HtmlFragment;
    use std::path::{Path, PathBuf};

    fn text(value: &str) -> Payload {
        Payload::Text(value.to_owned())
    }

    fn url_of(reference: Option<Reference>) -> Option<String> {
        match reference {
            Some(Reference::Url(url)) => Some(url),
            _ => None,
        }
    }

    #[test]
    fn keeps_every_representation_in_order() {
        let mut candidates = Candidates::from_payloads(vec![
            Payload::Files(vec![PathBuf::from("/a.png"), PathBuf::from("/b.png")]),
            Payload::Html(HtmlFragment::without_base(
                r#"<img src="https://a.test/1.png">"#.to_owned(),
            )),
            text("https://a.test/2.png"),
        ]);
        assert!(matches!(
            candidates.take_next(),
            Some(Reference::Path(path)) if path.as_path() == Path::new("/a.png")
        ));
        assert_eq!(
            url_of(candidates.take_next()).as_deref(),
            Some("https://a.test/1.png")
        );
        assert_eq!(
            url_of(candidates.take_next()).as_deref(),
            Some("https://a.test/2.png")
        );
        assert!(candidates.take_next().is_none());
    }

    #[test]
    fn repeated_targets_are_tried_once() {
        let mut candidates = Candidates::from_payloads(vec![
            Payload::Html(HtmlFragment::without_base(
                r#"<img src="https://a.test/1.png">"#.to_owned(),
            )),
            text("https://a.test/1.png\nhttps://a.test/other.png"),
            text("https://a.test/1.png"),
        ]);
        assert!(candidates.take_next().is_some());
        assert!(candidates.take_next().is_none());
    }

    #[test]
    fn unusable_payloads_are_skipped() {
        let mut candidates = Candidates::from_payloads(vec![
            Payload::Html(HtmlFragment::without_base("<p>no image</p>".to_owned())),
            Payload::Bytes(b"not an image".to_vec()),
            text("https://a.test/1.png"),
        ]);
        assert_eq!(
            url_of(candidates.take_next()).as_deref(),
            Some("https://a.test/1.png")
        );
    }

    #[test]
    fn a_broken_payload_does_not_hide_later_ones() {
        let mut candidates = Candidates::from_payloads(vec![
            text("data:image/png;base64,@@@@"),
            text("https://a.test/1.png"),
        ]);
        assert!(candidates.take_next().is_some());
        assert!(matches!(
            candidates.into_failure(),
            IngestError::MalformedDataUri
        ));
    }

    #[test]
    fn the_first_failure_is_the_one_reported() {
        let mut candidates = Candidates::from_payloads(vec![text("https://a.test/1.png")]);
        candidates.record_failure(IngestError::MalformedDataUri);
        candidates.record_failure(IngestError::NothingRecognised);
        assert!(matches!(
            candidates.into_failure(),
            IngestError::MalformedDataUri
        ));
    }

    #[test]
    fn nothing_usable_means_nothing_recognised() {
        let candidates = Candidates::from_payloads(vec![text("just words")]);
        assert!(matches!(
            candidates.into_failure(),
            IngestError::NothingRecognised
        ));
        let empty = Candidates::from_payloads(Vec::new());
        assert!(matches!(
            empty.into_failure(),
            IngestError::NothingRecognised
        ));
    }
}
