use std::io::Cursor;
use std::num::NonZeroU32;
use std::time::Duration;

use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::metadata::LoopCount;
use image::{AnimationDecoder, ImageFormat};

use crate::core::decode::{DecodedImage, from_straight_rgba};
use crate::error::DecodeError;

const BYTES_PER_PIXEL: u64 = 4;
const TOO_FAST: Duration = Duration::from_millis(10);
const SLOWED_TO: Duration = Duration::from_millis(100);

pub struct Frame {
    pub image: DecodedImage,
    pub delay: Duration,
}

pub struct Animation {
    pub frames: Vec<Frame>,
    pub plays: Option<NonZeroU32>,
    pub truncated: bool,
}

pub fn is_animated(bytes: &[u8], format: ImageFormat) -> bool {
    let cursor = Cursor::new(bytes);
    match format {
        ImageFormat::Gif => true,
        ImageFormat::WebP => WebPDecoder::new(cursor).is_ok_and(|decoder| decoder.has_animation()),
        ImageFormat::Png => PngDecoder::new(cursor)
            .and_then(|decoder| decoder.is_apng())
            .unwrap_or(false),
        _ => false,
    }
}

pub fn decode_frames(
    bytes: &[u8],
    budget: u64,
    is_current: impl Fn() -> bool,
) -> Result<Option<Animation>, DecodeError> {
    let format = image::guess_format(bytes).map_err(|_| DecodeError::UnknownFormat)?;
    let cursor = Cursor::new(bytes);
    match format {
        ImageFormat::Gif => collect(GifDecoder::new(cursor)?, budget, is_current),
        ImageFormat::WebP => collect(WebPDecoder::new(cursor)?, budget, is_current),
        ImageFormat::Png => collect(PngDecoder::new(cursor)?.apng()?, budget, is_current),
        _ => Err(DecodeError::UnknownFormat),
    }
}

fn collect<'a>(
    decoder: impl AnimationDecoder<'a>,
    budget: u64,
    is_current: impl Fn() -> bool,
) -> Result<Option<Animation>, DecodeError> {
    let plays = match decoder.loop_count() {
        LoopCount::Infinite => None,
        LoopCount::Finite(times) => Some(times),
    };
    let mut frames: Vec<Frame> = Vec::new();
    let mut used = 0u64;
    let mut truncated = false;
    for decoded in decoder.into_frames() {
        if !is_current() {
            return Ok(None);
        }
        let frame = match decoded {
            Ok(frame) => frame,
            Err(error) if frames.is_empty() => return Err(error.into()),
            Err(_) => {
                truncated = true;
                break;
            }
        };
        let delay = visible_delay(frame.delay().into());
        let buffer = frame.into_buffer();
        let (width, height) = (buffer.width() as usize, buffer.height() as usize);
        used = used.saturating_add((width * height) as u64 * BYTES_PER_PIXEL);
        let same_size = frames
            .first()
            .is_none_or(|first| (first.image.width, first.image.height) == (width, height));
        if used > budget || !same_size {
            truncated = true;
            break;
        }
        frames.push(Frame {
            image: from_straight_rgba(width, height, buffer.into_raw()),
            delay,
        });
    }
    Ok(Some(Animation {
        frames,
        plays,
        truncated,
    }))
}

fn visible_delay(delay: Duration) -> Duration {
    if delay <= TOO_FAST { SLOWED_TO } else { delay }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame as ImageFrame, Rgba, RgbaImage};

    fn gif(frame_delays_ms: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            encoder.set_repeat(Repeat::Infinite).unwrap();
            for (index, &delay) in frame_delays_ms.iter().enumerate() {
                let shade = (index * 40) as u8;
                let buffer = RgbaImage::from_pixel(3, 2, Rgba([shade, 0, 0, 255]));
                let delay = Delay::from_numer_denom_ms(delay, 1);
                encoder
                    .encode_frame(ImageFrame::from_parts(buffer, 0, 0, delay))
                    .unwrap();
            }
        }
        bytes
    }

    #[test]
    fn collects_every_frame_with_browser_like_delays() {
        let bytes = gif(&[200, 0, 50]);
        assert!(is_animated(&bytes, ImageFormat::Gif));
        let animation = decode_frames(&bytes, u64::MAX, || true).unwrap().unwrap();
        assert_eq!(animation.frames.len(), 3);
        assert_eq!(animation.plays, None);
        assert!(!animation.truncated);
        let delays: Vec<_> = animation.frames.iter().map(|frame| frame.delay).collect();
        assert_eq!(
            delays,
            [
                Duration::from_millis(200),
                SLOWED_TO,
                Duration::from_millis(50)
            ]
        );
        assert_eq!(animation.frames[2].image.width, 3);
        assert_ne!(
            animation.frames[0].image.premultiplied_rgba,
            animation.frames[2].image.premultiplied_rgba
        );
    }

    #[test]
    fn stops_quietly_when_the_load_is_superseded() {
        let bytes = gif(&[100, 100]);
        assert!(decode_frames(&bytes, u64::MAX, || false).unwrap().is_none());
    }

    #[test]
    fn still_formats_are_not_animations() {
        let mut png = Vec::new();
        RgbaImage::new(2, 2)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        assert!(!is_animated(&png, ImageFormat::Png));
        assert!(!is_animated(b"junk", ImageFormat::Jpeg));
    }

    #[test]
    fn the_decoded_size_limit_truncates_the_animation() {
        let bytes = gif(&[100, 100, 100]);
        let frame_bytes = 3 * 2 * BYTES_PER_PIXEL;
        let animation = decode_frames(&bytes, frame_bytes * 2, || true)
            .unwrap()
            .unwrap();
        assert_eq!(animation.frames.len(), 2);
        assert!(animation.truncated);
    }
}
