use crate::core::crop::PixelRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    fn moves_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::Left | Self::BottomLeft)
    }

    fn moves_right(self) -> bool {
        matches!(self, Self::TopRight | Self::Right | Self::BottomRight)
    }

    fn moves_top(self) -> bool {
        matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
    }

    fn moves_bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::Bottom | Self::BottomRight)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grab {
    Resize(Handle),
    Move,
}

#[derive(Clone, Copy)]
enum Gesture {
    Create {
        anchor: [f32; 2],
    },
    Move {
        origin: PixelRect,
        grabbed: [f32; 2],
    },
    Resize(Handle),
}

pub struct CropDraft {
    size: [usize; 2],
    rect: Option<PixelRect>,
    gesture: Option<Gesture>,
}

impl CropDraft {
    pub fn new(size: [usize; 2]) -> Self {
        Self {
            size,
            rect: None,
            gesture: None,
        }
    }

    pub fn rect(&self) -> Option<PixelRect> {
        self.rect
    }

    pub fn ready(&self) -> Option<PixelRect> {
        self.rect.filter(|rect| !rect.covers(self.size))
    }

    pub fn is_dragging(&self) -> bool {
        self.gesture.is_some()
    }

    pub fn active_grab(&self) -> Option<Grab> {
        match self.gesture? {
            Gesture::Create { .. } => None,
            Gesture::Move { .. } => Some(Grab::Move),
            Gesture::Resize(handle) => Some(Grab::Resize(handle)),
        }
    }

    pub fn begin(&mut self, grab: Option<Grab>, point: [f32; 2]) {
        self.gesture = Some(match (grab, self.rect) {
            (Some(Grab::Resize(handle)), Some(_)) => Gesture::Resize(handle),
            (Some(Grab::Move), Some(origin)) => Gesture::Move {
                origin,
                grabbed: point,
            },
            _ => Gesture::Create { anchor: point },
        });
    }

    pub fn drag(&mut self, point: [f32; 2]) {
        let Some(gesture) = self.gesture else {
            return;
        };
        let updated = match gesture {
            Gesture::Create { anchor } => self.created(anchor, point),
            Gesture::Move { origin, grabbed } => {
                self.moved(origin, [point[0] - grabbed[0], point[1] - grabbed[1]])
            }
            Gesture::Resize(handle) => self.rect.and_then(|rect| self.resized(rect, handle, point)),
        };
        self.rect = updated.or(self.rect);
    }

    pub fn finish(&mut self) {
        self.gesture = None;
    }

    fn created(&self, anchor: [f32; 2], point: [f32; 2]) -> Option<PixelRect> {
        let [width, height] = self.size;
        let (left, right) = span(edge(anchor[0], width), edge(point[0], width), width);
        let (top, bottom) = span(edge(anchor[1], height), edge(point[1], height), height);
        PixelRect::new([left, top], [right, bottom])
    }

    fn moved(&self, origin: PixelRect, delta: [f32; 2]) -> Option<PixelRect> {
        let [width, height] = origin.size();
        let left = shifted(
            origin.min()[0],
            delta[0],
            self.size[0].saturating_sub(width),
        );
        let top = shifted(
            origin.min()[1],
            delta[1],
            self.size[1].saturating_sub(height),
        );
        PixelRect::new([left, top], [left + width, top + height])
    }

    fn resized(&self, rect: PixelRect, handle: Handle, point: [f32; 2]) -> Option<PixelRect> {
        let [mut left, mut top] = rect.min();
        let [mut right, mut bottom] = rect.max();
        if handle.moves_left() {
            left = edge(point[0], right - 1);
        }
        if handle.moves_right() {
            right = edge(point[0], self.size[0]).max(left + 1);
        }
        if handle.moves_top() {
            top = edge(point[1], bottom - 1);
        }
        if handle.moves_bottom() {
            bottom = edge(point[1], self.size[1]).max(top + 1);
        }
        PixelRect::new([left, top], [right, bottom])
    }
}

fn edge(value: f32, limit: usize) -> usize {
    value.round().clamp(0.0, limit as f32) as usize
}

fn shifted(start: usize, delta: f32, room: usize) -> usize {
    (start as f32 + delta.round()).clamp(0.0, room as f32) as usize
}

fn span(first: usize, second: usize, limit: usize) -> (usize, usize) {
    let (low, high) = (first.min(second), first.max(second));
    if high > low {
        (low, high)
    } else if high < limit {
        (low, high + 1)
    } else {
        (low.saturating_sub(1), high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft_with(size: [usize; 2], min: [usize; 2], max: [usize; 2]) -> CropDraft {
        let mut draft = CropDraft::new(size);
        draft.rect = PixelRect::new(min, max);
        draft
    }

    fn bounds(draft: &CropDraft) -> ([usize; 2], [usize; 2]) {
        let rect = draft.rect().expect("a rectangle exists");
        (rect.min(), rect.max())
    }

    fn gesture(draft: &mut CropDraft, grab: Option<Grab>, from: [f32; 2], to: [f32; 2]) {
        draft.begin(grab, from);
        draft.drag(to);
        draft.finish();
    }

    #[test]
    fn a_fresh_draft_has_no_rectangle_and_nothing_to_apply() {
        let draft = CropDraft::new([100, 80]);
        assert!(draft.rect().is_none());
        assert!(draft.ready().is_none());
        assert!(!draft.is_dragging());
    }

    #[test]
    fn dragging_on_empty_space_creates_a_rectangle_in_any_direction() {
        let mut draft = CropDraft::new([100, 80]);
        gesture(&mut draft, None, [10.2, 20.4], [40.6, 50.0]);
        assert_eq!(bounds(&draft), ([10, 20], [41, 50]));
        gesture(&mut draft, None, [60.0, 60.0], [30.0, 10.0]);
        assert_eq!(bounds(&draft), ([30, 10], [60, 60]));
    }

    #[test]
    fn a_new_rectangle_stays_inside_the_image() {
        let mut draft = CropDraft::new([100, 80]);
        gesture(&mut draft, None, [-30.0, 90.0], [250.0, -40.0]);
        assert_eq!(bounds(&draft), ([0, 0], [100, 80]));
        assert!(draft.ready().is_none());
    }

    #[test]
    fn a_new_rectangle_is_at_least_one_pixel_even_without_movement() {
        let mut draft = CropDraft::new([100, 80]);
        gesture(&mut draft, None, [10.1, 10.1], [10.3, 10.2]);
        assert_eq!(bounds(&draft), ([10, 10], [11, 11]));
        gesture(&mut draft, None, [100.0, 80.0], [100.0, 80.0]);
        assert_eq!(bounds(&draft), ([99, 79], [100, 80]));
    }

    #[test]
    fn dragging_a_corner_resizes_both_adjacent_edges() {
        let mut draft = draft_with([100, 80], [20, 20], [60, 50]);
        gesture(
            &mut draft,
            Some(Grab::Resize(Handle::TopLeft)),
            [20.0, 20.0],
            [5.0, 12.0],
        );
        assert_eq!(bounds(&draft), ([5, 12], [60, 50]));
        gesture(
            &mut draft,
            Some(Grab::Resize(Handle::BottomRight)),
            [60.0, 50.0],
            [90.0, 70.0],
        );
        assert_eq!(bounds(&draft), ([5, 12], [90, 70]));
    }

    #[test]
    fn dragging_an_edge_moves_only_that_edge() {
        let mut draft = draft_with([100, 80], [20, 20], [60, 50]);
        gesture(
            &mut draft,
            Some(Grab::Resize(Handle::Right)),
            [60.0, 35.0],
            [75.0, 5.0],
        );
        assert_eq!(bounds(&draft), ([20, 20], [75, 50]));
        gesture(
            &mut draft,
            Some(Grab::Resize(Handle::Top)),
            [40.0, 20.0],
            [90.0, 8.0],
        );
        assert_eq!(bounds(&draft), ([20, 8], [75, 50]));
    }

    #[test]
    fn an_edge_cannot_cross_the_opposite_one_or_leave_the_image() {
        let mut draft = draft_with([100, 80], [20, 20], [60, 50]);
        gesture(
            &mut draft,
            Some(Grab::Resize(Handle::Left)),
            [20.0, 30.0],
            [500.0, 30.0],
        );
        assert_eq!(bounds(&draft), ([59, 20], [60, 50]));
        gesture(
            &mut draft,
            Some(Grab::Resize(Handle::Bottom)),
            [59.0, 50.0],
            [59.0, -80.0],
        );
        assert_eq!(bounds(&draft), ([59, 20], [60, 21]));
        gesture(
            &mut draft,
            Some(Grab::Resize(Handle::BottomRight)),
            [60.0, 21.0],
            [900.0, 900.0],
        );
        assert_eq!(bounds(&draft), ([59, 20], [100, 80]));
    }

    #[test]
    fn moving_keeps_the_size_and_stops_at_the_image_edges() {
        let mut draft = draft_with([100, 80], [20, 20], [60, 50]);
        gesture(&mut draft, Some(Grab::Move), [30.0, 30.0], [35.4, 25.6]);
        assert_eq!(bounds(&draft), ([25, 16], [65, 46]));
        gesture(&mut draft, Some(Grab::Move), [30.0, 30.0], [500.0, 500.0]);
        assert_eq!(bounds(&draft), ([60, 50], [100, 80]));
        gesture(&mut draft, Some(Grab::Move), [30.0, 30.0], [-500.0, -500.0]);
        assert_eq!(bounds(&draft), ([0, 0], [40, 30]));
    }

    #[test]
    fn grabbing_with_no_rectangle_creates_one() {
        let mut draft = CropDraft::new([100, 80]);
        gesture(&mut draft, Some(Grab::Move), [10.0, 10.0], [30.0, 30.0]);
        assert_eq!(bounds(&draft), ([10, 10], [30, 30]));
    }

    #[test]
    fn only_a_partial_rectangle_is_ready_to_apply() {
        assert!(draft_with([100, 80], [0, 0], [100, 80]).ready().is_none());
        assert!(draft_with([100, 80], [0, 0], [99, 80]).ready().is_some());
    }

    #[test]
    fn the_active_grab_reports_the_gesture_in_progress() {
        let mut draft = draft_with([100, 80], [20, 20], [60, 50]);
        assert_eq!(draft.active_grab(), None);
        draft.begin(Some(Grab::Resize(Handle::Left)), [20.0, 30.0]);
        assert!(draft.is_dragging());
        assert_eq!(draft.active_grab(), Some(Grab::Resize(Handle::Left)));
        draft.finish();
        assert!(!draft.is_dragging());
        draft.begin(None, [1.0, 1.0]);
        assert!(draft.is_dragging());
        assert_eq!(draft.active_grab(), None);
    }

    #[test]
    fn dragging_without_a_gesture_changes_nothing() {
        let mut draft = draft_with([100, 80], [20, 20], [60, 50]);
        draft.drag([5.0, 5.0]);
        assert_eq!(bounds(&draft), ([20, 20], [60, 50]));
    }
}
