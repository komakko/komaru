use crate::app::resize;
use crate::app::tiles::TiledLevel;
use crate::core::edit::Orientation;
use eframe::egui::{Color32, CursorIcon, PointerButton, Pos2, Rect, Response, Sense, Ui, Vec2};

const MIN_ZOOM: f32 = 0.02;
const MAX_ZOOM: f32 = 128.0;
const WHEEL_ZOOM_RATE: f32 = 0.0015;
const MIN_VISIBLE: f32 = 48.0;

#[derive(Clone, Copy, Default)]
enum Placement {
    #[default]
    Fitted,
    Free {
        scale: f32,
        offset: Vec2,
    },
}

impl Placement {
    fn resolve(self, fitted_scale: f32) -> (f32, Vec2) {
        match self {
            Self::Fitted => (fitted_scale, Vec2::ZERO),
            Self::Free { scale, offset } => (scale, offset),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Readout {
    pub zoom_percent: u32,
    pub pointer_pixel: Option<[usize; 2]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PrimaryDrag {
    Pans,
    Reserved,
    Picks,
}

pub struct CanvasView {
    pub readout: Readout,
    pub image_rect: Rect,
    pub scale: f32,
    pub response: Response,
}

impl CanvasView {
    pub fn pixel_at(&self, position: Pos2) -> Option<[usize; 2]> {
        pixel_under(
            position,
            self.image_rect,
            self.scale,
            self.image_rect.size() / self.scale,
        )
    }
}

#[derive(Default)]
pub struct Canvas {
    placement: Placement,
}

impl Canvas {
    pub fn reset(&mut self) {
        self.placement = Placement::Fitted;
    }

    pub fn actual_size(&mut self, pixels_per_point: f32) {
        self.placement = Placement::Free {
            scale: 1.0 / pixels_per_point,
            offset: Vec2::ZERO,
        };
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        levels: &mut [TiledLevel],
        orientation: Orientation,
        primary: PrimaryDrag,
        tint: Color32,
    ) -> CanvasView {
        let area = ui.max_rect();
        let interactive = Rect::from_min_max(
            area.min,
            Pos2::new(area.max.x - resize::grab_inset(ui.ctx()), area.max.y),
        );
        let response = ui.interact(
            interactive,
            ui.scope_id().with("canvas"),
            Sense::click_and_drag(),
        );
        let one_to_one = 1.0 / ui.ctx().pixels_per_point();
        let [width, height] = orientation.size(levels[0].size());
        let image_size = Vec2::new(width as f32, height as f32);
        let fitted_scale = fit_scale(area.size(), image_size, one_to_one);
        let (mut scale, mut offset) = self.placement.resolve(fitted_scale);
        let mut moved = false;

        if let Some(pointer) = response.hover_pos() {
            let factor = zoom_factor(ui);
            if factor != 1.0 {
                let bounds = (MIN_ZOOM * one_to_one).min(fitted_scale)..=MAX_ZOOM * one_to_one;
                let zoomed_scale = (scale * factor).clamp(*bounds.start(), *bounds.end());
                let anchor = pointer - area.center();
                offset = anchor + (offset - anchor) * (zoomed_scale / scale);
                scale = zoomed_scale;
                moved = true;
            }
        }
        let panning = response.dragged()
            && (primary == PrimaryDrag::Pans || !response.dragged_by(PointerButton::Primary));
        if panning {
            offset += response.drag_delta();
            moved = true;
        }
        offset = clamp_offset(offset, image_size * scale, area.size());

        if response.double_clicked() && primary != PrimaryDrag::Picks {
            self.reset();
        } else if moved {
            self.placement = Placement::Free { scale, offset };
        }

        let (scale, offset) = self.placement.resolve(fitted_scale);
        let level = level_for(scale / one_to_one, levels.len());
        let image_rect = Rect::from_center_size(area.center() + offset, image_size * scale);
        let painter = ui.painter().with_clip_rect(area);
        if !levels[level].paint(ui.ctx(), &painter, image_rect, orientation, tint) {
            ui.ctx().request_repaint();
        }
        set_cursor(ui, &response);
        let readout = Readout {
            zoom_percent: (scale / one_to_one * 100.0).round() as u32,
            pointer_pixel: response
                .hover_pos()
                .and_then(|pointer| pixel_under(pointer, image_rect, scale, image_size)),
        };
        CanvasView {
            readout,
            image_rect,
            scale,
            response,
        }
    }
}

fn pixel_under(
    pointer: Pos2,
    image_rect: Rect,
    scale: f32,
    image_size: Vec2,
) -> Option<[usize; 2]> {
    let local = (pointer - image_rect.min) / scale;
    let inside =
        local.x >= 0.0 && local.y >= 0.0 && local.x < image_size.x && local.y < image_size.y;
    if !inside {
        return None;
    }
    Some([local.x as usize, local.y as usize])
}

fn fit_scale(area: Vec2, image: Vec2, one_to_one: f32) -> f32 {
    (area.x / image.x).min(area.y / image.y).min(one_to_one)
}

fn level_for(display_ratio: f32, level_count: usize) -> usize {
    if display_ratio >= 1.0 {
        return 0;
    }
    (display_ratio.recip().log2().floor() as usize).min(level_count - 1)
}

fn zoom_factor(ui: &Ui) -> f32 {
    ui.input(|input| input.zoom_delta() * (input.smooth_scroll_delta.y * WHEEL_ZOOM_RATE).exp())
}

fn clamp_offset(offset: Vec2, shown: Vec2, area: Vec2) -> Vec2 {
    let reach = ((shown + area) / 2.0 - Vec2::splat(MIN_VISIBLE)).max(Vec2::ZERO);
    offset.clamp(-reach, reach)
}

fn set_cursor(ui: &Ui, response: &Response) {
    if response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
}
