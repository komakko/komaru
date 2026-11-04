use eframe::egui::{
    Align, Align2, Area, Button, Color32, CornerRadius, CursorIcon, Frame, Id, Label, Layout,
    Margin, Order, Painter, PointerButton, Pos2, Rect, RichText, Stroke, StrokeKind, Ui, Vec2,
    vec2,
};

use crate::app::action::Action;
use crate::app::canvas::CanvasView;
use crate::app::crop_draft::{CropDraft, Grab, Handle};
use crate::app::theme;
use crate::core::crop::PixelRect;

const HIT_REACH: f32 = 8.0;
const HANDLE_SIDE: f32 = 8.0;
const MIDPOINT_HANDLE_MIN_EDGE: f32 = HANDLE_SIDE * 3.0;
const OUTLINE_WIDTH: f32 = 3.0;
const DIMMING: Color32 = Color32::from_black_alpha(150);
const OUTLINE: Color32 = Color32::from_black_alpha(190);
const BAR_ID: &str = "crop_bar";
const BAR_BOTTOM_GAP: f32 = 16.0;
const BAR_RADIUS: u8 = 3;
const BAR_MARGIN: Margin = Margin::symmetric(10, 6);
const BAR_GAP: f32 = 8.0;
const LABEL_WIDTH: f32 = 160.0;
const LABEL_HEIGHT: f32 = 24.0;

pub fn show(ui: &mut Ui, view: &CanvasView, draft: &mut CropDraft) -> Option<Action> {
    drive(ui, view, draft);
    let painter = ui.painter().with_clip_rect(ui.max_rect());
    if let Some(rect) = draft.rect() {
        paint_selection(&painter, screen_rect(view, rect));
    }
    set_cursor(ui, view, draft);
    action_bar(ui, draft)
}

fn drive(ui: &Ui, view: &CanvasView, draft: &mut CropDraft) {
    let response = &view.response;
    let to_image = |position: Pos2| {
        let local = (position - view.image_rect.min) / view.scale;
        [local.x, local.y]
    };
    if response.drag_started_by(PointerButton::Primary)
        && let Some(origin) = ui.input(|input| input.pointer.press_origin())
    {
        let grab = draft
            .rect()
            .and_then(|rect| grab_at(screen_rect(view, rect), origin));
        draft.begin(grab, to_image(origin));
    }
    if response.dragged_by(PointerButton::Primary)
        && let Some(position) = response.interact_pointer_pos()
    {
        draft.drag(to_image(position));
    }
    if response.drag_stopped() {
        draft.finish();
    }
}

fn screen_rect(view: &CanvasView, rect: PixelRect) -> Rect {
    let corner = |[x, y]: [usize; 2]| view.image_rect.min + vec2(x as f32, y as f32) * view.scale;
    Rect::from_min_max(corner(rect.min()), corner(rect.max()))
}

fn grab_at(rect: Rect, pointer: Pos2) -> Option<Grab> {
    let reach = vec2(
        HIT_REACH.min(rect.width() / 3.0),
        HIT_REACH.min(rect.height() / 3.0),
    );
    if !rect.expand2(reach).contains(pointer) {
        return None;
    }
    let column = nearest_edge(pointer.x, rect.left(), rect.right(), reach.x);
    let row = nearest_edge(pointer.y, rect.top(), rect.bottom(), reach.y);
    Some(match (column, row) {
        (Some(Side::Low), Some(Side::Low)) => Grab::Resize(Handle::TopLeft),
        (Some(Side::High), Some(Side::Low)) => Grab::Resize(Handle::TopRight),
        (Some(Side::Low), Some(Side::High)) => Grab::Resize(Handle::BottomLeft),
        (Some(Side::High), Some(Side::High)) => Grab::Resize(Handle::BottomRight),
        (Some(Side::Low), None) => Grab::Resize(Handle::Left),
        (Some(Side::High), None) => Grab::Resize(Handle::Right),
        (None, Some(Side::Low)) => Grab::Resize(Handle::Top),
        (None, Some(Side::High)) => Grab::Resize(Handle::Bottom),
        (None, None) => Grab::Move,
    })
}

#[derive(Clone, Copy)]
enum Side {
    Low,
    High,
}

fn nearest_edge(value: f32, low: f32, high: f32, reach: f32) -> Option<Side> {
    if (value - low).abs() <= reach {
        Some(Side::Low)
    } else if (value - high).abs() <= reach {
        Some(Side::High)
    } else {
        None
    }
}

fn paint_selection(painter: &Painter, rect: Rect) {
    let bounds = painter.clip_rect();
    for outside in [
        Rect::from_min_max(bounds.min, Pos2::new(bounds.max.x, rect.top())),
        Rect::from_min_max(Pos2::new(bounds.min.x, rect.bottom()), bounds.max),
        Rect::from_min_max(
            Pos2::new(bounds.min.x, rect.top()),
            Pos2::new(rect.left(), rect.bottom()),
        ),
        Rect::from_min_max(
            Pos2::new(rect.right(), rect.top()),
            Pos2::new(bounds.max.x, rect.bottom()),
        ),
    ] {
        painter.rect_filled(outside, CornerRadius::ZERO, DIMMING);
    }
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(OUTLINE_WIDTH, OUTLINE),
        StrokeKind::Middle,
    );
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::WHITE),
        StrokeKind::Middle,
    );
    for center in handle_centers(rect) {
        let square = Rect::from_center_size(center, Vec2::splat(HANDLE_SIDE));
        painter.rect(
            square,
            CornerRadius::ZERO,
            Color32::WHITE,
            Stroke::new(1.0, OUTLINE),
            StrokeKind::Middle,
        );
    }
}

fn handle_centers(rect: Rect) -> Vec<Pos2> {
    let mut centers = vec![
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ];
    if rect.width() >= MIDPOINT_HANDLE_MIN_EDGE {
        centers.push(Pos2::new(rect.center().x, rect.top()));
        centers.push(Pos2::new(rect.center().x, rect.bottom()));
    }
    if rect.height() >= MIDPOINT_HANDLE_MIN_EDGE {
        centers.push(Pos2::new(rect.left(), rect.center().y));
        centers.push(Pos2::new(rect.right(), rect.center().y));
    }
    centers
}

fn set_cursor(ui: &Ui, view: &CanvasView, draft: &CropDraft) {
    let response = &view.response;
    if response.dragged() && !response.dragged_by(PointerButton::Primary) {
        return;
    }
    let grab = if draft.is_dragging() {
        draft.active_grab()
    } else if let (Some(pointer), Some(rect)) = (response.hover_pos(), draft.rect()) {
        grab_at(screen_rect(view, rect), pointer)
    } else {
        None
    };
    if draft.is_dragging() || response.hovered() {
        ui.ctx().set_cursor_icon(cursor_for(grab));
    }
}

fn cursor_for(grab: Option<Grab>) -> CursorIcon {
    match grab {
        None => CursorIcon::Crosshair,
        Some(Grab::Move) => CursorIcon::Move,
        Some(Grab::Resize(Handle::TopLeft | Handle::BottomRight)) => CursorIcon::ResizeNwSe,
        Some(Grab::Resize(Handle::TopRight | Handle::BottomLeft)) => CursorIcon::ResizeNeSw,
        Some(Grab::Resize(Handle::Top | Handle::Bottom)) => CursorIcon::ResizeVertical,
        Some(Grab::Resize(Handle::Left | Handle::Right)) => CursorIcon::ResizeHorizontal,
    }
}

fn action_bar(ui: &Ui, draft: &CropDraft) -> Option<Action> {
    let viewer = ui.max_rect();
    let position = Pos2::new(viewer.center().x, viewer.bottom() - BAR_BOTTOM_GAP);
    Area::new(Id::unique(BAR_ID))
        .order(Order::Foreground)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(position)
        .show(ui.ctx(), |ui| {
            Frame::NONE
                .fill(theme::panel_fill())
                .stroke(Stroke::new(1.0, theme::hover_fill()))
                .corner_radius(CornerRadius::same(BAR_RADIUS))
                .inner_margin(BAR_MARGIN)
                .show(ui, |ui| bar_content(ui, draft))
                .inner
        })
        .inner
}

fn bar_content(ui: &mut Ui, draft: &CropDraft) -> Option<Action> {
    theme::flat_item_style(ui.style_mut());
    ui.spacing_mut().item_spacing.x = BAR_GAP;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            vec2(LABEL_WIDTH, LABEL_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| ui.add(Label::new(summary(draft)).selectable(false)),
        );
        let cancel = ui.add(Button::new("Cancel")).clicked();
        let apply = ui
            .add_enabled(draft.ready().is_some(), Button::new("Crop"))
            .clicked();
        if apply {
            Some(Action::ApplyCrop)
        } else if cancel {
            Some(Action::CancelCrop)
        } else {
            None
        }
    })
    .inner
}

fn summary(draft: &CropDraft) -> RichText {
    match draft.rect() {
        Some(rect) => {
            let [width, height] = rect.size();
            RichText::new(format!("{width} x {height} px"))
        }
        None => RichText::new("Drag to select an area").color(theme::text_dim()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Rect {
        Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 250.0))
    }

    fn at(x: f32, y: f32) -> Option<Grab> {
        grab_at(square(), Pos2::new(x, y))
    }

    #[test]
    fn corners_edges_and_the_inside_are_told_apart() {
        assert_eq!(at(100.0, 100.0), Some(Grab::Resize(Handle::TopLeft)));
        assert_eq!(at(304.0, 98.0), Some(Grab::Resize(Handle::TopRight)));
        assert_eq!(at(96.0, 255.0), Some(Grab::Resize(Handle::BottomLeft)));
        assert_eq!(at(300.0, 250.0), Some(Grab::Resize(Handle::BottomRight)));
        assert_eq!(at(200.0, 103.0), Some(Grab::Resize(Handle::Top)));
        assert_eq!(at(200.0, 247.0), Some(Grab::Resize(Handle::Bottom)));
        assert_eq!(at(95.0, 170.0), Some(Grab::Resize(Handle::Left)));
        assert_eq!(at(305.0, 170.0), Some(Grab::Resize(Handle::Right)));
        assert_eq!(at(200.0, 170.0), Some(Grab::Move));
    }

    #[test]
    fn far_outside_the_rectangle_grabs_nothing() {
        assert_eq!(at(50.0, 170.0), None);
        assert_eq!(at(200.0, 300.0), None);
        assert_eq!(at(320.0, 260.0), None);
    }

    #[test]
    fn a_tiny_rectangle_keeps_its_middle_and_edges_apart() {
        let tiny = Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(13.0, 13.0));
        assert_eq!(
            grab_at(tiny, Pos2::new(10.5, 11.5)),
            Some(Grab::Resize(Handle::Left))
        );
        assert_eq!(grab_at(tiny, Pos2::new(11.5, 11.5)), Some(Grab::Move));
        assert_eq!(
            grab_at(tiny, Pos2::new(12.6, 11.5)),
            Some(Grab::Resize(Handle::Right))
        );
    }

    #[test]
    fn small_rectangles_lose_their_midpoint_handles() {
        let big = square();
        assert_eq!(handle_centers(big).len(), 8);
        let narrow = Rect::from_min_size(Pos2::ZERO, vec2(10.0, 100.0));
        assert_eq!(handle_centers(narrow).len(), 6);
        let small = Rect::from_min_size(Pos2::ZERO, vec2(10.0, 10.0));
        assert_eq!(handle_centers(small).len(), 4);
    }
}
