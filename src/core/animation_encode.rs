use std::mem::MaybeUninit;
use std::num::NonZeroU32;
use std::time::Duration;

use libwebp_sys::{
    WebPAnimEncoder, WebPAnimEncoderAdd, WebPAnimEncoderAssemble, WebPAnimEncoderDelete,
    WebPAnimEncoderNewInternal, WebPAnimEncoderOptions, WebPAnimEncoderOptionsInitInternal,
    WebPData, WebPDataClear, WebPGetMuxABIVersion, WebPPicture, WebPPictureFree,
    WebPPictureImportRGBA,
};

use crate::core::encode::{EncodeOptions, Format, PngLevel, WebpMode};
use crate::core::gif_encode;
use crate::core::ingest::RawPixels;
use crate::error::EncodeError;

const NO_FRAMES: &str = "there are no frames to encode";
const NOT_ANIMATED: &str = "this format cannot hold an animation";
const WEBP_FAILURE: &str = "animated WebP encoder failed";
const MILLISECONDS: u16 = 1000;

pub fn encode_animation(
    frames: impl ExactSizeIterator<Item = (RawPixels, Duration)>,
    plays: Option<NonZeroU32>,
    format: Format,
    options: EncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    match format {
        Format::Gif => gif_encode::encode(frames, plays, options.gif),
        Format::Png => apng(frames, plays, options.png),
        Format::WebP => animated_webp(frames, plays, options),
        Format::Jpeg | Format::Avif | Format::Bmp => Err(EncodeError::Backend(NOT_ANIMATED)),
    }
}

fn delay_ms(delay: Duration) -> u16 {
    delay.as_millis().min(u128::from(u16::MAX)) as u16
}

fn apng(
    mut frames: impl ExactSizeIterator<Item = (RawPixels, Duration)>,
    plays: Option<NonZeroU32>,
    level: PngLevel,
) -> Result<Vec<u8>, EncodeError> {
    let count = frames.len() as u32;
    let first = frames.next().ok_or(EncodeError::Backend(NO_FRAMES))?;
    let mut bytes = Vec::new();
    {
        let mut encoder =
            png::Encoder::new(&mut bytes, first.0.width as u32, first.0.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(level.animated_compression());
        encoder.set_filter(png::Filter::Adaptive);
        encoder.set_animated(count, plays.map_or(0, NonZeroU32::get))?;
        let mut writer = encoder.write_header()?;
        for (image, delay) in std::iter::once(first).chain(frames) {
            writer.set_frame_delay(delay_ms(delay), MILLISECONDS)?;
            writer.write_image_data(&image.rgba)?;
        }
        writer.finish()?;
    }
    Ok(bytes)
}

fn animated_webp(
    frames: impl Iterator<Item = (RawPixels, Duration)>,
    plays: Option<NonZeroU32>,
    options: EncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    let frames: Vec<(RawPixels, Duration)> = frames.collect();
    if frames.is_empty() {
        return Err(EncodeError::Backend(NO_FRAMES));
    }
    let mut config = webp::WebPConfig::new().map_err(|()| EncodeError::Backend(WEBP_FAILURE))?;
    match options.webp {
        WebpMode::Lossless => config.lossless = 1,
        WebpMode::Lossy => {
            config.lossless = 0;
            config.alpha_compression = 1;
            config.quality = f32::from(options.lossy_quality);
        }
    }
    let loop_count = plays.map_or(0, |times| i32::try_from(times.get()).unwrap_or(i32::MAX));
    unsafe { assemble_webp(&frames, loop_count, &config) }.ok_or(EncodeError::Backend(WEBP_FAILURE))
}

unsafe fn assemble_webp(
    frames: &[(RawPixels, Duration)],
    loop_count: i32,
    config: &webp::WebPConfig,
) -> Option<Vec<u8>> {
    let version = WebPGetMuxABIVersion();
    let mut options = MaybeUninit::<WebPAnimEncoderOptions>::uninit();
    if unsafe { WebPAnimEncoderOptionsInitInternal(options.as_mut_ptr(), version) } == 0 {
        return None;
    }
    let mut options = unsafe { options.assume_init() };
    options.anim_params.loop_count = loop_count;
    let first = &frames.first()?.0;
    let encoder = unsafe {
        WebPAnimEncoderNewInternal(first.width as i32, first.height as i32, &options, version)
    };
    if encoder.is_null() {
        return None;
    }
    let assembled = unsafe { add_and_assemble(encoder, frames, config) };
    unsafe { WebPAnimEncoderDelete(encoder) };
    assembled
}

unsafe fn add_and_assemble(
    encoder: *mut WebPAnimEncoder,
    frames: &[(RawPixels, Duration)],
    config: &webp::WebPConfig,
) -> Option<Vec<u8>> {
    let mut timestamp = 0i32;
    for (image, delay) in frames {
        let mut picture = WebPPicture::new().ok()?;
        picture.use_argb = 1;
        picture.width = image.width as i32;
        picture.height = image.height as i32;
        let imported = unsafe {
            WebPPictureImportRGBA(&mut picture, image.rgba.as_ptr(), image.width as i32 * 4)
        };
        let added = imported != 0
            && unsafe { WebPAnimEncoderAdd(encoder, &mut picture, timestamp, config) } != 0;
        unsafe { WebPPictureFree(&mut picture) };
        if !added {
            return None;
        }
        timestamp = timestamp.saturating_add(i32::from(delay_ms(*delay)));
    }
    if unsafe { WebPAnimEncoderAdd(encoder, std::ptr::null_mut(), timestamp, std::ptr::null()) }
        == 0
    {
        return None;
    }
    let mut data = WebPData {
        bytes: std::ptr::null(),
        size: 0,
    };
    if unsafe { WebPAnimEncoderAssemble(encoder, &mut data) } == 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data.bytes, data.size) }.to_vec();
    unsafe { WebPDataClear(&mut data) };
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::animation::{decode_frames, is_animated};

    fn images() -> Vec<(RawPixels, Duration)> {
        (0..3u8)
            .map(|step| {
                let mut rgba = Vec::new();
                for index in 0..40usize {
                    let (x, y) = (index % 8, index / 8);
                    rgba.extend([step * 70, (x * 30) as u8, (y * 40) as u8, 255]);
                }
                (
                    RawPixels {
                        width: 8,
                        height: 5,
                        rgba,
                    },
                    Duration::from_millis(100 + u64::from(step) * 50),
                )
            })
            .collect()
    }

    fn expected() -> Vec<Vec<u8>> {
        images().into_iter().map(|(image, _)| image.rgba).collect()
    }

    fn round_trip(
        format: Format,
        plays: Option<NonZeroU32>,
    ) -> (Vec<u8>, crate::core::animation::Animation) {
        let bytes = encode_animation(
            images().into_iter(),
            plays,
            format,
            EncodeOptions::default(),
        )
        .expect("encodes");
        let animation = decode_frames(&bytes, u64::MAX, || true)
            .expect("decodes")
            .expect("is current");
        (bytes, animation)
    }

    fn delays(animation: &crate::core::animation::Animation) -> Vec<u64> {
        animation
            .frames
            .iter()
            .map(|frame| frame.delay.as_millis() as u64)
            .collect()
    }

    #[test]
    fn apng_keeps_frames_delays_and_plays() {
        let (bytes, animation) = round_trip(Format::Png, NonZeroU32::new(4));
        assert!(is_animated(&bytes, image::ImageFormat::Png));
        let pixels: Vec<_> = animation
            .frames
            .iter()
            .map(|frame| frame.image.premultiplied_rgba.clone())
            .collect();
        assert_eq!(pixels, expected());
        assert_eq!(delays(&animation), [100, 150, 200]);
        assert_eq!(animation.plays, NonZeroU32::new(4));
    }

    #[test]
    fn animated_webp_keeps_frames_delays_and_plays() {
        let (bytes, animation) = round_trip(Format::WebP, None);
        assert!(is_animated(&bytes, image::ImageFormat::WebP));
        let decoded = webp::AnimDecoder::new(&bytes)
            .decode()
            .expect("libwebp decodes");
        let pixels: Vec<Vec<u8>> = decoded
            .into_iter()
            .map(|frame| frame.get_image().to_vec())
            .collect();
        assert_eq!(pixels, expected());
        assert_eq!(delays(&animation), [100, 150, 200]);
        assert_eq!(animation.plays, None);
    }

    #[test]
    fn gif_dispatch_writes_every_frame() {
        let (bytes, animation) = round_trip(Format::Gif, None);
        assert!(is_animated(&bytes, image::ImageFormat::Gif));
        assert_eq!(animation.frames.len(), 3);
    }

    #[test]
    fn stills_only_formats_are_refused() {
        for format in [Format::Jpeg, Format::Avif, Format::Bmp] {
            assert!(
                encode_animation(images().into_iter(), None, format, EncodeOptions::default())
                    .is_err()
            );
        }
    }
}
