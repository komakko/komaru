use std::fs;
use std::io;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::ColorImage;
use image::ImageFormat;
use rfd::AsyncFileDialog;

use crate::app::copy::SourceFile;
use crate::app::jobs::{Completed, Outcome, Reporter};
use crate::app::size_warning;
use crate::core::animation_encode::encode_animation;
use crate::core::edit::Orientation;
use crate::core::encode::{EncodeOptions, Format, encode};
use crate::core::export::straight_oriented;
use crate::error::JobError;

const DEFAULT_FORMAT: Format = Format::Png;
const PART_SUFFIX: &str = ".komaru-part";

pub struct AnimatedSave {
    pub frames: Vec<(Arc<ColorImage>, Duration)>,
    pub plays: Option<NonZeroU32>,
    pub unedited_file: Option<SourceFile>,
}

pub struct SaveRequest {
    pub suggested_name: String,
    pub directory: Option<PathBuf>,
    pub animation: Option<AnimatedSave>,
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
    let animation = request.animation.as_ref().filter(|_| format.animates());
    let bytes = match animation {
        Some(animation) => {
            reporter.working("Saving animation...");
            animation_bytes(&request, animation, format)?
        }
        None => {
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
            bytes
        }
    };
    write_atomically(&path, &bytes).map_err(|source| JobError::Io {
        path: path.clone(),
        source,
    })?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let name = name.unwrap_or_default();
    Ok(if request.animation.is_some() && animation.is_none() {
        Completed::SavedFrame(name)
    } else {
        Completed::Saved(name)
    })
}

fn animation_bytes(
    request: &SaveRequest,
    animation: &AnimatedSave,
    format: Format,
) -> Result<Vec<u8>, JobError> {
    if let Some(bytes) = unedited_bytes(animation, format) {
        return Ok(bytes);
    }
    let frames = animation.frames.iter().map(|(pixels, delay)| {
        let image = straight_oriented(
            bytemuck::cast_slice(&pixels.pixels),
            pixels.size,
            request.orientation,
        );
        (image, *delay)
    });
    Ok(encode_animation(
        frames,
        animation.plays,
        format,
        request.options,
    )?)
}

fn unedited_bytes(animation: &AnimatedSave, format: Format) -> Option<Vec<u8>> {
    let file = animation.unedited_file.as_ref()?;
    let bytes = fs::read(&file.path).ok()?;
    let same_file = bytes.len() as u64 == file.size;
    (same_file && image::guess_format(&bytes).ok() == container(format)).then_some(bytes)
}

fn container(format: Format) -> Option<ImageFormat> {
    match format {
        Format::Gif => Some(ImageFormat::Gif),
        Format::WebP => Some(ImageFormat::WebP),
        Format::Png => Some(ImageFormat::Png),
        Format::Jpeg | Format::Avif | Format::Bmp => None,
    }
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

    fn animation(file: Option<SourceFile>) -> AnimatedSave {
        let frame = |shade: u8| {
            (
                Arc::new(ColorImage::filled(
                    [3, 2],
                    eframe::egui::Color32::from_rgb(shade, 20, 30),
                )),
                Duration::from_millis(80),
            )
        };
        AnimatedSave {
            frames: vec![frame(10), frame(200)],
            plays: None,
            unedited_file: file,
        }
    }

    fn request(animation: AnimatedSave) -> SaveRequest {
        SaveRequest {
            suggested_name: String::new(),
            directory: None,
            pixels: Arc::clone(&animation.frames[0].0),
            animation: Some(animation),
            orientation: Orientation::default(),
            options: EncodeOptions::default(),
            warn_above: None,
        }
    }

    #[test]
    fn an_unedited_animation_in_its_own_format_is_saved_as_is() {
        let directory = std::env::temp_dir().join(format!("komaru-as-is-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temp directory is creatable");
        let path = directory.join("source.gif");
        let original = b"GIF89a-not-really-but-guessable".to_vec();
        fs::write(&path, &original).expect("source is writable");
        let file = SourceFile {
            path,
            size: original.len() as u64,
        };
        let request = request(animation(Some(file)));
        let saved = animation_bytes(&request, request.animation.as_ref().unwrap(), Format::Gif)
            .expect("saves");
        assert_eq!(saved, original);
        let converted = animation_bytes(&request, request.animation.as_ref().unwrap(), Format::Png)
            .expect("saves");
        assert_ne!(converted, original);
        assert_eq!(
            image::guess_format(&converted).expect("known"),
            ImageFormat::Png
        );
        fs::remove_dir_all(directory).expect("temp directory is removable");
    }

    #[test]
    fn an_edited_animation_is_encoded_with_every_frame() {
        let request = request(animation(None));
        let bytes = animation_bytes(&request, request.animation.as_ref().unwrap(), Format::Gif)
            .expect("saves");
        let decoded = crate::core::animation::decode_frames(&bytes, u64::MAX, || true)
            .expect("decodes")
            .expect("is current");
        assert_eq!(decoded.frames.len(), 2);
    }

    #[test]
    fn names_keep_the_stem_and_a_writable_extension() {
        assert_eq!(suggested_name("holiday.JPG", None), "holiday.jpg");
        assert_eq!(suggested_name("scan.png", None), "scan.png");
        assert_eq!(suggested_name("photo.webp", None), "photo.webp");
        assert_eq!(suggested_name("diagram.gif", None), "diagram.gif");
        assert_eq!(suggested_name("photo.avif", None), "photo.avif");
        assert_eq!(suggested_name("shot.tiff", None), "shot.png");
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
