use eframe::egui::{
    Color32, CursorIcon, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
    Vec2,
};

use super::paint::{self, color32};
use super::{GAP, Parts};
use crate::core::rgba::{FULL_TURN, Hsva, Rgba};

pub const AREA_WIDTH: f32 = 176.0;
pub const SLIDER_WIDTH: f32 = 16.0;
pub const FULL_WIDTH: f32 = AREA_WIDTH + 2.0 * (GAP + SLIDER_WIDTH);
const HEIGHT: f32 = 160.0;
const HANDLE_RADIUS: f32 = 5.5;
const THUMB_HEIGHT: f32 = 5.0;
const THUMB_OVERHANG: f32 = 2.0;
const THUMB_CORNER: f32 = 2.0;
const OUTLINE_WIDTH: f32 = 1.5;
const SHADOW_WIDTH: f32 = 3.5;
const SHADOW: Color32 = Color32::from_black_alpha(170);
const HUE_STOPS: usize = 6;
const ALPHA_STEPS: f32 = 255.0;

pub fn show(ui: &mut Ui, color: &mut Hsva, parts: Parts) {
    if !(parts.area || parts.hue || parts.alpha) {
        return;
    }
    let origin = ui.available_rect_before_wrap().min;
    let mut next_left = origin.x;
    let mut place = |width: f32| {
        let rect = Rect::from_min_size(Pos2::new(next_left, origin.y), Vec2::new(width, HEIGHT));
        next_left = rect.right() + GAP;
        rect
    };
    let area = parts.area.then(|| place(AREA_WIDTH));
    let hue = parts.hue.then(|| place(SLIDER_WIDTH));
    let alpha = parts.alpha.then(|| place(SLIDER_WIDTH));
    let responses = [("area", area), ("hue", hue), ("alpha", alpha)].map(|(name, rect)| {
        rect.map(|rect| ui.interact(rect, ui.scope_id().with(name), Sense::click_and_drag()))
    });
    let [area_response, hue_response, alpha_response] = responses;

    if let Some(position) = area_response.as_ref().and_then(pointer_fraction) {
        color.saturation = position.x;
        color.value = 1.0 - position.y;
    }
    if let Some(position) = hue_response.as_ref().and_then(pointer_fraction) {
        color.hue = position.y * FULL_TURN;
    }
    if let Some(position) = alpha_response.as_ref().and_then(pointer_fraction) {
        color.alpha = (position.y * ALPHA_STEPS).round() as u8;
    }

    let painter = ui.painter();
    if let Some(response) = &area_response {
        paint_area(painter, response.rect, *color);
    }
    if let Some(response) = &hue_response {
        paint_hue(painter, response.rect, color.hue);
    }
    if let Some(response) = &alpha_response {
        paint_alpha(painter, response.rect, *color);
    }
    if area_response.as_ref().is_some_and(Response::hovered) {
        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    }
    let last = next_left - GAP;
    ui.allocate_rect(
        Rect::from_min_max(origin, Pos2::new(last, origin.y + HEIGHT)),
        Sense::hover(),
    );
}

fn pointer_fraction(response: &Response) -> Option<Vec2> {
    if !response.is_pointer_button_down_on() {
        return None;
    }
    let pointer = response.interact_pointer_pos()?;
    let rect = response.rect;
    Some(Vec2::new(
        ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
        ((pointer.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
    ))
}

fn paint_area(painter: &Painter, rect: Rect, color: Hsva) {
    let pure_hue = color32(
        Hsva {
            saturation: 1.0,
            value: 1.0,
            alpha: 255,
            ..color
        }
        .rgba(),
    );
    let mut mesh = Mesh::default();
    paint::gradient(
        &mut mesh,
        rect,
        [Color32::WHITE, pure_hue, Color32::WHITE, pure_hue],
    );
    paint::gradient(
        &mut mesh,
        rect,
        [
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            Color32::BLACK,
            Color32::BLACK,
        ],
    );
    painter.add(Shape::mesh(mesh));
    let handle = Pos2::new(
        rect.left() + color.saturation * rect.width(),
        rect.top() + (1.0 - color.value) * rect.height(),
    );
    painter.circle_stroke(handle, HANDLE_RADIUS, Stroke::new(SHADOW_WIDTH, SHADOW));
    painter.circle_stroke(
        handle,
        HANDLE_RADIUS,
        Stroke::new(OUTLINE_WIDTH, Color32::WHITE),
    );
}

fn paint_hue(painter: &Painter, rect: Rect, hue: f32) {
    let stop_height = rect.height() / HUE_STOPS as f32;
    let at = |stop: usize| {
        color32(
            Hsva {
                hue: stop as f32 * FULL_TURN / HUE_STOPS as f32,
                saturation: 1.0,
                value: 1.0,
                alpha: 255,
            }
            .rgba(),
        )
    };
    let mut mesh = Mesh::default();
    for stop in 0..HUE_STOPS {
        let top = rect.top() + stop as f32 * stop_height;
        let band = Rect::from_min_max(
            Pos2::new(rect.left(), top),
            Pos2::new(rect.right(), top + stop_height),
        );
        let (upper, lower) = (at(stop), at(stop + 1));
        paint::gradient(&mut mesh, band, [upper, upper, lower, lower]);
    }
    painter.add(Shape::mesh(mesh));
    thumb(painter, rect, hue / FULL_TURN);
}

fn paint_alpha(painter: &Painter, rect: Rect, color: Hsva) {
    paint::checker(painter, rect);
    let opaque = color32(Rgba {
        a: 255,
        ..color.rgba()
    });
    let mut mesh = Mesh::default();
    paint::gradient(
        &mut mesh,
        rect,
        [Color32::TRANSPARENT, Color32::TRANSPARENT, opaque, opaque],
    );
    painter.add(Shape::mesh(mesh));
    thumb(painter, rect, f32::from(color.alpha) / ALPHA_STEPS);
}

fn thumb(painter: &Painter, track: Rect, fraction: f32) {
    let center = Pos2::new(track.center().x, track.top() + fraction * track.height());
    let size = Vec2::new(track.width() + 2.0 * THUMB_OVERHANG, THUMB_HEIGHT);
    let rect = Rect::from_center_size(center, size);
    painter.rect_stroke(
        rect,
        THUMB_CORNER,
        Stroke::new(SHADOW_WIDTH, SHADOW),
        StrokeKind::Middle,
    );
    painter.rect_stroke(
        rect,
        THUMB_CORNER,
        Stroke::new(OUTLINE_WIDTH, Color32::WHITE),
        StrokeKind::Middle,
    );
}
