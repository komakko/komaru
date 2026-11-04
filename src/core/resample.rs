use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

use crate::core::decode::{DecodedImage, check_decoded_size};
use crate::error::ResampleError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    Box,
    Bilinear,
    Hamming,
    CatmullRom,
    Mitchell,
    #[default]
    Lanczos3,
}

impl Filter {
    fn algorithm(self) -> ResizeAlg {
        match self {
            Self::Nearest => ResizeAlg::Nearest,
            Self::Box => ResizeAlg::Convolution(FilterType::Box),
            Self::Bilinear => ResizeAlg::Convolution(FilterType::Bilinear),
            Self::Hamming => ResizeAlg::Convolution(FilterType::Hamming),
            Self::CatmullRom => ResizeAlg::Convolution(FilterType::CatmullRom),
            Self::Mitchell => ResizeAlg::Convolution(FilterType::Mitchell),
            Self::Lanczos3 => ResizeAlg::Convolution(FilterType::Lanczos3),
        }
    }
}

pub fn resample(
    premultiplied: &[u8],
    from: [usize; 2],
    to: [usize; 2],
    filter: Filter,
) -> Result<DecodedImage, ResampleError> {
    let [width, height] = to;
    let (Some(target_width), Some(target_height)) = (side(width), side(height)) else {
        return Err(ResampleError::EmptyTarget { width, height });
    };
    check_decoded_size(width, height)?;
    let source = ImageRef::new(
        from[0] as u32,
        from[1] as u32,
        premultiplied,
        PixelType::U8x4,
    )?;
    let mut target = Image::new(target_width, target_height, PixelType::U8x4);
    let options = ResizeOptions::new()
        .resize_alg(filter.algorithm())
        .use_alpha(false);
    Resizer::new().resize(&source, &mut target, &options)?;
    Ok(DecodedImage {
        width,
        height,
        premultiplied_rgba: target.into_vec(),
    })
}

fn side(length: usize) -> Option<u32> {
    u32::try_from(length).ok().filter(|&side| side > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILTERS: [Filter; 7] = [
        Filter::Nearest,
        Filter::Box,
        Filter::Bilinear,
        Filter::Hamming,
        Filter::CatmullRom,
        Filter::Mitchell,
        Filter::Lanczos3,
    ];

    fn solid(width: usize, height: usize, rgba: [u8; 4]) -> Vec<u8> {
        rgba.repeat(width * height)
    }

    #[test]
    fn the_default_filter_is_lanczos3() {
        assert_eq!(Filter::default(), Filter::Lanczos3);
    }

    #[test]
    fn every_filter_produces_the_requested_size() {
        let source = solid(7, 5, [10, 20, 30, 255]);
        for filter in FILTERS {
            for target in [[3, 2], [7, 5], [20, 11], [1, 1]] {
                let result = resample(&source, [7, 5], target, filter).expect("resize works");
                assert_eq!([result.width, result.height], target, "{filter:?}");
                assert_eq!(result.premultiplied_rgba.len(), target[0] * target[1] * 4);
            }
        }
    }

    #[test]
    fn premultiplied_translucent_colour_is_kept_as_it_is() {
        let pixel = [64, 32, 16, 128];
        let source = solid(8, 8, pixel);
        for filter in FILTERS {
            let result = resample(&source, [8, 8], [3, 5], filter).expect("resize works");
            for resized in result.premultiplied_rgba.chunks_exact(4) {
                for (value, expected) in resized.iter().zip(pixel) {
                    assert!(value.abs_diff(expected) <= 1, "{filter:?}: {resized:?}");
                }
            }
        }
    }

    #[test]
    fn nearest_halving_picks_source_pixels() {
        let mut source = Vec::new();
        for index in 0..16u8 {
            source.extend_from_slice(&[index * 10, 0, 0, 255]);
        }
        let result = resample(&source, [4, 4], [2, 2], Filter::Nearest).expect("resize works");
        assert!(
            result
                .premultiplied_rgba
                .chunks_exact(4)
                .all(|pixel| source.chunks_exact(4).any(|original| original == pixel))
        );
    }

    #[test]
    fn an_empty_target_is_refused() {
        let source = solid(2, 2, [0, 0, 0, 255]);
        let refused = resample(&source, [2, 2], [0, 4], Filter::Lanczos3);
        assert!(matches!(refused, Err(ResampleError::EmptyTarget { .. })));
    }

    #[test]
    fn a_target_beyond_the_decoded_limit_is_refused() {
        let source = solid(2, 2, [0, 0, 0, 255]);
        let refused = resample(&source, [2, 2], [40_000, 40_000], Filter::Lanczos3);
        assert!(matches!(refused, Err(ResampleError::TooLarge(_))));
    }

    #[test]
    fn a_short_source_buffer_is_refused() {
        let refused = resample(&[0; 8], [2, 2], [1, 1], Filter::Lanczos3);
        assert!(matches!(refused, Err(ResampleError::Source(_))));
    }
}
