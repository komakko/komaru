use std::borrow::Cow;

use arboard::{Clipboard, Error, ImageData};

use crate::core::ingest::{HtmlFragment, Payload, RawPixels};
use crate::error::PlatformError;

pub fn read() -> Result<Vec<Payload>, PlatformError> {
    let mut clipboard = Clipboard::new()?;

    if let Some(files) = present(clipboard.get().file_list())?
        && !files.is_empty()
    {
        return Ok(vec![Payload::Files(files)]);
    }
    if let Some(image) = present(clipboard.get().image())? {
        return Ok(vec![Payload::Pixels(RawPixels {
            width: image.width,
            height: image.height,
            rgba: image.bytes.into_owned(),
        })]);
    }

    let mut payloads = Vec::new();
    if let Some(html) = present_html(clipboard.get().html())? {
        payloads.push(Payload::Html(HtmlFragment::without_base(html)));
    }
    if let Some(text) = present(clipboard.get().text())? {
        payloads.push(Payload::Text(text));
    }
    Ok(payloads)
}

pub fn write(image: RawPixels) -> Result<(), PlatformError> {
    let data = ImageData {
        width: image.width,
        height: image.height,
        bytes: Cow::Owned(image.rgba),
    };
    with_writer(|clipboard| clipboard.set_image(data))
}

pub fn write_text(text: String) -> Result<(), PlatformError> {
    with_writer(|clipboard| clipboard.set_text(text))
}

pub fn write_file(path: &std::path::Path) -> Result<(), PlatformError> {
    with_writer(|clipboard| clipboard.set().file_list(&[path]))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn with_writer(
    write: impl FnOnce(&mut Clipboard) -> Result<(), Error>,
) -> Result<(), PlatformError> {
    static WRITER: std::sync::Mutex<Option<Clipboard>> = std::sync::Mutex::new(None);
    let mut slot = WRITER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let clipboard = match &mut *slot {
        Some(clipboard) => clipboard,
        empty => empty.insert(Clipboard::new().map_err(PlatformError::ClipboardWrite)?),
    };
    write(clipboard).map_err(PlatformError::ClipboardWrite)
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn with_writer(
    write: impl FnOnce(&mut Clipboard) -> Result<(), Error>,
) -> Result<(), PlatformError> {
    let mut clipboard = Clipboard::new().map_err(PlatformError::ClipboardWrite)?;
    write(&mut clipboard).map_err(PlatformError::ClipboardWrite)
}

fn present<T>(result: Result<T, Error>) -> Result<Option<T>, Error> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(Error::ContentNotAvailable | Error::ConversionFailure) => Ok(None),
        Err(error) => Err(error),
    }
}

fn present_html(result: Result<String, Error>) -> Result<Option<String>, Error> {
    match result {
        Err(Error::Unknown { .. }) => Ok(None),
        other => present(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unknown(description: &str) -> Error {
        Error::Unknown {
            description: description.to_owned(),
        }
    }

    #[test]
    fn missing_content_is_absent_not_an_error() {
        assert!(matches!(
            present::<String>(Err(Error::ContentNotAvailable)),
            Ok(None)
        ));
        assert!(matches!(
            present::<String>(Err(Error::ConversionFailure)),
            Ok(None)
        ));
    }

    #[test]
    fn real_failures_are_still_errors() {
        assert!(present::<String>(Err(Error::ClipboardOccupied)).is_err());
        assert!(present::<String>(Err(unknown("boom"))).is_err());
    }

    #[test]
    fn windows_reports_missing_html_as_unknown_and_that_is_absent() {
        assert!(matches!(
            present_html(Err(unknown("failed to read clipboard string"))),
            Ok(None)
        ));
        assert!(present_html(Err(Error::ClipboardOccupied)).is_err());
        assert_eq!(
            present_html(Ok("<img>".to_owned())).unwrap().as_deref(),
            Some("<img>")
        );
    }
}
