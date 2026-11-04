use std::io::Cursor;

use image::metadata::Orientation;
use mp4parse::{AvifContext, ImageMirror, ImageRotation, ParseStrictness, read_avif};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mirror {
    None,
    LeftRight,
    TopBottom,
}

pub fn avif_orientation(bytes: &[u8]) -> Orientation {
    let Ok(context) = read_avif(&mut Cursor::new(bytes), ParseStrictness::Normal) else {
        return Orientation::NoTransforms;
    };
    let anticlockwise_turns = match context.image_rotation() {
        Ok(ImageRotation::D90) => 1,
        Ok(ImageRotation::D180) => 2,
        Ok(ImageRotation::D270) => 3,
        Ok(ImageRotation::D0) | Err(_) => 0,
    };
    orientation_from(anticlockwise_turns, mirror_of(&context))
}

fn mirror_of(context: &AvifContext) -> Mirror {
    let Ok(pointer) = context.image_mirror_ptr() else {
        return Mirror::None;
    };
    match unsafe { pointer.as_ref() } {
        Some(ImageMirror::LeftRight) => Mirror::LeftRight,
        Some(ImageMirror::TopBottom) => Mirror::TopBottom,
        None => Mirror::None,
    }
}

fn orientation_from(anticlockwise_turns: u8, mirror: Mirror) -> Orientation {
    let mut clockwise_turns = (4 - anticlockwise_turns % 4) % 4;
    let mut flipped = false;
    match mirror {
        Mirror::None => {}
        Mirror::LeftRight => flipped = true,
        Mirror::TopBottom => {
            clockwise_turns = (clockwise_turns + 2) % 4;
            flipped = true;
        }
    }
    match (clockwise_turns, flipped) {
        (0, false) => Orientation::NoTransforms,
        (1, false) => Orientation::Rotate90,
        (2, false) => Orientation::Rotate180,
        (3, false) => Orientation::Rotate270,
        (0, true) => Orientation::FlipHorizontal,
        (1, true) => Orientation::Rotate90FlipH,
        (2, true) => Orientation::FlipVertical,
        _ => Orientation::Rotate270FlipH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};

    fn labelled() -> DynamicImage {
        let mut image = RgbaImage::new(3, 2);
        for (index, pixel) in image.pixels_mut().enumerate() {
            *pixel = image::Rgba([index as u8 * 40 + 10, 0, 0, 255]);
        }
        DynamicImage::ImageRgba8(image)
    }

    fn expected(anticlockwise_turns: u8, mirror: Mirror) -> DynamicImage {
        let mut image = labelled();
        for _ in 0..anticlockwise_turns {
            image = image.rotate270();
        }
        match mirror {
            Mirror::None => image,
            Mirror::LeftRight => image.fliph(),
            Mirror::TopBottom => image.flipv(),
        }
    }

    #[test]
    fn every_rotation_and_mirror_matches_applying_them_in_sequence() {
        for turns in 0..4 {
            for mirror in [Mirror::None, Mirror::LeftRight, Mirror::TopBottom] {
                let mut image = labelled();
                image.apply_orientation(orientation_from(turns, mirror));
                assert_eq!(
                    image.to_rgba8().into_raw(),
                    expected(turns, mirror).to_rgba8().into_raw(),
                    "turns {turns}"
                );
            }
        }
    }

    #[test]
    fn a_quarter_turn_anticlockwise_is_a_three_quarter_turn_clockwise() {
        assert_eq!(orientation_from(1, Mirror::None), Orientation::Rotate270);
    }

    #[test]
    fn files_without_transform_boxes_stay_upright() {
        let bytes = include_bytes!("fixtures/alpha_4x2.avif");
        assert_eq!(avif_orientation(bytes), Orientation::NoTransforms);
    }

    #[test]
    fn unreadable_containers_stay_upright() {
        assert_eq!(avif_orientation(b"not avif"), Orientation::NoTransforms);
    }
}
