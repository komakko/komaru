use crate::core::edit::Orientation;
use crate::core::ingest::RawPixels;

const BLOCK: usize = 64;

pub fn straight_oriented(
    premultiplied: &[u8],
    size: [usize; 2],
    orientation: Orientation,
) -> RawPixels {
    let [width, height] = size;
    let [oriented_width, oriented_height] = orientation.size(size);
    let map = orientation.index_map(size);
    let mut rgba = vec![0; oriented_width * oriented_height * 4];
    for block_y in (0..height).step_by(BLOCK) {
        for block_x in (0..width).step_by(BLOCK) {
            for y in block_y..(block_y + BLOCK).min(height) {
                for x in block_x..(block_x + BLOCK).min(width) {
                    let from = (y * width + x) * 4;
                    let to = map.target(x, y) * 4;
                    rgba[to..to + 4].copy_from_slice(&unpremultiplied(&premultiplied[from..]));
                }
            }
        }
    }
    RawPixels {
        width: oriented_width,
        height: oriented_height,
        rgba,
    }
}

fn unpremultiplied(pixel: &[u8]) -> [u8; 4] {
    let alpha = u32::from(pixel[3]);
    match alpha {
        255 => [pixel[0], pixel[1], pixel[2], 255],
        0 => [0, 0, 0, 0],
        _ => {
            let restore =
                |channel: u8| ((u32::from(channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            [
                restore(pixel[0]),
                restore(pixel[1]),
                restore(pixel[2]),
                pixel[3],
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, RgbaImage};

    use super::*;
    use crate::core::edit::Edit;

    fn labelled(width: u32, height: u32) -> DynamicImage {
        let mut image = RgbaImage::new(width, height);
        for (index, pixel) in image.pixels_mut().enumerate() {
            let index = index as u32;
            *pixel = image::Rgba([
                (index % 251) as u8,
                (index / 7 % 251) as u8,
                (index * 3 % 251) as u8,
                255,
            ]);
        }
        DynamicImage::ImageRgba8(image)
    }

    fn reference(image: DynamicImage, edit: Edit) -> DynamicImage {
        match edit {
            Edit::RotateLeft => image.rotate270(),
            Edit::RotateRight => image.rotate90(),
            Edit::FlipHorizontal => image.fliph(),
            Edit::FlipVertical => image.flipv(),
        }
    }

    fn exported(image: &DynamicImage, orientation: Orientation) -> RawPixels {
        let size = [image.width() as usize, image.height() as usize];
        straight_oriented(&image.to_rgba8().into_raw(), size, orientation)
    }

    #[test]
    fn opaque_pixels_land_where_the_image_crate_puts_them() {
        let sequences: [&[Edit]; 6] = [
            &[],
            &[Edit::RotateLeft],
            &[Edit::RotateRight, Edit::FlipHorizontal],
            &[Edit::FlipVertical],
            &[Edit::RotateRight, Edit::RotateRight, Edit::FlipVertical],
            &[Edit::FlipHorizontal, Edit::RotateLeft, Edit::RotateLeft],
        ];
        for (width, height) in [(3, 2), (70, 5), (130, 67)] {
            for sequence in sequences {
                let source = labelled(width, height);
                let mut orientation = Orientation::default();
                let mut expected = source.clone();
                for &edit in sequence {
                    orientation = orientation.after(edit);
                    expected = reference(expected, edit);
                }
                let result = exported(&source, orientation);
                assert_eq!(
                    (result.width as u32, result.height as u32),
                    (expected.width(), expected.height())
                );
                assert_eq!(
                    result.rgba,
                    expected.to_rgba8().into_raw(),
                    "{width}x{height} {sequence:?}"
                );
            }
        }
    }

    #[test]
    fn translucent_pixels_are_restored_to_straight_alpha() {
        let straight = [200u8, 100, 40, 128];
        let premultiplied: Vec<u8> = [200u32, 100, 40]
            .iter()
            .map(|channel| ((channel * 128 + 127) / 255) as u8)
            .chain([128])
            .collect();
        let result = straight_oriented(&premultiplied, [1, 1], Orientation::default());
        for (restored, original) in result.rgba.iter().zip(straight) {
            assert!(restored.abs_diff(original) <= 1, "{restored} vs {original}");
        }
        assert_eq!(result.rgba[3], 128);
    }

    #[test]
    fn fully_transparent_pixels_become_zero_and_opaque_ones_are_untouched() {
        let premultiplied = [0, 0, 0, 0, 12, 34, 56, 255];
        let result = straight_oriented(&premultiplied, [2, 1], Orientation::default());
        assert_eq!(result.rgba, [0, 0, 0, 0, 12, 34, 56, 255]);
    }
}
