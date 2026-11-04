use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::ColorImage;
use rfd::AsyncFileDialog;

use crate::app::jobs::{Completed, Outcome, Reporter};
use crate::app::size_warning;
use crate::core::edit::Orientation;
use crate::core::encode::{EncodeOptions, Format, encode};
use crate::core::export::straight_oriented;
use crate::error::JobError;

const DEFAULT_FORMAT: Format = Format::Png;
const PART_SUFFIX: &str = ".komaru-part";

pub struct SaveRequest {
    pub suggested_name: String,
    pub directory: Option<PathBuf>,
    pub pixels: Arc<ColorImage>,
    pub orientation: Orientation,
    pub options: EncodeOptions,
    pub warn_above: Option<u64>,
}

pub fn suggested_name(title: &str, forced: Option<Format>) -> String {
    let path = Path::new(title);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("image");
    let format = forced.or_else(|| format_of(path)).unwrap_or(DEFAULT_FORMAT);
    format!("{stem}.{}", format.extensions()[0])
}

pub fn save_as(request: SaveRequest, reporter: &Reporter) -> Outcome {
    let Some(chosen) = pick_path(&request) else {
        return Ok(Completed::Cancelled);
    };
    let (path, format) = resolve_format(chosen)?;
    reporter.working("Saving image...");
    let image = straight_oriented(
        bytemuck::cast_slice(&request.pixels.pixels),
        request.pixels.size,
        request.orientation,
    );
    let bytes = encode(&image, format, request.options)?;
    if let Some(source_bytes) = request.warn_above.filter(|_| format == Format::Png)
        && !size_warning::confirmed(reporter, bytes.len() as u64, source_bytes)
    {
        return Ok(Completed::Cancelled);
    }
    write_atomically(&path, &bytes).map_err(|source| JobError::Io {
        path: path.clone(),
        source,
    })?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    Ok(Completed::Saved(name.unwrap_or_default()))
}

fn format_of(path: &Path) -> Option<Format> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(Format::from_extension)
}

fn pick_path(request: &SaveRequest) -> Option<PathBuf> {
    let preferred = format_of(Path::new(&request.suggested_name));
    let mut formats = Format::ALL;
    formats.sort_by_key(|format| Some(*format) != preferred);
    let mut dialog = AsyncFileDialog::new().set_file_name(&request.suggested_name);
    if let Some(directory) = &request.directory {
        dialog = dialog.set_directory(directory);
    }
    for format in formats {
        dialog = dialog.add_filter(format.label(), format.extensions());
    }
    pollster::block_on(dialog.save_file()).map(|file| file.path().to_owned())
}

fn resolve_format(path: PathBuf) -> Result<(PathBuf, Format), JobError> {
    let Some(extension) = path.extension() else {
        let completed = path.with_extension(DEFAULT_FORMAT.extensions()[0]);
        return Ok((completed, DEFAULT_FORMAT));
    };
    match Format::from_extension(&extension.to_string_lossy()) {
        Some(format) => Ok((path, format)),
        None => Err(JobError::UnsupportedExtension {
            extension: extension.to_string_lossy().into_owned(),
        }),
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut part = path.as_os_str().to_owned();
    part.push(PART_SUFFIX);
    let part = PathBuf::from(part);
    let result = fs::write(&part, bytes).and_then(|()| fs::rename(&part, path));
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_the_stem_and_a_writable_extension() {
        assert_eq!(suggested_name("holiday.JPG", None), "holiday.jpg");
        assert_eq!(suggested_name("scan.png", None), "scan.png");
        assert_eq!(suggested_name("photo.webp", None), "photo.webp");
        assert_eq!(suggested_name("diagram.gif", None), "diagram.png");
        assert_eq!(suggested_name("noextension", None), "noextension.png");
        assert_eq!(suggested_name("", None), "image.png");
        assert_eq!(
            suggested_name("holiday.jpg", Some(Format::Png)),
            "holiday.png"
        );
        assert_eq!(suggested_name("pasted", Some(Format::Jpeg)), "pasted.jpg");
    }

    #[test]
    fn a_missing_extension_becomes_png_and_an_unknown_one_is_refused() {
        let (path, format) = resolve_format(PathBuf::from("/x/shot")).expect("png is the default");
        assert_eq!((path, format), (PathBuf::from("/x/shot.png"), Format::Png));
        let (path, format) = resolve_format(PathBuf::from("/x/a.JPEG")).expect("jpeg is known");
        assert_eq!((path, format), (PathBuf::from("/x/a.JPEG"), Format::Jpeg));
        let refused = resolve_format(PathBuf::from("/x/a.tiff"));
        assert!(matches!(
            refused,
            Err(JobError::UnsupportedExtension { .. })
        ));
    }

    #[test]
    fn writing_replaces_the_target_and_leaves_no_part_file() {
        let directory = std::env::temp_dir().join(format!("komaru-save-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory is creatable");
        let target = directory.join("out.bin");
        fs::write(&target, b"old contents").expect("target is writable");
        write_atomically(&target, b"new").expect("write succeeds");
        assert_eq!(fs::read(&target).expect("target is readable"), b"new");
        let leftovers = fs::read_dir(&directory)
            .expect("directory is readable")
            .count();
        assert_eq!(leftovers, 1);
        fs::remove_dir_all(directory).expect("temp directory is removable");
    }

    #[test]
    fn a_failed_write_leaves_the_old_file_alone() {
        let directory = std::env::temp_dir().join(format!("komaru-fail-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory is creatable");
        let missing_parent = directory.join("nowhere").join("out.bin");
        assert!(write_atomically(&missing_parent, b"x").is_err());
        fs::remove_dir_all(directory).expect("temp directory is removable");
    }
}
