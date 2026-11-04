use crate::core::crop::crop;
use crate::core::decode::DecodedImage;
use crate::core::edit::Step;
use crate::core::resample::resample;
use crate::error::ReplayError;

pub fn replay(
    source: &[u8],
    size: [usize; 2],
    steps: &[Step],
) -> Result<DecodedImage, ReplayError> {
    let mut current: Option<DecodedImage> = None;
    for &step in steps {
        let (bytes, from) = match &current {
            Some(image) => (
                image.premultiplied_rgba.as_slice(),
                [image.width, image.height],
            ),
            None => (source, size),
        };
        if let Some(next) = apply(bytes, from, step)? {
            current = Some(next);
        }
    }
    Ok(current.unwrap_or_else(|| DecodedImage {
        width: size[0],
        height: size[1],
        premultiplied_rgba: source.to_vec(),
    }))
}

fn apply(
    premultiplied: &[u8],
    from: [usize; 2],
    step: Step,
) -> Result<Option<DecodedImage>, ReplayError> {
    match step {
        Step::Orient(_) => Ok(None),
        Step::Resize { size, filter } => Ok(Some(resample(premultiplied, from, size, filter)?)),
        Step::Crop { rect } => Ok(Some(crop(premultiplied, from, rect)?)),
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::*;
    use crate::core::crop::PixelRect;
    use crate::core::edit::Edit;
    use crate::core::resample::Filter;
    use crate::error::CropError;

    fn labelled() -> RgbaImage {
        RgbaImage::from_fn(8, 6, |x, y| Rgba([x as u8 * 30, y as u8 * 40, 9, 255]))
    }

    fn rect(min: [usize; 2], max: [usize; 2]) -> PixelRect {
        PixelRect::new(min, max).expect("test rectangle is not empty")
    }

    #[test]
    fn no_pixel_steps_return_a_copy_of_the_source() {
        let source = labelled();
        let steps = [
            Step::Orient(Edit::RotateLeft),
            Step::Orient(Edit::FlipVertical),
        ];
        let image = replay(source.as_raw(), [8, 6], &steps).expect("replay works");
        assert_eq!([image.width, image.height], [8, 6]);
        assert_eq!(image.premultiplied_rgba, source.into_raw());
    }

    #[test]
    fn steps_apply_one_after_another_and_skip_orientation() {
        let source = labelled();
        let wanted = rect([1, 1], [5, 4]);
        let steps = [
            Step::Resize {
                size: [4, 3],
                filter: Filter::default(),
            },
            Step::Orient(Edit::RotateRight),
            Step::Crop {
                rect: rect([1, 0], [3, 2]),
            },
        ];
        let resized =
            resample(source.as_raw(), [8, 6], [4, 3], Filter::default()).expect("resample works");
        let expected =
            crop(&resized.premultiplied_rgba, [4, 3], rect([1, 0], [3, 2])).expect("crop works");
        let image = replay(source.as_raw(), [8, 6], &steps).expect("replay works");
        assert_eq!([image.width, image.height], [2, 2]);
        assert_eq!(image.premultiplied_rgba, expected.premultiplied_rgba);
        let cropped_only =
            replay(source.as_raw(), [8, 6], &[Step::Crop { rect: wanted }]).expect("replay works");
        assert_eq!([cropped_only.width, cropped_only.height], wanted.size());
    }

    #[test]
    fn a_step_that_no_longer_fits_is_refused() {
        let source = labelled();
        let steps = [
            Step::Resize {
                size: [4, 3],
                filter: Filter::default(),
            },
            Step::Crop {
                rect: rect([0, 0], [8, 6]),
            },
        ];
        let refused = replay(source.as_raw(), [8, 6], &steps);
        assert!(matches!(
            refused,
            Err(ReplayError::Crop(CropError::OutsideImage { .. }))
        ));
    }
}
