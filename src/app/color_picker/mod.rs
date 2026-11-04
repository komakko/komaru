mod fields;
pub mod paint;
mod surface;

use std::time::Duration;

use eframe::egui::{Align, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::app::flat_button::{ButtonState, icon_button};
use crate::app::icons::Icon;
use crate::app::theme;
use crate::core::rgba::{FULL_TURN, Hsva, Rgba};
use crate::error::PlatformError;
use crate::platform::clipboard;
use fields::Draft;
use paint::color32;

const GAP: f32 = 8.0;
const SWATCH_SIZE: Vec2 = Vec2::new(72.0, 24.0);
const BUTTON_SIDE: f32 = 26.0;
const BUTTON_REACH: f32 = 7.0;
const COPIED_SECONDS: f64 = 1.2;
const PERCENT: f32 = 100.0;
const MAX_BYTE: u32 = 255;
const MAX_PERCENT: u32 = 100;

#[derive(Clone, Copy)]
pub struct Parts {
    pub area: bool,
    pub hue: bool,
    pub alpha: bool,
    pub swatch: bool,
    pub channels: bool,
    pub hex: bool,
    pub copy: bool,
    pub revert: bool,
}

impl Parts {
    pub const ALL: Self = Self {
        area: true,
        hue: true,
        alpha: true,
        swatch: true,
        channels: true,
        hex: true,
        copy: true,
        revert: true,
    };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Model {
    Rgb,
    Hsv,
}

pub struct ColorPicker {
    color: Hsva,
    seed: Option<Rgba>,
    model: Model,
    draft: Option<Draft>,
    copied_until: f64,
    copy_failure: Option<PlatformError>,
    hash_sign: bool,
}

impl ColorPicker {
    pub fn new(seed: Option<Rgba>) -> Self {
        Self {
            color: Hsva::from_rgba(seed.unwrap_or(Rgba::OPAQUE_BLACK)),
            seed,
            model: Model::Rgb,
            draft: None,
            copied_until: 0.0,
            copy_failure: None,
            hash_sign: true,
        }
    }

    pub fn reseed(&mut self, seed: Rgba) {
        self.color = Hsva::from_rgba_keeping(seed, self.color);
        self.seed = Some(seed);
        self.draft = None;
    }

    pub fn set_hash_sign(&mut self, on: bool) {
        self.hash_sign = on;
    }

    fn copy_text(&self, parts: Parts) -> String {
        let hex = self.shown(parts).hex();
        if self.hash_sign {
            hex
        } else {
            hex.trim_start_matches('#').to_owned()
        }
    }

    pub fn take_copy_failure(&mut self) -> Option<PlatformError> {
        self.copy_failure.take()
    }

    fn shown(&self, parts: Parts) -> Rgba {
        let color = self.color.rgba();
        if parts.alpha { color } else { color.opaque() }
    }

    fn revert_target(&self, parts: Parts) -> Option<Rgba> {
        self.seed
            .map(|seed| if parts.alpha { seed } else { seed.opaque() })
            .filter(|target| *target != self.shown(parts))
    }
}

pub fn show(ui: &mut Ui, picker: &mut ColorPicker, parts: Parts) -> Option<Rgba> {
    let before = picker.shown(parts);
    picker.color.alpha = before.a;
    ui.set_min_width(surface::FULL_WIDTH);
    ui.spacing_mut().item_spacing = Vec2::splat(GAP);
    surface::show(ui, &mut picker.color, parts);
    if parts.swatch || parts.channels {
        header(ui, picker, parts);
    }
    if parts.channels {
        channels(ui, picker, parts);
    }
    if parts.hex || parts.copy || parts.revert {
        footer(ui, picker, parts);
    }
    if !parts.alpha {
        picker.color.alpha = 255;
    }
    let after = picker.shown(parts);
    (after != before).then_some(after)
}

fn header(ui: &mut Ui, picker: &mut ColorPicker, parts: Parts) {
    ui.horizontal(|ui| {
        if parts.swatch {
            swatch(ui, picker, parts);
        }
        if parts.channels {
            let taken = if parts.swatch {
                SWATCH_SIZE.x + GAP
            } else {
                0.0
            };
            let size = Vec2::new(surface::FULL_WIDTH - taken, SWATCH_SIZE.y);
            ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
                ui.selectable_value(&mut picker.model, Model::Hsv, "HSV");
                ui.selectable_value(&mut picker.model, Model::Rgb, "RGB");
            });
        }
    });
}

fn swatch(ui: &mut Ui, picker: &ColorPicker, parts: Parts) {
    let (rect, response) = ui.allocate_exact_size(SWATCH_SIZE, Sense::hover());
    let painter = ui.painter();
    paint::checker(painter, rect);
    let current = color32(picker.shown(parts));
    match picker.seed {
        Some(seed) => {
            let (previous, now) = split(rect);
            let seed = if parts.alpha { seed } else { seed.opaque() };
            painter.rect_filled(previous, 0.0, color32(seed));
            painter.rect_filled(now, 0.0, current);
            response.on_hover_text("Starting colour on the left, current on the right");
        }
        None => {
            painter.rect_filled(rect, 0.0, current);
        }
    }
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, theme::hover_fill()),
        StrokeKind::Inside,
    );
}

fn split(rect: Rect) -> (Rect, Rect) {
    let middle = rect.center().x;
    (
        Rect::from_min_max(rect.min, Pos2::new(middle, rect.bottom())),
        Rect::from_min_max(Pos2::new(middle, rect.top()), rect.max),
    )
}

fn channels(ui: &mut Ui, picker: &mut ColorPicker, parts: Parts) {
    ui.horizontal_top(|ui| {
        match picker.model {
            Model::Rgb => rgb_fields(ui, picker),
            Model::Hsv => hsv_fields(ui, picker),
        }
        if parts.alpha {
            let value = u32::from(picker.color.alpha);
            if let Some(alpha) = fields::number(ui, &mut picker.draft, "A", value, MAX_BYTE) {
                picker.color.alpha = alpha as u8;
            }
        }
    });
}

fn rgb_fields(ui: &mut Ui, picker: &mut ColorPicker) {
    let current = picker.color.rgba();
    let mut edited = current;
    for (label, channel) in [
        ("R", &mut edited.r),
        ("G", &mut edited.g),
        ("B", &mut edited.b),
    ] {
        if let Some(value) =
            fields::number(ui, &mut picker.draft, label, u32::from(*channel), MAX_BYTE)
        {
            *channel = value as u8;
        }
    }
    if edited != current {
        picker.color = Hsva::from_rgba_keeping(edited, picker.color);
    }
}

fn hsv_fields(ui: &mut Ui, picker: &mut ColorPicker) {
    let color = &mut picker.color;
    let hue = color.hue.round() as u32;
    if let Some(value) = fields::number(ui, &mut picker.draft, "H", hue, FULL_TURN as u32) {
        color.hue = value as f32;
    }
    let saturation = (color.saturation * PERCENT).round() as u32;
    if let Some(value) = fields::number(ui, &mut picker.draft, "S", saturation, MAX_PERCENT) {
        color.saturation = value as f32 / PERCENT;
    }
    let brightness = (color.value * PERCENT).round() as u32;
    if let Some(value) = fields::number(ui, &mut picker.draft, "V", brightness, MAX_PERCENT) {
        color.value = value as f32 / PERCENT;
    }
}

fn footer(ui: &mut Ui, picker: &mut ColorPicker, parts: Parts) {
    ui.horizontal(|ui| {
        let buttons = usize::from(parts.copy) + usize::from(parts.revert);
        let width = surface::FULL_WIDTH - buttons as f32 * (BUTTON_SIDE + GAP);
        if parts.hex {
            let shown = picker.shown(parts);
            if let Some(typed) = fields::hex(ui, &mut picker.draft, shown, width) {
                picker.color = Hsva::from_rgba_keeping(typed, picker.color);
            }
        }
        if parts.copy {
            copy_button(ui, picker, parts);
        }
        if parts.revert {
            revert_button(ui, picker, parts);
        }
    });
}

fn copy_button(ui: &mut Ui, picker: &mut ColorPicker, parts: Parts) {
    let now = ui.input(|input| input.time);
    let copied = now < picker.copied_until;
    let response = icon_button(ui, Icon::Copy, BUTTON_SIDE, BUTTON_REACH, ButtonState::Idle)
        .on_hover_text(if copied { "Copied" } else { "Copy hex code" });
    if response.clicked() {
        match clipboard::write_text(picker.copy_text(parts)) {
            Ok(()) => {
                picker.copied_until = now + COPIED_SECONDS;
                ui.ctx()
                    .request_repaint_after(Duration::from_secs_f64(COPIED_SECONDS));
            }
            Err(failure) => picker.copy_failure = Some(failure),
        }
    }
}

fn revert_button(ui: &mut Ui, picker: &mut ColorPicker, parts: Parts) {
    let target = picker.revert_target(parts);
    let state = if target.is_some() {
        ButtonState::Idle
    } else {
        ButtonState::Disabled
    };
    let response = icon_button(ui, Icon::Reset, BUTTON_SIDE, BUTTON_REACH, state)
        .on_hover_text("Back to the starting colour");
    if let Some(target) = target.filter(|_| response.clicked()) {
        picker.color = Hsva::from_rgba_keeping(target, picker.color);
    }
}
