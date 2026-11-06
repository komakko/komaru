use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::num::NonZeroU32;
use std::time::Duration;

use color_quant::NeuQuant;
use gif::{DisposalMethod, Encoder, Frame, Repeat};

use crate::core::ingest::RawPixels;
use crate::error::EncodeError;

pub const MIN_COLORS: u16 = 4;
pub const MAX_COLORS: u16 = 256;
const SAMPLING: i32 = 10;
const OPAQUE_FROM: u8 = 128;
const CENTISECOND_MS: u128 = 10;
const ERROR_DENOMINATOR: i32 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GifOptions {
    pub colors: u16,
    pub dither: bool,
    pub compress: bool,
}

impl Default for GifOptions {
    fn default() -> Self {
        Self {
            colors: MAX_COLORS,
            dither: true,
            compress: true,
        }
    }
}

#[derive(Clone, Copy)]
struct Area {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
}

enum Quantiser {
    Exact(HashMap<u32, u8>),
    Network(NeuQuant),
}

struct Session<W: Write> {
    encoder: Encoder<W>,
    width: usize,
    height: usize,
    options: GifOptions,
    shown: Option<Vec<u8>>,
}

pub fn encode(
    mut frames: impl Iterator<Item = (RawPixels, Duration)>,
    plays: Option<NonZeroU32>,
    options: GifOptions,
) -> Result<Vec<u8>, EncodeError> {
    let (first, first_delay) = frames
        .next()
        .ok_or(EncodeError::Backend("there are no frames to encode"))?;
    let mut bytes = Vec::new();
    let mut session = Session::start(&mut bytes, &first, plays, options)?;
    session.add(first, first_delay)?;
    for (image, delay) in frames {
        session.add(image, delay)?;
    }
    drop(session);
    Ok(bytes)
}

impl<W: Write> Session<W> {
    fn start(
        sink: W,
        first: &RawPixels,
        plays: Option<NonZeroU32>,
        options: GifOptions,
    ) -> Result<Self, EncodeError> {
        let side = |length: usize| {
            u16::try_from(length)
                .map_err(|_| EncodeError::Backend("GIF images are limited to 65535 pixels a side"))
        };
        let mut encoder = Encoder::new(sink, side(first.width)?, side(first.height)?, &[])?;
        match plays {
            None => encoder.set_repeat(Repeat::Infinite)?,
            Some(times) if times.get() > 1 => {
                let more = (times.get() - 1).min(u32::from(u16::MAX)) as u16;
                encoder.set_repeat(Repeat::Finite(more))?;
            }
            Some(_) => {}
        }
        Ok(Self {
            encoder,
            width: first.width,
            height: first.height,
            options,
            shown: None,
        })
    }

    fn add(&mut self, image: RawPixels, delay: Duration) -> Result<(), EncodeError> {
        if (image.width, image.height) != (self.width, self.height) {
            return Err(EncodeError::Backend("frames differ in size"));
        }
        let translucent = image
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[3] < OPAQUE_FROM);
        let previous = if self.options.compress && !translucent {
            self.shown.take()
        } else {
            None
        };
        let area = changed_area(previous.as_deref(), &image.rgba, self.width, self.height);
        let (region, hidden) = cut_region(previous.as_deref(), &image.rgba, self.width, area);
        let uses_transparent = translucent || previous.is_some();
        let limit = if self.options.compress {
            usize::from(self.options.colors).clamp(usize::from(MIN_COLORS), usize::from(MAX_COLORS))
        } else {
            usize::from(MAX_COLORS)
        };
        let slots = limit - usize::from(uses_transparent);
        let visible = visible_pixels(&region, &hidden);
        let quantiser = Quantiser::build(&visible, slots);
        let mut palette = quantiser.palette();
        let transparent_index = (palette.len() / 3) as u8;
        if uses_transparent {
            palette.extend([0, 0, 0]);
        }
        let dither = self.options.compress && self.options.dither;
        let indices = quantiser.indices(&region, &hidden, area.width, transparent_index, dither);
        let frame = Frame {
            width: area.width as u16,
            height: area.height as u16,
            left: area.left as u16,
            top: area.top as u16,
            delay: centiseconds(delay),
            dispose: if translucent {
                DisposalMethod::Background
            } else {
                DisposalMethod::Keep
            },
            transparent: uses_transparent.then_some(transparent_index),
            palette: Some(palette),
            buffer: Cow::Owned(indices),
            ..Frame::default()
        };
        self.encoder.write_frame(&frame)?;
        self.shown = (self.options.compress && !translucent).then_some(image.rgba);
        Ok(())
    }
}

fn centiseconds(delay: Duration) -> u16 {
    ((delay.as_millis() + CENTISECOND_MS / 2) / CENTISECOND_MS).min(u128::from(u16::MAX)) as u16
}

fn changed_area(previous: Option<&[u8]>, current: &[u8], width: usize, height: usize) -> Area {
    let whole = Area {
        left: 0,
        top: 0,
        width,
        height,
    };
    let Some(previous) = previous else {
        return whole;
    };
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (at, (old, new)) in previous
        .chunks_exact(4)
        .zip(current.chunks_exact(4))
        .enumerate()
    {
        if old != new {
            let (x, y) = (at % width, at / width);
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if right == 0 {
        return Area {
            width: 1,
            height: 1,
            ..whole
        };
    }
    Area {
        left,
        top,
        width: right - left,
        height: bottom - top,
    }
}

fn cut_region(
    previous: Option<&[u8]>,
    current: &[u8],
    width: usize,
    area: Area,
) -> (Vec<u8>, Vec<bool>) {
    let mut region = Vec::with_capacity(area.width * area.height * 4);
    let mut hidden = Vec::with_capacity(area.width * area.height);
    for y in area.top..area.top + area.height {
        let start = (y * width + area.left) * 4;
        let end = start + area.width * 4;
        region.extend_from_slice(&current[start..end]);
        for (offset, pixel) in current[start..end].chunks_exact(4).enumerate() {
            let unchanged = previous
                .is_some_and(|old| old[start + offset * 4..start + offset * 4 + 4] == *pixel);
            hidden.push(unchanged || pixel[3] < OPAQUE_FROM);
        }
    }
    (region, hidden)
}

fn visible_pixels(region: &[u8], hidden: &[bool]) -> Vec<u8> {
    let mut visible = Vec::new();
    for (pixel, _) in region
        .chunks_exact(4)
        .zip(hidden)
        .filter(|(_, hidden)| !**hidden)
    {
        visible.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    visible
}

fn packed(pixel: &[u8]) -> u32 {
    u32::from_be_bytes([0, pixel[0], pixel[1], pixel[2]])
}

impl Quantiser {
    fn build(visible: &[u8], slots: usize) -> Self {
        Self::exact(visible, slots).unwrap_or_else(|| {
            let colors = slots.max(usize::from(MIN_COLORS));
            Self::Network(NeuQuant::new(SAMPLING, colors, visible))
        })
    }

    fn exact(visible: &[u8], slots: usize) -> Option<Self> {
        let mut seen: HashMap<u32, u8> = HashMap::new();
        for pixel in visible.chunks_exact(4) {
            let next = seen.len();
            seen.entry(packed(pixel)).or_insert(next as u8);
            if seen.len() > slots {
                return None;
            }
        }
        Some(Self::Exact(seen))
    }

    fn palette(&self) -> Vec<u8> {
        match self {
            Self::Exact(seen) => {
                let mut palette = vec![0; seen.len().max(1) * 3];
                for (&colour, &index) in seen {
                    let [_, red, green, blue] = colour.to_be_bytes();
                    let at = usize::from(index) * 3;
                    palette[at..at + 3].copy_from_slice(&[red, green, blue]);
                }
                palette
            }
            Self::Network(network) => network.color_map_rgb(),
        }
    }

    fn indices(
        &self,
        region: &[u8],
        hidden: &[bool],
        width: usize,
        transparent: u8,
        dither: bool,
    ) -> Vec<u8> {
        match self {
            Self::Exact(seen) => region
                .chunks_exact(4)
                .zip(hidden)
                .map(|(pixel, &hidden)| {
                    if hidden {
                        transparent
                    } else {
                        seen.get(&packed(pixel)).copied().unwrap_or(0)
                    }
                })
                .collect(),
            Self::Network(network) if dither => {
                dithered(network, region, hidden, width, transparent)
            }
            Self::Network(network) => region
                .chunks_exact(4)
                .zip(hidden)
                .map(|(pixel, &hidden)| {
                    if hidden {
                        transparent
                    } else {
                        network.index_of(&[pixel[0], pixel[1], pixel[2], 255]) as u8
                    }
                })
                .collect(),
        }
    }
}

fn dithered(
    network: &NeuQuant,
    region: &[u8],
    hidden: &[bool],
    width: usize,
    transparent: u8,
) -> Vec<u8> {
    let mut indices = vec![transparent; hidden.len()];
    let mut current = vec![[0i32; 3]; width + 2];
    let mut below = vec![[0i32; 3]; width + 2];
    for (row, flags) in hidden.chunks_exact(width).enumerate() {
        below.fill([0; 3]);
        for (x, &hidden) in flags.iter().enumerate() {
            if hidden {
                continue;
            }
            let at = row * width + x;
            let pixel = &region[at * 4..at * 4 + 3];
            let slot = x + 1;
            let wanted: [i32; 3] = std::array::from_fn(|c| {
                (i32::from(pixel[c]) + current[slot][c] / ERROR_DENOMINATOR).clamp(0, 255)
            });
            let index = network.index_of(&[wanted[0] as u8, wanted[1] as u8, wanted[2] as u8, 255]);
            indices[at] = index as u8;
            let chosen = network.lookup(index).unwrap_or([0, 0, 0, 255]);
            for c in 0..3 {
                let error = wanted[c] - i32::from(chosen[c]);
                current[slot + 1][c] += error * 7;
                below[slot - 1][c] += error * 3;
                below[slot][c] += error * 5;
                below[slot + 1][c] += error;
            }
        }
        std::mem::swap(&mut current, &mut below);
    }
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::animation::decode_frames;

    fn frame(width: usize, height: usize, pixel: impl Fn(usize, usize) -> [u8; 4]) -> RawPixels {
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                rgba.extend(pixel(x, y));
            }
        }
        RawPixels {
            width,
            height,
            rgba,
        }
    }

    fn flat(shade: u8) -> impl Fn(usize, usize) -> [u8; 4] {
        move |x, y| [shade, (x * 20) as u8, (y * 30) as u8, 255]
    }

    fn timed(images: Vec<RawPixels>) -> impl Iterator<Item = (RawPixels, Duration)> {
        images
            .into_iter()
            .map(|image| (image, Duration::from_millis(120)))
    }

    fn decoded(bytes: &[u8]) -> Vec<Vec<u8>> {
        decode_frames(bytes, u64::MAX, || true)
            .expect("decodes")
            .expect("is current")
            .frames
            .into_iter()
            .map(|frame| frame.image.premultiplied_rgba)
            .collect()
    }

    fn square(size: usize) -> Vec<RawPixels> {
        let base = frame(size, size, flat(10));
        let mut changed = frame(size, size, flat(10));
        changed.rgba[(size + 1) * 4] = 200;
        vec![base, changed.clone_pixels(), changed]
    }

    impl RawPixels {
        fn clone_pixels(&self) -> RawPixels {
            RawPixels {
                width: self.width,
                height: self.height,
                rgba: self.rgba.clone(),
            }
        }
    }

    #[test]
    fn frames_that_fit_the_palette_survive_exactly_with_or_without_compression() {
        for compress in [true, false] {
            let options = GifOptions {
                compress,
                ..GifOptions::default()
            };
            let images = square(8);
            let expected: Vec<Vec<u8>> = images.iter().map(|image| image.rgba.clone()).collect();
            let bytes = encode(timed(images), None, options).expect("encodes");
            assert_eq!(decoded(&bytes), expected, "compress {compress}");
        }
    }

    #[test]
    fn compression_stores_only_what_changed() {
        let optimised = encode(timed(square(24)), None, GifOptions::default()).expect("encodes");
        let whole = encode(
            timed(square(24)),
            None,
            GifOptions {
                compress: false,
                ..GifOptions::default()
            },
        )
        .expect("encodes");
        assert!(
            optimised.len() < whole.len(),
            "{} {}",
            optimised.len(),
            whole.len()
        );
    }

    #[test]
    fn transparent_pixels_stay_transparent() {
        let make = |shade: u8| {
            frame(6, 6, move |x, y| {
                if (x + y) % 2 == 0 {
                    [0, 0, 0, 0]
                } else {
                    [shade, 40, 90, 255]
                }
            })
        };
        let images = vec![make(10), make(200), make(60)];
        let alphas: Vec<Vec<u8>> = images
            .iter()
            .map(|image| image.rgba.chunks_exact(4).map(|pixel| pixel[3]).collect())
            .collect();
        let bytes = encode(timed(images), None, GifOptions::default()).expect("encodes");
        let seen: Vec<Vec<u8>> = decoded(&bytes)
            .iter()
            .map(|pixels| pixels.chunks_exact(4).map(|pixel| pixel[3]).collect())
            .collect();
        assert_eq!(seen, alphas);
    }

    #[test]
    fn busy_frames_are_reduced_to_the_palette_size() {
        let gradient = frame(64, 64, |x, y| {
            [(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, 255]
        });
        for (colors, dither) in [(16, true), (16, false), (4, true), (256, true)] {
            let options = GifOptions {
                colors,
                dither,
                compress: true,
            };
            let bytes =
                encode(timed(vec![gradient.clone_pixels()]), None, options).expect("encodes");
            let pixels = &decoded(&bytes)[0];
            let mut distinct: Vec<[u8; 3]> = pixels
                .chunks_exact(4)
                .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                .collect();
            distinct.sort_unstable();
            distinct.dedup();
            assert!(distinct.len() <= usize::from(colors), "{colors} {dither}");
        }
    }

    #[test]
    fn plays_and_delays_are_written() {
        let bytes =
            encode(timed(square(6)), NonZeroU32::new(3), GifOptions::default()).expect("encodes");
        let animation = decode_frames(&bytes, u64::MAX, || true).unwrap().unwrap();
        assert_eq!(animation.frames.len(), 3);
        assert_eq!(animation.frames[0].delay, Duration::from_millis(120));
        assert_eq!(animation.plays, NonZeroU32::new(2));
    }
}
