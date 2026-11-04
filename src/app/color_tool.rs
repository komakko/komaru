use eframe::egui::{
    Color32, ColorImage, Context, CursorIcon, Order, Painter, PointerButton, Pos2, Rect, Shape,
    Stroke, StrokeKind, Ui, Vec2,
};

use crate::app::canvas::CanvasView;
use crate::app::color_picker::ColorPicker;
use crate::app::color_picker::paint::{self, color32};
use crate::core::edit::Orientation;
use crate::core::rgba::Rgba;

const RING_RADIUS: f32 = 12.0;
const RING_OFFSET: f32 = 15.0;
const POINTER_LEG: f32 = 9.0;
const WHITE_WIDTH: f32 = 2.0;
const DARK_WIDTH: f32 = 1.5;
const SHADOW: Color32 = Color32::from_black_alpha(190);
const LOUPE_CELLS: isize = 4;
const LOUPE_CELL_SIDE: f32 = 9.0;
const LOUPE_MARGIN: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PickMode {
    #[default]
    Sticky,
    OneShot,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PickCursor {
    #[default]
    Ring,
    Loupe,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ToolStyle {
    pub mode: PickMode,
    pub cursor: PickCursor,
    pub hash_sign: bool,
}

pub struct ColorTool {
    picker: Option<ColorPicker>,
    style: ToolStyle,
    armed: bool,
}

impl Default for ColorTool {
    fn default() -> Self {
        Self {
            picker: None,
            style: ToolStyle::default(),
            armed: true,
        }
    }
}

impl ColorTool {
    pub fn picker_mut(&mut self) -> Option<&mut ColorPicker> {
        self.picker.as_mut()
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn configure(&mut self, style: ToolStyle) {
        self.style = style;
        if let Some(picker) = &mut self.picker {
            picker.set_hash_sign(style.hash_sign);
        }
    }

    pub fn drive(
        &mut self,
        ui: &Ui,
        view: &CanvasView,
        pixels: &ColorImage,
        orientation: Orientation,
    ) {
        if !self.armed {
            return;
        }
        let response = &view.response;
        if response.dragged() && !response.dragged_by(PointerButton::Primary) {
            return;
        }
        let sample_at = |position: Pos2| {
            let oriented = view.pixel_at(position)?;
            sample(pixels, orientation, oriented)
        };
        if let Some(position) = response.hover_pos() {
            match sample_at(position) {
                Some(color) => {
                    ui.ctx().set_cursor_icon(CursorIcon::None);
                    let painter = ui.painter().with_clip_rect(ui.max_rect());
                    match self.style.cursor {
                        PickCursor::Ring => paint_ring(&painter, position, color),
                        PickCursor::Loupe => {
                            let center = view.pixel_at(position).unwrap_or_default();
                            paint_loupe(&painter, position, |dx, dy| {
                                let x = center[0].checked_add_signed(dx)?;
                                let y = center[1].checked_add_signed(dy)?;
                                sample(pixels, orientation, [x, y])
                            });
                        }
                    }
                }
                None => ui.ctx().set_cursor_icon(CursorIcon::Default),
            }
        }
        let pressing = response.is_pointer_button_down_on()
            && ui.input(|input| input.pointer.button_down(PointerButton::Primary));
        if pressing && let Some(color) = response.interact_pointer_pos().and_then(sample_at) {
            self.pick(color);
            self.armed = self.style.mode == PickMode::Sticky;
        }
    }

    fn pick(&mut self, color: Rgba) {
        match &mut self.picker {
            Some(picker) => picker.reseed(color),
            None => {
                let mut picker = ColorPicker::new(Some(color));
                picker.set_hash_sign(self.style.hash_sign);
                self.picker = Some(picker);
            }
        }
    }
}

pub fn clicked_outside(ctx: &Context, viewer: Rect) -> bool {
    let click = ctx.input(|input| {
        input
            .pointer
            .any_click()
            .then(|| input.pointer.latest_pos())
    });
    click.flatten().is_some_and(|position| {
        !viewer.contains(position)
            && ctx
                .layer_id_at(position)
                .is_none_or(|layer| layer.order == Order::Background)
    })
}

fn sample(pixels: &ColorImage, orientation: Orientation, oriented: [usize; 2]) -> Option<Rgba> {
    let [width, height] = orientation.size(pixels.size);
    if oriented[0] >= width || oriented[1] >= height {
        return None;
    }
    let index = orientation.source_index(pixels.size, oriented);
    pixels
        .pixels
        .get(index)
        .map(|pixel| Rgba::from_premultiplied(pixel.to_array()))
}

fn paint_ring(painter: &Painter, tip: Pos2, color: Rgba) {
    let center = tip + Vec2::splat(RING_OFFSET);
    if color.a < 255 {
        paint::checker_disc(painter, center, RING_RADIUS);
    }
    painter.circle_filled(center, RING_RADIUS, color32(color));
    painter.circle_stroke(center, RING_RADIUS + 1.5, Stroke::new(DARK_WIDTH, SHADOW));
    painter.circle_stroke(
        center,
        RING_RADIUS,
        Stroke::new(WHITE_WIDTH, Color32::WHITE),
    );
    painter.circle_stroke(center, RING_RADIUS - 1.5, Stroke::new(1.0, SHADOW));
    painter.add(Shape::convex_polygon(
        vec![
            tip,
            tip + Vec2::new(POINTER_LEG, 0.0),
            tip + Vec2::new(0.0, POINTER_LEG),
        ],
        Color32::WHITE,
        Stroke::new(1.0, SHADOW),
    ));
}

fn paint_loupe(painter: &Painter, tip: Pos2, color_at: impl Fn(isize, isize) -> Option<Rgba>) {
    let cells = (2 * LOUPE_CELLS + 1) as f32;
    let side = cells * LOUPE_CELL_SIDE;
    let clip = painter.clip_rect();
    let left = if tip.x + RING_OFFSET + side > clip.right() - LOUPE_MARGIN {
        tip.x - RING_OFFSET - side
    } else {
        tip.x + RING_OFFSET
    };
    let top = if tip.y + RING_OFFSET + side > clip.bottom() - LOUPE_MARGIN {
        tip.y - RING_OFFSET - side
    } else {
        tip.y + RING_OFFSET
    };
    let frame = Rect::from_min_size(Pos2::new(left, top), Vec2::splat(side));
    paint::checker(painter, frame);
    for dy in -LOUPE_CELLS..=LOUPE_CELLS {
        for dx in -LOUPE_CELLS..=LOUPE_CELLS {
            let Some(color) = color_at(dx, dy) else {
                continue;
            };
            let origin = frame.min
                + Vec2::new(
                    (dx + LOUPE_CELLS) as f32 * LOUPE_CELL_SIDE,
                    (dy + LOUPE_CELLS) as f32 * LOUPE_CELL_SIDE,
                );
            let cell = Rect::from_min_size(origin, Vec2::splat(LOUPE_CELL_SIDE));
            painter.rect_filled(cell, 0.0, color32(color));
        }
    }
    let middle = Rect::from_center_size(frame.center(), Vec2::splat(LOUPE_CELL_SIDE));
    painter.rect_stroke(
        frame.expand(1.5),
        0.0,
        Stroke::new(DARK_WIDTH, SHADOW),
        StrokeKind::Outside,
    );
    painter.rect_stroke(
        frame,
        0.0,
        Stroke::new(WHITE_WIDTH, Color32::WHITE),
        StrokeKind::Inside,
    );
    painter.rect_stroke(middle, 0.0, Stroke::new(1.0, SHADOW), StrokeKind::Outside);
    painter.rect_stroke(
        middle,
        0.0,
        Stroke::new(1.0, Color32::WHITE),
        StrokeKind::Inside,
    );
    painter.add(Shape::convex_polygon(
        vec![
            tip,
            tip + Vec2::new(POINTER_LEG, 0.0),
            tip + Vec2::new(0.0, POINTER_LEG),
        ],
        Color32::WHITE,
        Stroke::new(1.0, SHADOW),
    ));
}
