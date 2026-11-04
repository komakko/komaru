use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader};

use crate::core::animation;
use crate::core::avif_orientation::avif_orientation;
use crate::core::colour::convert_to_srgb;
use crate::core::info::{ImageInfo, Probe};
use crate::core::ingest::{Acquired, RawPixels};
use crate::core::limits;
use crate::error::DecodeError;

const BYTES_PER_PIXEL: u64 = 4;
const BYTES_PER_MIB: u64 = 1024 * 1024;

pub struct DecodedImage {
    pub width: usize,
    pub height: usize,
    pub premultiplied_rgba: Vec<u8>,
}

pub struct Decoded {
    pub image: DecodedImage,
    pub info: ImageInfo,
    pub animated: bool,
}

pub fn decode(acquired: &Acquired) -> Result<Decoded, DecodeError> {
    decode_within(acquired, limits::decoded())
}

fn decode_within(acquired: &Acquired, limit: u64) -> Result<Decoded, DecodeError> {
    match acquired {
        Acquired::Encoded(bytes) => decode_encoded(bytes, limit),
        Acquired::Pixels(pixels) => decode_pixels(pixels, limit).map(|image| Decoded {
            image,
            info: ImageInfo::default(),
            animated: false,
        }),
    }
}

fn decode_encoded(bytes: &[u8], limit: u64) -> Result<Decoded, DecodeError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(ImageError::from)?;
    let Some(format) = reader.format() else {
        return Err(DecodeError::UnknownFormat);
    };
    let mut decoder = reader.into_decoder()?;

    let (width, height) = decoder.dimensions();
    check_decoded_limit(width as usize, height as usize, limit)?;

    let orientation = if format == ImageFormat::Avif {
        avif_orientation(bytes)
    } else {
        decoder.orientation()?
    };
    let icc_profile = decoder.icc_profile()?;
    let exif = decoder.exif_metadata().ok().flatten();
    let info = ImageInfo::from_probe(&Probe {
        format,
        color: decoder.original_color_type(),
        orientation,
        icc_profile: icc_profile.as_deref(),
        exif: exif.as_deref(),
    });
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);

    Ok(Decoded {
        image: into_decoded_image(image, icc_profile.as_deref()),
        info,
        animated: animation::is_animated(bytes, format),
    })
}

fn decode_pixels(pixels: &RawPixels, limit: u64) -> Result<DecodedImage, DecodeError> {
    let mismatch = || DecodeError::PixelBufferMismatch {
        width: pixels.width,
        height: pixels.height,
    };
    let expected = pixels
        .width
        .checked_mul(pixels.height)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(mismatch)?;
    if pixels.rgba.len() != expected {
        return Err(mismatch());
    }
    check_decoded_limit(pixels.width, pixels.height, limit)?;

    let mut rgba = pixels.rgba.clone();
    premultiply_in_place(&mut rgba);
    Ok(DecodedImage {
        width: pixels.width,
        height: pixels.height,
        premultiplied_rgba: rgba,
    })
}

pub fn check_decoded_size(width: usize, height: usize) -> Result<(), DecodeError> {
    check_decoded_limit(width, height, limits::decoded())
}

fn check_decoded_limit(width: usize, height: usize, limit: u64) -> Result<(), DecodeError> {
    let decoded_bytes = (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(BYTES_PER_PIXEL);
    if decoded_bytes > limit {
        return Err(DecodeError::TooLarge {
            width,
            height,
            limit_mib: limit / BYTES_PER_MIB,
        });
    }
    Ok(())
}

fn into_decoded_image(image: DynamicImage, icc_profile: Option<&[u8]>) -> DecodedImage {
    let has_alpha = image.color().has_alpha();
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut rgba = image.into_rgba8().into_raw();
    if let Some(profile) = icc_profile {
        convert_to_srgb(profile, &mut rgba);
    }
    if has_alpha {
        premultiply_in_place(&mut rgba);
    }
    rgba.shrink_to_fit();
    DecodedImage {
        width,
        height,
        premultiplied_rgba: rgba,
    }
}

pub fn from_straight_rgba(width: usize, height: usize, mut rgba: Vec<u8>) -> DecodedImage {
    premultiply_in_place(&mut rgba);
    DecodedImage {
        width,
        height,
        premultiplied_rgba: rgba,
    }
}

fn premultiply_in_place(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = scale_by_alpha(*channel, alpha);
        }
    }
}

fn scale_by_alpha(channel: u8, alpha: u32) -> u8 {
    ((u32::from(channel) * alpha + 127) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbaImage};

    fn encoded(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        RgbaImage::new(width, height)
            .write_to(&mut Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }

    #[test]
    fn detects_format_from_magic_bytes() {
        for format in [ImageFormat::Png, ImageFormat::Bmp, ImageFormat::Gif] {
            let image = decode(&Acquired::Encoded(encoded(format, 3, 2)))
                .unwrap()
                .image;
            assert_eq!((image.width, image.height), (3, 2));
        }
    }

    #[test]
    fn decodes_avif_with_alpha() {
        let bytes = include_bytes!("fixtures/alpha_4x2.avif").to_vec();
        let image = decode(&Acquired::Encoded(bytes)).unwrap().image;
        assert_eq!((image.width, image.height), (4, 2));
        let opaque = &image.premultiplied_rgba[..4];
        assert!(opaque[0] > 150 && opaque[1] < 90 && opaque[3] == 255);
        let translucent = &image.premultiplied_rgba[12..16];
        assert!((120..=136).contains(&translucent[3]));
        assert!(translucent[0] < opaque[0]);
    }

    #[test]
    fn rejects_unknown_bytes() {
        let result = decode(&Acquired::Encoded(b"definitely not an image".to_vec()));
        assert!(matches!(result, Err(DecodeError::UnknownFormat)));
        assert!(matches!(
            decode(&Acquired::Encoded(Vec::new())),
            Err(DecodeError::UnknownFormat)
        ));
    }

    #[test]
    fn rejects_truncated_images() {
        let mut bytes = encoded(ImageFormat::Png, 64, 64);
        bytes.truncate(bytes.len() / 2);
        assert!(decode(&Acquired::Encoded(bytes)).is_err());
    }

    #[test]
    fn rejects_oversized_dimensions_before_decoding() {
        let bytes = encoded(ImageFormat::Png, 64, 64);
        let limit = 64 * 64 * BYTES_PER_PIXEL - 1;
        assert!(matches!(
            decode_within(&Acquired::Encoded(bytes.clone()), limit),
            Err(DecodeError::TooLarge { .. })
        ));
        assert!(decode_within(&Acquired::Encoded(bytes), limit + 1).is_ok());
    }

    #[test]
    fn accepts_sides_beyond_a_single_texture() {
        let bytes = encoded(ImageFormat::Png, 9000, 2);
        let image = decode(&Acquired::Encoded(bytes)).unwrap().image;
        assert_eq!((image.width, image.height), (9000, 2));
    }

    #[test]
    fn rejects_oversized_raw_pixels() {
        let pixels = RawPixels {
            width: 2,
            height: 2,
            rgba: vec![0; 16],
        };
        assert!(matches!(
            decode_within(&Acquired::Pixels(pixels), 15),
            Err(DecodeError::TooLarge { .. })
        ));
    }

    fn png_with_profile(profile: &moxcms::ColorProfile) -> Vec<u8> {
        use image::ImageEncoder;
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_icc_profile(profile.encode().unwrap()).unwrap();
        encoder
            .write_image(&[200, 100, 50, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    #[test]
    fn converts_wide_gamut_images_to_srgb() {
        let bytes = png_with_profile(&moxcms::ColorProfile::new_display_p3());
        let image = decode(&Acquired::Encoded(bytes)).unwrap().image;
        assert_ne!(image.premultiplied_rgba[..3], [200, 100, 50]);
        assert_eq!(image.premultiplied_rgba[3], 255);
    }

    #[test]
    fn leaves_srgb_tagged_images_untouched() {
        let bytes = png_with_profile(&moxcms::ColorProfile::new_srgb());
        let image = decode(&Acquired::Encoded(bytes)).unwrap().image;
        assert_eq!(image.premultiplied_rgba, [200, 100, 50, 255]);
    }

    #[test]
    fn collects_exif_and_profile_details_for_the_info_popover() {
        use image::ImageEncoder;
        let tiff = [
            b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 0x01, 0x0F, 0, 2, 0, 0, 0, 2, b'A', 0, 0, 0, 0, 0,
            0, 0,
        ];
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_exif_metadata(tiff.to_vec()).unwrap();
        encoder
            .write_image(&[1, 2, 3, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let info = decode(&Acquired::Encoded(bytes)).unwrap().info;
        assert_eq!(info.format, Some("PNG"));
        assert_eq!(info.metadata.len(), 1);
    }

    #[test]
    fn premultiplies_raw_pixels() {
        let pixels = RawPixels {
            width: 1,
            height: 1,
            rgba: vec![200, 100, 50, 128],
        };
        let image = decode(&Acquired::Pixels(pixels)).unwrap().image;
        assert_eq!(image.premultiplied_rgba, vec![100, 50, 25, 128]);
    }

    #[test]
    fn rejects_mismatched_pixel_buffers() {
        let pixels = RawPixels {
            width: 2,
            height: 2,
            rgba: vec![0; 15],
        };
        assert!(matches!(
            decode(&Acquired::Pixels(pixels)),
            Err(DecodeError::PixelBufferMismatch { .. })
        ));
        let huge = RawPixels {
            width: usize::MAX,
            height: 2,
            rgba: Vec::new(),
        };
        assert!(matches!(
            decode(&Acquired::Pixels(huge)),
            Err(DecodeError::PixelBufferMismatch { .. })
        ));
    }
}
