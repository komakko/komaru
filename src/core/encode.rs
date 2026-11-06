use image::codecs::avif::AvifEncoder;
use image::codecs::bmp::BmpEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};

use crate::core::gif_encode::{self, GifOptions};
use crate::core::ingest::RawPixels;
use crate::core::rgba::Rgba;
use crate::error::EncodeError;

const WEBP_LOSSY_FAILURE: &str = "lossy WebP encoder failed";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PngLevel {
    Fast,
    Default,
    Best,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WebpMode {
    Lossless,
    Lossy,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AvifSpeed {
    Fast,
    Balanced,
    Best,
}

impl AvifSpeed {
    fn rav1e(self) -> u8 {
        match self {
            Self::Fast => 10,
            Self::Balanced => 7,
            Self::Best => 4,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EncodeOptions {
    pub lossy_quality: u8,
    pub webp: WebpMode,
    pub png: PngLevel,
    pub avif_speed: AvifSpeed,
    pub gif: GifOptions,
    pub flatten: Rgba,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            lossy_quality: 90,
            webp: WebpMode::Lossless,
            png: PngLevel::Default,
            avif_speed: AvifSpeed::Balanced,
            gif: GifOptions::default(),
            flatten: Rgba::new(255, 255, 255, 255),
        }
    }
}

impl PngLevel {
    fn compression(self) -> CompressionType {
        match self {
            Self::Fast => CompressionType::Fast,
            Self::Default => CompressionType::Default,
            Self::Best => CompressionType::Best,
        }
    }

    pub fn animated_compression(self) -> png::Compression {
        match self {
            Self::Fast => png::Compression::Fast,
            Self::Default => png::Compression::Balanced,
            Self::Best => png::Compression::High,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Png,
    Jpeg,
    WebP,
    Avif,
    Gif,
    Bmp,
}

impl Format {
    pub const ALL: [Self; 6] = [
        Self::Png,
        Self::Jpeg,
        Self::WebP,
        Self::Avif,
        Self::Gif,
        Self::Bmp,
    ];

    pub fn animates(self) -> bool {
        matches!(self, Self::Png | Self::WebP | Self::Gif)
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| {
            format
                .extensions()
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Png => &["png"],
            Self::Jpeg => &["jpg", "jpeg"],
            Self::WebP => &["webp"],
            Self::Avif => &["avif"],
            Self::Gif => &["gif"],
            Self::Bmp => &["bmp"],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::WebP => "WebP",
            Self::Avif => "AVIF",
            Self::Gif => "GIF",
            Self::Bmp => "BMP",
        }
    }
}

pub fn encode(
    image: &RawPixels,
    format: Format,
    options: EncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    let (width, height) = (image.width as u32, image.height as u32);
    let mut bytes = Vec::new();
    match format {
        Format::Png => PngEncoder::new_with_quality(
            &mut bytes,
            options.png.compression(),
            FilterType::Adaptive,
        )
        .write_image(&image.rgba, width, height, ExtendedColorType::Rgba8)?,
        Format::WebP => match options.webp {
            WebpMode::Lossless => WebPEncoder::new_lossless(&mut bytes).write_image(
                &image.rgba,
                width,
                height,
                ExtendedColorType::Rgba8,
            )?,
            WebpMode::Lossy => {
                let encoder = webp::Encoder::from_rgba(&image.rgba, width, height);
                let encoded = encoder
                    .encode_simple(false, f32::from(options.lossy_quality))
                    .map_err(|_| EncodeError::Backend(WEBP_LOSSY_FAILURE))?;
                bytes.extend_from_slice(&encoded);
            }
        },
        Format::Jpeg => JpegEncoder::new_with_quality(&mut bytes, options.lossy_quality)
            .write_image(
                &flattened(&image.rgba, options.flatten),
                width,
                height,
                ExtendedColorType::Rgb8,
            )?,
        Format::Avif => AvifEncoder::new_with_speed_quality(
            &mut bytes,
            options.avif_speed.rav1e(),
            options.lossy_quality,
        )
        .write_image(&image.rgba, width, height, ExtendedColorType::Rgba8)?,
        Format::Gif => {
            let still = RawPixels {
                width: image.width,
                height: image.height,
                rgba: image.rgba.clone(),
            };
            bytes = gif_encode::encode(
                std::iter::once((still, std::time::Duration::ZERO)),
                std::num::NonZeroU32::new(1),
                options.gif,
            )?;
        }
        Format::Bmp => BmpEncoder::new(&mut bytes).write_image(
            &flattened(&image.rgba, options.flatten),
            width,
            height,
            ExtendedColorType::Rgb8,
        )?,
    }
    Ok(bytes)
}

fn flattened(rgba: &[u8], background: Rgba) -> Vec<u8> {
    let background = [background.r, background.g, background.b].map(u32::from);
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        let alpha = u32::from(pixel[3]);
        rgb.extend(pixel[..3].iter().zip(background).map(|(&channel, behind)| {
            ((u32::from(channel) * alpha + behind * (255 - alpha) + 127) / 255) as u8
        }));
    }
    rgb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(width: usize, height: usize, alpha: impl Fn(usize) -> u8) -> RawPixels {
        let mut rgba = Vec::with_capacity(width * height * 4);
        for index in 0..width * height {
            let (x, y) = (index % width, index / width);
            rgba.extend([
                (x * 255 / width) as u8,
                (y * 255 / height) as u8,
                90,
                alpha(index),
            ]);
        }
        RawPixels {
            width,
            height,
            rgba,
        }
    }

    fn decoded(bytes: &[u8]) -> image::RgbaImage {
        image::load_from_memory(bytes)
            .expect("encoder output decodes")
            .to_rgba8()
    }

    #[test]
    fn lossless_formats_round_trip_with_alpha() {
        let source = gradient(37, 23, |index| (index * 7 % 256) as u8);
        for format in [Format::Png, Format::WebP] {
            let bytes = encode(&source, format, EncodeOptions::default()).expect("encodes");
            assert_eq!(decoded(&bytes).into_raw(), source.rgba, "{format:?}");
        }
    }

    #[test]
    fn avif_round_trips_with_alpha_and_gif_is_exact_when_it_fits() {
        let source = gradient(48, 32, |index| if index % 5 == 0 { 0 } else { 255 });
        let options = EncodeOptions {
            avif_speed: AvifSpeed::Fast,
            lossy_quality: 95,
            ..EncodeOptions::default()
        };
        let avif = decoded(&encode(&source, Format::Avif, options).expect("encodes"));
        assert_eq!((avif.width(), avif.height()), (48, 32));
        let flat = RawPixels {
            width: 4,
            height: 2,
            rgba: [[255, 0, 0, 255], [0, 255, 0, 255]]
                .into_iter()
                .cycle()
                .take(8)
                .flatten()
                .collect(),
        };
        let gif = decoded(&encode(&flat, Format::Gif, EncodeOptions::default()).expect("encodes"));
        assert_eq!(gif.into_raw(), flat.rgba);
    }

    #[test]
    fn bmp_and_opaque_jpeg_round_trip_closely() {
        let source = gradient(40, 24, |_| 255);
        let bmp =
            decoded(&encode(&source, Format::Bmp, EncodeOptions::default()).expect("encodes"));
        assert_eq!(bmp.into_raw(), source.rgba);
        let jpeg =
            decoded(&encode(&source, Format::Jpeg, EncodeOptions::default()).expect("encodes"));
        let worst = jpeg
            .into_raw()
            .iter()
            .zip(&source.rgba)
            .map(|(got, want)| got.abs_diff(*want))
            .max();
        assert!(
            worst.is_some_and(|difference| difference <= 12),
            "{worst:?}"
        );
    }

    #[test]
    fn jpeg_and_bmp_flatten_transparency_onto_white() {
        let source = RawPixels {
            width: 3,
            height: 1,
            rgba: vec![10, 20, 30, 0, 0, 0, 0, 128, 40, 50, 60, 255],
        };
        let bmp =
            decoded(&encode(&source, Format::Bmp, EncodeOptions::default()).expect("encodes"))
                .into_raw();
        assert_eq!(&bmp[0..4], &[255, 255, 255, 255]);
        assert_eq!(&bmp[4..8], &[127, 127, 127, 255]);
        assert_eq!(&bmp[8..12], &[40, 50, 60, 255]);
    }

    #[test]
    fn alpha_survives_in_png_and_webp() {
        let source = RawPixels {
            width: 2,
            height: 1,
            rgba: vec![10, 20, 30, 0, 200, 100, 50, 128],
        };
        for format in [Format::Png, Format::WebP] {
            let pixels =
                decoded(&encode(&source, format, EncodeOptions::default()).expect("encodes"))
                    .into_raw();
            assert_eq!(pixels[7], 128, "{format:?}");
            assert_eq!(pixels[3], 0, "{format:?}");
        }
    }

    #[test]
    fn lossy_webp_decodes_and_is_smaller_than_lossless() {
        let source = gradient(64, 48, |_| 255);
        let lossy_options = EncodeOptions {
            webp: WebpMode::Lossy,
            lossy_quality: 60,
            ..EncodeOptions::default()
        };
        let lossy = encode(&source, Format::WebP, lossy_options).expect("encodes");
        let lossless = encode(&source, Format::WebP, EncodeOptions::default()).expect("encodes");
        assert_eq!(decoded(&lossy).dimensions(), (64, 48));
        assert!(lossy.len() < lossless.len());
    }

    #[test]
    fn flatten_colour_replaces_white_behind_transparent_pixels() {
        let source = RawPixels {
            width: 1,
            height: 1,
            rgba: vec![9, 9, 9, 0],
        };
        let options = EncodeOptions {
            flatten: Rgba::new(10, 20, 30, 255),
            ..EncodeOptions::default()
        };
        let bmp = decoded(&encode(&source, Format::Bmp, options).expect("encodes")).into_raw();
        assert_eq!(&bmp[..3], &[10, 20, 30]);
    }

    #[test]
    fn extensions_map_to_formats_ignoring_case() {
        assert_eq!(Format::from_extension("PNG"), Some(Format::Png));
        assert_eq!(Format::from_extension("jpg"), Some(Format::Jpeg));
        assert_eq!(Format::from_extension("JPEG"), Some(Format::Jpeg));
        assert_eq!(Format::from_extension("webp"), Some(Format::WebP));
        assert_eq!(Format::from_extension("bmp"), Some(Format::Bmp));
        assert_eq!(Format::from_extension("tiff"), None);
        assert_eq!(Format::from_extension(""), None);
    }
}
