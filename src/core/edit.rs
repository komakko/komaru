use crate::core::crop::PixelRect;
use crate::core::resample::Filter;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edit {
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Orientation {
    matrix: [[i8; 2]; 2],
}

impl Default for Orientation {
    fn default() -> Self {
        Self {
            matrix: [[1, 0], [0, 1]],
        }
    }
}

impl Orientation {
    pub fn after(self, edit: Edit) -> Self {
        let step = match edit {
            Edit::RotateLeft => [[0, 1], [-1, 0]],
            Edit::RotateRight => [[0, -1], [1, 0]],
            Edit::FlipHorizontal => [[-1, 0], [0, 1]],
            Edit::FlipVertical => [[1, 0], [0, -1]],
        };
        Self {
            matrix: multiply(step, self.matrix),
        }
    }

    pub fn inverse(self) -> Self {
        let [[a, b], [c, d]] = self.matrix;
        Self {
            matrix: [[a, c], [b, d]],
        }
    }

    pub fn size(self, source: [usize; 2]) -> [usize; 2] {
        if self.swaps_axes() {
            [source[1], source[0]]
        } else {
            source
        }
    }

    pub fn map(self, source_size: [f32; 2], point: [f32; 2]) -> [f32; 2] {
        let target_size = if self.swaps_axes() {
            [source_size[1], source_size[0]]
        } else {
            source_size
        };
        let x = point[0] - source_size[0] / 2.0;
        let y = point[1] - source_size[1] / 2.0;
        let [[a, b], [c, d]] = self.matrix.map(|row| row.map(f32::from));
        [
            a * x + b * y + target_size[0] / 2.0,
            c * x + d * y + target_size[1] / 2.0,
        ]
    }

    pub fn source_rect(self, source: [usize; 2], oriented: PixelRect) -> Option<PixelRect> {
        let [width, height] = self.size(source).map(|side| side as isize);
        let [source_width, source_height] = source.map(|side| side as isize);
        let [[a, b], [c, d]] = self.inverse().matrix.map(|row| row.map(isize::from));
        let corner = |[x, y]: [usize; 2]| {
            let (x, y) = (2 * x as isize - width, 2 * y as isize - height);
            [
                (a * x + b * y + source_width) / 2,
                (c * x + d * y + source_height) / 2,
            ]
        };
        let (first, second) = (corner(oriented.min()), corner(oriented.max()));
        PixelRect::new(
            [first[0].min(second[0]), first[1].min(second[1])].map(|side| side as usize),
            [first[0].max(second[0]), first[1].max(second[1])].map(|side| side as usize),
        )
    }

    pub fn source_index(self, source: [usize; 2], oriented: [usize; 2]) -> usize {
        self.inverse()
            .index_map(self.size(source))
            .target(oriented[0], oriented[1])
    }

    pub fn index_map(self, source: [usize; 2]) -> IndexMap {
        let [source_width, source_height] = source.map(|side| side as isize);
        let [width, height] = self.size(source).map(|side| side as isize);
        let [[a, b], [c, d]] = self.matrix.map(|row| row.map(isize::from));
        let column_offset = (a * (1 - source_width) + b * (1 - source_height) + width - 1) / 2;
        let row_offset = (c * (1 - source_width) + d * (1 - source_height) + height - 1) / 2;
        IndexMap {
            start: row_offset * width + column_offset,
            step_x: c * width + a,
            step_y: d * width + b,
        }
    }

    fn swaps_axes(self) -> bool {
        self.matrix[0][0] == 0
    }
}

#[derive(Clone, Copy)]
pub struct IndexMap {
    start: isize,
    step_x: isize,
    step_y: isize,
}

impl IndexMap {
    pub fn target(self, x: usize, y: usize) -> usize {
        (self.start + x as isize * self.step_x + y as isize * self.step_y) as usize
    }
}

fn multiply(left: [[i8; 2]; 2], right: [[i8; 2]; 2]) -> [[i8; 2]; 2] {
    let cell = |row: usize, column: usize| {
        left[row][0] * right[0][column] + left[row][1] * right[1][column]
    };
    [[cell(0, 0), cell(0, 1)], [cell(1, 0), cell(1, 1)]]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Orient(Edit),
    Resize { size: [usize; 2], filter: Filter },
    Crop { rect: PixelRect },
}

impl Step {
    pub fn changes_pixels(self) -> bool {
        !matches!(self, Self::Orient(_))
    }
}

#[derive(Default)]
pub struct EditLog {
    steps: Vec<Step>,
    applied: usize,
    orientation: Orientation,
}

impl EditLog {
    pub fn push(&mut self, edit: Edit) {
        self.record(Step::Orient(edit));
    }

    pub fn record_resize(&mut self, size: [usize; 2], filter: Filter) {
        self.record(Step::Resize { size, filter });
    }

    pub fn record_crop(&mut self, rect: PixelRect) {
        self.record(Step::Crop { rect });
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    pub fn is_modified(&self) -> bool {
        self.applied > 0
    }

    pub fn can_undo(&self) -> bool {
        self.applied > 0
    }

    pub fn can_redo(&self) -> bool {
        self.applied < self.steps.len()
    }

    pub fn undo_target(&self) -> Option<usize> {
        self.applied.checked_sub(1)
    }

    pub fn redo_target(&self) -> Option<usize> {
        self.can_redo().then_some(self.applied + 1)
    }

    pub fn steps_before(&self, target: usize) -> &[Step] {
        &self.steps[..target.min(self.steps.len())]
    }

    pub fn pixels_change_at(&self, target: usize) -> bool {
        let (low, high) = (self.applied.min(target), self.applied.max(target));
        self.steps_before(high)[low..]
            .iter()
            .any(|step| step.changes_pixels())
    }

    pub fn seek(&mut self, target: usize) {
        self.applied = target.min(self.steps.len());
        self.orientation = orientation_after(self.steps_before(self.applied));
    }

    fn record(&mut self, step: Step) {
        self.steps.truncate(self.applied);
        self.steps.push(step);
        self.seek(self.steps.len());
    }
}

fn orientation_after(steps: &[Step]) -> Orientation {
    steps
        .iter()
        .fold(Orientation::default(), |orientation, step| match *step {
            Step::Orient(edit) => orientation.after(edit),
            Step::Resize { .. } | Step::Crop { .. } => orientation,
        })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use image::{DynamicImage, GenericImageView, RgbaImage};

    use super::*;

    const ALL: [Edit; 4] = [
        Edit::RotateLeft,
        Edit::RotateRight,
        Edit::FlipHorizontal,
        Edit::FlipVertical,
    ];

    fn labelled() -> DynamicImage {
        let mut image = RgbaImage::new(3, 2);
        for (index, pixel) in image.pixels_mut().enumerate() {
            *pixel = image::Rgba([index as u8 * 40 + 10, 0, 0, 255]);
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

    fn assert_matches_reference(orientation: Orientation, expected: &DynamicImage) {
        let source = labelled();
        let (width, height) = source.dimensions();
        let [oriented_width, oriented_height] = orientation.size([width as usize, height as usize]);
        assert_eq!(
            expected.dimensions(),
            (oriented_width as u32, oriented_height as u32)
        );
        for y in 0..height {
            for x in 0..width {
                let centre = [x as f32 + 0.5, y as f32 + 0.5];
                let [ox, oy] = orientation.map([width as f32, height as f32], centre);
                assert_eq!(
                    expected.get_pixel(ox as u32, oy as u32),
                    source.get_pixel(x, y),
                    "source pixel ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn each_edit_matches_the_image_crate() {
        for edit in ALL {
            let orientation = Orientation::default().after(edit);
            assert_matches_reference(orientation, &reference(labelled(), edit));
        }
    }

    #[test]
    fn sequences_match_applying_the_edits_one_after_another() {
        let sequences = [
            [Edit::RotateRight, Edit::FlipHorizontal, Edit::RotateLeft],
            [Edit::FlipVertical, Edit::RotateRight, Edit::RotateRight],
            [Edit::RotateLeft, Edit::FlipVertical, Edit::FlipHorizontal],
        ];
        for sequence in sequences {
            let mut orientation = Orientation::default();
            let mut expected = labelled();
            for edit in sequence {
                orientation = orientation.after(edit);
                expected = reference(expected, edit);
            }
            assert_matches_reference(orientation, &expected);
        }
    }

    #[test]
    fn source_index_inverts_the_forward_index_map() {
        let source = [5, 3];
        let mut orientation = Orientation::default();
        for edit in [
            Edit::RotateRight,
            Edit::FlipHorizontal,
            Edit::RotateLeft,
            Edit::RotateLeft,
            Edit::FlipVertical,
        ] {
            orientation = orientation.after(edit);
            let forward = orientation.index_map(source);
            let [oriented_width, _] = orientation.size(source);
            for y in 0..source[1] {
                for x in 0..source[0] {
                    let target = forward.target(x, y);
                    let oriented = [target % oriented_width, target / oriented_width];
                    assert_eq!(
                        orientation.source_index(source, oriented),
                        y * source[0] + x
                    );
                }
            }
        }
    }

    #[test]
    fn exactly_eight_orientations_are_reachable() {
        let mut seen = HashSet::from([Orientation::default().matrix]);
        let mut frontier = vec![Orientation::default()];
        while let Some(current) = frontier.pop() {
            for edit in ALL {
                let next = current.after(edit);
                if seen.insert(next.matrix) {
                    frontier.push(next);
                }
            }
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn inverse_maps_back_to_the_source() {
        let size = [5.0, 3.0];
        let point = [1.25, 2.5];
        let mut orientation = Orientation::default();
        for edit in [
            Edit::RotateRight,
            Edit::FlipHorizontal,
            Edit::RotateLeft,
            Edit::RotateLeft,
        ] {
            orientation = orientation.after(edit);
            let oriented_size = orientation.size([5, 3]).map(|side| side as f32);
            let back = orientation
                .inverse()
                .map(oriented_size, orientation.map(size, point));
            assert_eq!(back, point);
        }
    }

    #[test]
    fn quarter_turns_swap_the_sides_and_half_turns_do_not() {
        let turned = Orientation::default().after(Edit::RotateRight);
        assert_eq!(turned.size([5, 3]), [3, 5]);
        assert_eq!(turned.after(Edit::RotateRight).size([5, 3]), [5, 3]);
        assert_eq!(
            Orientation::default()
                .after(Edit::FlipVertical)
                .size([5, 3]),
            [5, 3]
        );
    }

    #[test]
    fn four_turns_return_to_the_start() {
        let mut orientation = Orientation::default();
        for _ in 0..4 {
            orientation = orientation.after(Edit::RotateRight);
        }
        assert_eq!(orientation, Orientation::default());
    }

    #[test]
    fn the_log_tracks_orientation_and_modification() {
        let mut log = EditLog::default();
        assert!(!log.is_modified());
        log.push(Edit::RotateRight);
        log.push(Edit::FlipHorizontal);
        assert!(log.is_modified());
        let expected = Orientation::default()
            .after(Edit::RotateRight)
            .after(Edit::FlipHorizontal);
        assert_eq!(log.orientation(), expected);
    }

    #[test]
    fn a_resize_marks_the_log_modified_and_keeps_the_orientation() {
        let mut log = EditLog::default();
        log.push(Edit::RotateLeft);
        let turned = log.orientation();
        log.record_resize([40, 30], Filter::default());
        assert!(log.is_modified());
        assert_eq!(log.orientation(), turned);
        let mut resized_only = EditLog::default();
        resized_only.record_resize([40, 30], Filter::default());
        assert!(resized_only.is_modified());
        assert_eq!(resized_only.orientation(), Orientation::default());
    }

    fn all_orientations() -> Vec<Orientation> {
        let mut seen = vec![Orientation::default()];
        let mut index = 0;
        while index < seen.len() {
            for edit in ALL {
                let next = seen[index].after(edit);
                if !seen.contains(&next) {
                    seen.push(next);
                }
            }
            index += 1;
        }
        seen
    }

    #[test]
    fn the_source_rectangle_holds_exactly_the_pixels_shown_inside_the_oriented_one() {
        let source = [5, 3];
        for orientation in all_orientations() {
            let [width, height] = orientation.size(source);
            for min in [[0, 0], [1, 1], [2, 0]] {
                for max in [
                    [width, height],
                    [width - 1, height],
                    [min[0] + 1, min[1] + 1],
                ] {
                    let Some(oriented) = PixelRect::new(min, max) else {
                        continue;
                    };
                    let found = orientation
                        .source_rect(source, oriented)
                        .expect("a valid rectangle maps to a valid rectangle");
                    for y in 0..source[1] {
                        for x in 0..source[0] {
                            let centre = [x as f32 + 0.5, y as f32 + 0.5];
                            let [ox, oy] = orientation.map([5.0, 3.0], centre);
                            let shown = (min[0] as f32..max[0] as f32).contains(&ox)
                                && (min[1] as f32..max[1] as f32).contains(&oy);
                            let kept = (found.min()[0]..found.max()[0]).contains(&x)
                                && (found.min()[1]..found.max()[1]).contains(&y);
                            assert_eq!(shown, kept, "{orientation:?} {oriented:?} ({x}, {y})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_crop_marks_the_log_modified_and_keeps_the_orientation() {
        let mut log = EditLog::default();
        log.push(Edit::FlipHorizontal);
        let flipped = log.orientation();
        log.record_crop(PixelRect::new([1, 1], [3, 3]).expect("not empty"));
        assert!(log.is_modified());
        assert_eq!(log.orientation(), flipped);
    }

    #[test]
    fn undo_and_redo_move_through_the_log() {
        let mut log = EditLog::default();
        log.push(Edit::RotateRight);
        log.record_resize([4, 3], Filter::default());
        log.push(Edit::FlipHorizontal);
        assert_eq!(log.undo_target(), Some(2));
        log.seek(2);
        assert_eq!(
            log.orientation(),
            Orientation::default().after(Edit::RotateRight)
        );
        assert!(log.can_redo() && log.can_undo());
        assert_eq!(log.redo_target(), Some(3));
        log.seek(0);
        assert!(!log.is_modified() && !log.can_undo());
        assert_eq!(log.orientation(), Orientation::default());
        assert_eq!(log.undo_target(), None);
    }

    #[test]
    fn a_new_step_discards_the_undone_ones() {
        let mut log = EditLog::default();
        log.push(Edit::RotateRight);
        log.push(Edit::FlipVertical);
        log.seek(1);
        log.push(Edit::FlipHorizontal);
        assert!(!log.can_redo());
        assert_eq!(log.steps_before(9).len(), 2);
        assert_eq!(log.steps_before(9)[1], Step::Orient(Edit::FlipHorizontal));
    }

    #[test]
    fn only_pixel_steps_between_the_cursor_and_the_target_force_a_rebuild() {
        let mut log = EditLog::default();
        log.push(Edit::RotateLeft);
        log.record_crop(PixelRect::new([0, 0], [2, 2]).expect("not empty"));
        log.push(Edit::FlipVertical);
        assert!(!log.pixels_change_at(2));
        assert!(log.pixels_change_at(1));
        log.seek(0);
        assert!(!log.pixels_change_at(1));
        assert!(log.pixels_change_at(2));
        assert!(log.pixels_change_at(3));
    }
}
