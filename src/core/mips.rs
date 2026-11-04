use crate::core::decode::DecodedImage;

const CHANNELS: usize = 4;
const SMALLEST_LEVEL_SIDE: usize = 64;

fn needs_levels(width: usize, height: usize) -> bool {
    width.max(height) > SMALLEST_LEVEL_SIDE
}

pub fn build_levels_below(
    size: [usize; 2],
    base: &[u8],
    is_current: impl Fn() -> bool,
) -> Option<Vec<DecodedImage>> {
    let mut levels: Vec<DecodedImage> = Vec::new();
    loop {
        let (width, height, source) = levels.last().map_or((size[0], size[1], base), |level| {
            (
                level.width,
                level.height,
                level.premultiplied_rgba.as_slice(),
            )
        });
        if !needs_levels(width, height) {
            return Some(levels);
        }
        if !is_current() {
            return None;
        }
        let next = halve(width, height, source);
        levels.push(next);
    }
}

fn halve(source_width: usize, source_height: usize, source: &[u8]) -> DecodedImage {
    let width = source_width.div_ceil(2);
    let height = source_height.div_ceil(2);
    let stride = source_width * CHANNELS;
    let mut pixels = vec![0u8; width * height * CHANNELS];
    for (y, output_row) in pixels.chunks_exact_mut(width * CHANNELS).enumerate() {
        let top = row(source, stride, 2 * y);
        let bottom = row(source, stride, (2 * y + 1).min(source_height - 1));
        for (x, output) in output_row.chunks_exact_mut(CHANNELS).enumerate() {
            let left = 2 * x * CHANNELS;
            let right = (2 * x + 1).min(source_width - 1) * CHANNELS;
            for (channel, value) in output.iter_mut().enumerate() {
                let sum = u32::from(top[left + channel])
                    + u32::from(top[right + channel])
                    + u32::from(bottom[left + channel])
                    + u32::from(bottom[right + channel]);
                *value = ((sum + 2) / 4) as u8;
            }
        }
    }
    DecodedImage {
        width,
        height,
        premultiplied_rgba: pixels,
    }
}

fn row(pixels: &[u8], stride: usize, y: usize) -> &[u8] {
    &pixels[y * stride..(y + 1) * stride]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: usize, height: usize, rgba: [u8; 4]) -> DecodedImage {
        DecodedImage {
            width,
            height,
            premultiplied_rgba: rgba.repeat(width * height),
        }
    }

    fn levels_below(
        image: &DecodedImage,
        is_current: impl Fn() -> bool,
    ) -> Option<Vec<DecodedImage>> {
        build_levels_below(
            [image.width, image.height],
            &image.premultiplied_rgba,
            is_current,
        )
    }

    fn levels_of(image: DecodedImage) -> Vec<DecodedImage> {
        levels_below(&image, || true).unwrap()
    }

    #[test]
    fn levels_halve_with_rounding_up_until_small() {
        let levels = levels_of(solid(300, 101, [10, 20, 30, 255]));
        let sizes: Vec<_> = levels.iter().map(|l| (l.width, l.height)).collect();
        assert_eq!(sizes, [(150, 51), (75, 26), (38, 13)]);
    }

    #[test]
    fn solid_colour_survives_every_level() {
        let levels = levels_of(solid(257, 129, [200, 100, 50, 255]));
        assert!(!levels.is_empty());
        for level in &levels {
            assert!(
                level
                    .premultiplied_rgba
                    .chunks_exact(4)
                    .all(|p| p == [200, 100, 50, 255])
            );
        }
    }

    #[test]
    fn small_image_needs_no_levels() {
        assert!(levels_of(solid(64, 40, [0; 4])).is_empty());
    }

    #[test]
    fn a_superseded_build_stops_without_levels() {
        assert!(levels_below(&solid(300, 101, [0; 4]), || false).is_none());
    }

    #[test]
    fn a_build_stops_once_it_is_superseded_midway() {
        let remaining = std::cell::Cell::new(1);
        let is_current = || {
            let left = remaining.get();
            remaining.set(left - 1);
            left > 0
        };
        assert!(levels_below(&solid(300, 101, [0; 4]), is_current).is_none());
    }
}
