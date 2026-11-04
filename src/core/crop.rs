use crate::core::decode::DecodedImage;
use crate::error::CropError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    min: [usize; 2],
    max: [usize; 2],
}

impl PixelRect {
    pub fn new(min: [usize; 2], max: [usize; 2]) -> Option<Self> {
        (min[0] < max[0] && min[1] < max[1]).then_some(Self { min, max })
    }

    pub fn min(self) -> [usize; 2] {
        self.min
    }

    pub fn max(self) -> [usize; 2] {
        self.max
    }

    pub fn size(self) -> [usize; 2] {
        [self.max[0] - self.min[0], self.max[1] - self.min[1]]
    }

    pub fn covers(self, size: [usize; 2]) -> bool {
        self.min == [0, 0] && self.max == size
    }

    fn fits_in(self, size: [usize; 2]) -> bool {
        self.max[0] <= size[0] && self.max[1] <= size[1]
    }
}

pub fn crop(
    premultiplied: &[u8],
    from: [usize; 2],
    rect: PixelRect,
) -> Result<DecodedImage, CropError> {
    if !rect.fits_in(from) {
        return Err(CropError::OutsideImage { rect, size: from });
    }
    let row_bytes = from[0]
        .checked_mul(4)
        .ok_or(CropError::SourceMismatch { size: from })?;
    if from[1].checked_mul(row_bytes) != Some(premultiplied.len()) {
        return Err(CropError::SourceMismatch { size: from });
    }
    let [left, top] = rect.min();
    let [width, height] = rect.size();
    let (first, last) = (left * 4, (left + width) * 4);
    let mut cropped = Vec::with_capacity(width * height * 4);
    for row in premultiplied.chunks_exact(row_bytes).skip(top).take(height) {
        cropped.extend_from_slice(&row[first..last]);
    }
    Ok(DecodedImage {
        width,
        height,
        premultiplied_rgba: cropped,
    })
}

#[cfg(test)]
mod tests {
    use image::{GenericImageView, Rgba, RgbaImage};

    use super::*;

    fn labelled(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| Rgba([x as u8, y as u8, 7, 255]))
    }

    fn rect(min: [usize; 2], max: [usize; 2]) -> PixelRect {
        PixelRect::new(min, max).expect("test rectangle is not empty")
    }

    #[test]
    fn an_empty_or_inverted_rectangle_cannot_exist() {
        assert!(PixelRect::new([2, 2], [2, 5]).is_none());
        assert!(PixelRect::new([2, 2], [5, 2]).is_none());
        assert!(PixelRect::new([4, 0], [3, 9]).is_none());
        assert!(PixelRect::new([0, 0], [1, 1]).is_some());
    }

    #[test]
    fn size_and_coverage_follow_the_corners() {
        let inner = rect([1, 2], [4, 7]);
        assert_eq!(inner.size(), [3, 5]);
        assert!(!inner.covers([4, 7]));
        assert!(rect([0, 0], [4, 7]).covers([4, 7]));
        assert!(!rect([0, 0], [4, 6]).covers([4, 7]));
    }

    #[test]
    fn a_crop_matches_the_image_crate() {
        let source = labelled(9, 6);
        let wanted = rect([2, 1], [7, 5]);
        let cropped = crop(source.as_raw(), [9, 6], wanted).expect("crop works");
        let reference = source.view(2, 1, 5, 4).to_image();
        assert_eq!([cropped.width, cropped.height], [5, 4]);
        assert_eq!(cropped.premultiplied_rgba, reference.into_raw());
    }

    #[test]
    fn cropping_the_whole_image_returns_it_unchanged() {
        let source = labelled(4, 3);
        let cropped = crop(source.as_raw(), [4, 3], rect([0, 0], [4, 3])).expect("crop works");
        assert_eq!(cropped.premultiplied_rgba, source.into_raw());
    }

    #[test]
    fn a_single_corner_pixel_can_be_cropped() {
        let source = labelled(4, 3);
        let cropped = crop(source.as_raw(), [4, 3], rect([3, 2], [4, 3])).expect("crop works");
        assert_eq!(cropped.premultiplied_rgba, vec![3, 2, 7, 255]);
    }

    #[test]
    fn a_rectangle_past_the_edge_is_refused() {
        let source = labelled(4, 3);
        let refused = crop(source.as_raw(), [4, 3], rect([1, 1], [5, 3]));
        assert!(matches!(refused, Err(CropError::OutsideImage { .. })));
    }

    #[test]
    fn a_buffer_of_the_wrong_length_is_refused() {
        let refused = crop(&[0; 20], [4, 3], rect([0, 0], [1, 1]));
        assert!(matches!(refused, Err(CropError::SourceMismatch { .. })));
    }
}
