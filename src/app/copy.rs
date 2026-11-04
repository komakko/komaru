use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::ColorImage;

use crate::app::jobs::{Completed, Outcome, Reporter};
use crate::app::size_warning;
use crate::core::edit::Orientation;
use crate::core::encode::{EncodeOptions, Format, encode};
use crate::core::export::straight_oriented;
use crate::platform::clipboard;

#[derive(Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub size: u64,
}

pub struct CopyRequest {
    pub file: Option<SourceFile>,
    pub pixels: Arc<ColorImage>,
    pub orientation: Orientation,
    pub options: EncodeOptions,
    pub warn_above: Option<u64>,
}

pub fn copy(request: CopyRequest, reporter: &Reporter) -> Outcome {
    if let Some(path) = request.file.as_ref().and_then(unchanged_path) {
        clipboard::write_file(&path)?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        return Ok(Completed::CopiedFile(name.unwrap_or_default()));
    }
    reporter.working("Copying image...");
    let image = straight_oriented(
        bytemuck::cast_slice(&request.pixels.pixels),
        request.pixels.size,
        request.orientation,
    );
    let size = [image.width, image.height];
    if let Some(source_bytes) = request.warn_above {
        let png_bytes = encode(&image, Format::Png, request.options)?.len() as u64;
        if !size_warning::confirmed(reporter, png_bytes, source_bytes) {
            return Ok(Completed::Cancelled);
        }
    }
    clipboard::write(image)?;
    Ok(Completed::CopiedImage(size))
}

fn unchanged_path(file: &SourceFile) -> Option<PathBuf> {
    let metadata = fs::metadata(&file.path).ok()?;
    if !metadata.is_file() || metadata.len() != file.size {
        return None;
    }
    std::path::absolute(&file.path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(label: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("komaru-{label}-{}", std::process::id()));
        fs::write(&path, bytes).expect("temp file is writable");
        path
    }

    #[test]
    fn an_unchanged_file_is_offered_by_absolute_path() {
        let path = temp_file("copy-same", b"12345");
        let file = SourceFile {
            path: path.clone(),
            size: 5,
        };
        assert_eq!(unchanged_path(&file), Some(path.clone()));
        fs::remove_file(path).expect("temp file is removable");
    }

    #[test]
    fn a_file_that_changed_size_or_vanished_is_not_offered() {
        let path = temp_file("copy-changed", b"12345");
        let resized = SourceFile {
            path: path.clone(),
            size: 9,
        };
        assert_eq!(unchanged_path(&resized), None);
        fs::remove_file(&path).expect("temp file is removable");
        let gone = SourceFile { path, size: 5 };
        assert_eq!(unchanged_path(&gone), None);
    }
}
