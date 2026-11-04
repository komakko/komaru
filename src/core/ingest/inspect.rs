use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD_PAD_INDIFFERENT;
use percent_encoding::percent_decode_str;

use crate::core::ingest::{Acquired, Payload, Reference, html, relative_url};
use crate::core::ingest::{keep_first_discard_rest, looks_like_image};
use crate::core::limits;
use crate::error::IngestError;
use crate::platform::path_from_file_uri;

pub fn reference_from(payload: Payload) -> Result<Option<Reference>, IngestError> {
    match payload {
        Payload::Files(paths) => Ok(keep_first_discard_rest(paths).map(Reference::Path)),
        Payload::Pixels(pixels) => Ok(Some(Reference::Inline(Acquired::Pixels(pixels)))),
        Payload::Bytes(bytes) => {
            ensure_within_limit(bytes.len() as u64)?;
            Ok(looks_like_image(&bytes).then_some(Reference::Inline(Acquired::Encoded(bytes))))
        }
        Payload::Html(fragment) => {
            let address = html::first_image_source(&fragment.markup)
                .and_then(|source| absolute_html_source(source, fragment.base_url.as_deref()));
            match address {
                Some(address) => reference_from_address(&address),
                None => Ok(None),
            }
        }
        Payload::Text(text) => {
            let first_line = keep_first_discard_rest(
                text.lines().map(str::trim).filter(|line| !line.is_empty()),
            );
            match first_line {
                Some(line) => reference_from_address(line),
                None => Ok(None),
            }
        }
    }
}

fn absolute_html_source(source: String, base_url: Option<&str>) -> Option<String> {
    if let Some(base) = base_url
        && !has_absolute_scheme(&source)
    {
        return relative_url::join(base, &source);
    }
    match source.strip_prefix("//") {
        Some(network_path) => Some(format!("https://{network_path}")),
        None => has_absolute_scheme(&source).then_some(source),
    }
}

fn has_absolute_scheme(address: &str) -> bool {
    ["http://", "https://", "data:", "file://"]
        .iter()
        .any(|scheme| has_prefix(address, scheme))
}

fn reference_from_address(address: &str) -> Result<Option<Reference>, IngestError> {
    let address = unquote(address);
    if has_prefix(address, "http://") || has_prefix(address, "https://") {
        return Ok(Some(Reference::Url(address.to_owned())));
    }
    if has_prefix(address, "data:") {
        let bytes = decode_data_uri(&address["data:".len()..])?;
        return Ok(Some(Reference::Inline(Acquired::Encoded(bytes))));
    }
    if has_prefix(address, "file://") {
        return Ok(path_from_file_uri(address).map(Reference::Path));
    }
    let path = Path::new(address);
    Ok(path.is_absolute().then(|| Reference::Path(path.to_owned())))
}

fn unquote(address: &str) -> &str {
    let address = address.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = address
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner.trim();
        }
    }
    address
}

fn has_prefix(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

fn decode_data_uri(body: &str) -> Result<Vec<u8>, IngestError> {
    let (header, payload) = body.split_once(',').ok_or(IngestError::MalformedDataUri)?;
    let is_base64 = header
        .rsplit(';')
        .next()
        .is_some_and(|parameter| parameter.trim().eq_ignore_ascii_case("base64"));
    let bytes = if is_base64 {
        let compact: Vec<u8> = payload
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        ensure_within_limit((compact.len() / 4 * 3) as u64)?;
        STANDARD_PAD_INDIFFERENT
            .decode(compact)
            .map_err(|_| IngestError::MalformedDataUri)?
    } else {
        ensure_within_limit(payload.len() as u64)?;
        percent_decode_str(payload).collect()
    };
    ensure_within_limit(bytes.len() as u64)?;
    Ok(bytes)
}

fn ensure_within_limit(size: u64) -> Result<(), IngestError> {
    if size > limits::encoded() {
        return Err(IngestError::InputTooLarge {
            size,
            limit: limits::encoded(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ingest::HtmlFragment;
    use std::path::PathBuf;

    fn text(value: &str) -> Result<Option<Reference>, IngestError> {
        reference_from(Payload::Text(value.to_owned()))
    }

    fn url_of(reference: Result<Option<Reference>, IngestError>) -> Option<String> {
        match reference {
            Ok(Some(Reference::Url(url))) => Some(url),
            _ => None,
        }
    }

    fn path_of(reference: Result<Option<Reference>, IngestError>) -> Option<PathBuf> {
        match reference {
            Ok(Some(Reference::Path(path))) => Some(path),
            _ => None,
        }
    }

    fn bytes_of(reference: Result<Option<Reference>, IngestError>) -> Option<Vec<u8>> {
        match reference {
            Ok(Some(Reference::Inline(Acquired::Encoded(bytes)))) => Some(bytes),
            _ => None,
        }
    }

    #[test]
    fn text_links_become_urls() {
        assert_eq!(
            url_of(text("  https://a.test/x.png \n")).as_deref(),
            Some("https://a.test/x.png")
        );
        assert_eq!(
            url_of(text("HTTP://a.test/x.png")).as_deref(),
            Some("HTTP://a.test/x.png")
        );
    }

    #[test]
    fn text_keeps_first_non_empty_line_only() {
        assert_eq!(
            url_of(text("\n\nhttps://a.test/1.png\nhttps://a.test/2.png")).as_deref(),
            Some("https://a.test/1.png")
        );
        assert!(matches!(text("not a link\nhttps://a.test/2.png"), Ok(None)));
    }

    #[test]
    fn quoted_paths_are_unquoted() {
        let reference = text(if cfg!(windows) {
            "\"C:\\pics\\a.png\""
        } else {
            "\"/pics/a.png\""
        });
        let expected = if cfg!(windows) {
            "C:\\pics\\a.png"
        } else {
            "/pics/a.png"
        };
        assert_eq!(path_of(reference), Some(PathBuf::from(expected)));
    }

    #[test]
    fn relative_text_is_not_a_path() {
        assert!(matches!(text("pics/a.png"), Ok(None)));
        assert!(matches!(text("hello"), Ok(None)));
        assert!(matches!(text(""), Ok(None)));
    }

    #[cfg(unix)]
    #[test]
    fn file_uris_become_paths() {
        assert_eq!(
            path_of(text("file:///tmp/my%20pic.png")),
            Some(PathBuf::from("/tmp/my pic.png"))
        );
        assert!(matches!(text("file://otherhost/tmp/a.png"), Ok(None)));
    }

    #[test]
    fn files_keep_first() {
        let payload = Payload::Files(vec![PathBuf::from("a"), PathBuf::from("b")]);
        assert_eq!(path_of(reference_from(payload)), Some(PathBuf::from("a")));
        assert!(matches!(
            reference_from(Payload::Files(Vec::new())),
            Ok(None)
        ));
    }

    #[test]
    fn base64_data_uris_decode() {
        assert_eq!(
            bytes_of(text("data:image/png;base64,AQID")),
            Some(vec![1, 2, 3])
        );
        assert_eq!(
            bytes_of(text("data:image/png;base64,AQI")),
            Some(vec![1, 2])
        );
        assert_eq!(
            bytes_of(reference_from(Payload::Html(HtmlFragment::without_base(
                "<img src=\"data:image/png;base64,AQ\n ID\">".to_owned()
            )))),
            Some(vec![1, 2, 3])
        );
    }

    #[test]
    fn plain_data_uris_percent_decode() {
        assert_eq!(
            bytes_of(text("data:image/bmp,%01%02a")),
            Some(vec![1, 2, b'a'])
        );
    }

    #[test]
    fn malformed_data_uris_are_errors() {
        assert!(matches!(
            text("data:image/png;base64,@@@@"),
            Err(IngestError::MalformedDataUri)
        ));
        assert!(matches!(
            text("data:image/png;base64"),
            Err(IngestError::MalformedDataUri)
        ));
    }

    const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    #[test]
    fn image_blobs_are_recognised_by_content() {
        let blob = |bytes: &[u8]| reference_from(Payload::Bytes(bytes.to_vec()));
        assert_eq!(bytes_of(blob(&PNG_SIGNATURE)), Some(PNG_SIGNATURE.to_vec()));
        assert!(matches!(blob(b"https://a.test/x.png"), Ok(None)));
        assert!(matches!(blob(&[]), Ok(None)));
    }

    #[test]
    fn html_sources_are_resolved() {
        let html = |markup: &str| {
            reference_from(Payload::Html(HtmlFragment::without_base(markup.to_owned())))
        };
        assert_eq!(
            url_of(html(r#"<img src="https://a.test/i.png?x=1&amp;y=2">"#)).as_deref(),
            Some("https://a.test/i.png?x=1&y=2")
        );
        assert_eq!(
            url_of(html(r#"<img src="//cdn.test/i.png">"#)).as_deref(),
            Some("https://cdn.test/i.png")
        );
        assert_eq!(
            bytes_of(html(r#"<img src="data:image/png;base64,AQID">"#)),
            Some(vec![1, 2, 3])
        );
        assert!(matches!(html(r#"<img src="relative/i.png">"#), Ok(None)));
        assert!(matches!(html(r#"<img src="/tmp/i.png">"#), Ok(None)));
        assert!(matches!(html("<p>text</p>"), Ok(None)));
    }

    fn based_html(markup: &str, base: &str) -> Result<Option<Reference>, IngestError> {
        reference_from(Payload::Html(HtmlFragment {
            markup: markup.to_owned(),
            base_url: Some(base.to_owned()),
        }))
    }

    #[test]
    fn relative_html_sources_resolve_against_the_base_url() {
        let base = "https://site.test/gallery/index.html";
        let resolved = |markup: &str| url_of(based_html(markup, base));
        assert_eq!(
            resolved(r#"<img src="img/a.png">"#).as_deref(),
            Some("https://site.test/gallery/img/a.png")
        );
        assert_eq!(
            resolved(r#"<img src="/a.png">"#).as_deref(),
            Some("https://site.test/a.png")
        );
        assert_eq!(
            resolved(r#"<img src="//cdn.test/a.png">"#).as_deref(),
            Some("https://cdn.test/a.png")
        );
    }

    #[test]
    fn absolute_html_sources_ignore_the_base_url() {
        let base = "https://site.test/gallery/index.html";
        assert_eq!(
            url_of(based_html(r#"<img src="https://other.test/a.png">"#, base)).as_deref(),
            Some("https://other.test/a.png")
        );
        assert_eq!(
            bytes_of(based_html(
                r#"<img src="data:image/png;base64,AQID">"#,
                base
            )),
            Some(vec![1, 2, 3])
        );
    }

    #[test]
    fn unusable_base_urls_leave_relative_sources_unrecognised() {
        assert!(matches!(
            based_html(r#"<img src="a.png">"#, "about:blank"),
            Ok(None)
        ));
    }
}
