use std::sync::atomic::{AtomicU8, Ordering};

use eframe::egui::{Color32, Context, CornerRadius, Stroke, Style, Theme, ThemePreference, vec2};

const ITEM_RADIUS: u8 = 2;
const FIELD_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    Slate,
    Midnight,
    Light,
    FollowSystem,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Slate,
    Midnight,
    Light,
}

struct Palette {
    panel_fill: Color32,
    viewer_fill: Color32,
    row_stripe: Color32,
    hover_fill: Color32,
    pressed_fill: Color32,
    selected_fill: Color32,
    drop_hover_fill: Color32,
    drop_hover_stroke: Color32,
    icon_idle: Color32,
    icon_selected: Color32,
    icon_disabled: Color32,
    text_dim: Color32,
    text_busy: Color32,
    text_error: Color32,
}

const SLATE: Palette = Palette {
    panel_fill: Color32::from_rgb(30, 34, 42),
    viewer_fill: Color32::from_rgb(22, 25, 31),
    row_stripe: Color32::from_rgb(34, 39, 49),
    hover_fill: Color32::from_rgb(50, 58, 74),
    pressed_fill: Color32::from_rgb(38, 44, 56),
    selected_fill: Color32::from_rgb(58, 86, 130),
    drop_hover_fill: Color32::from_rgba_premultiplied(10, 20, 36, 64),
    drop_hover_stroke: Color32::from_rgb(110, 150, 215),
    icon_idle: Color32::from_rgb(198, 206, 220),
    icon_selected: Color32::WHITE,
    icon_disabled: Color32::from_rgb(84, 92, 108),
    text_dim: Color32::from_rgb(150, 160, 178),
    text_busy: Color32::from_rgb(198, 206, 220),
    text_error: Color32::from_rgb(232, 112, 112),
};

const MIDNIGHT: Palette = Palette {
    panel_fill: Color32::from_rgb(14, 16, 22),
    viewer_fill: Color32::from_rgb(6, 7, 10),
    row_stripe: Color32::from_rgb(18, 21, 28),
    hover_fill: Color32::from_rgb(36, 42, 56),
    pressed_fill: Color32::from_rgb(26, 30, 40),
    selected_fill: Color32::from_rgb(48, 74, 120),
    drop_hover_fill: Color32::from_rgba_premultiplied(8, 16, 30, 64),
    drop_hover_stroke: Color32::from_rgb(100, 140, 205),
    icon_idle: Color32::from_rgb(188, 196, 210),
    icon_selected: Color32::WHITE,
    icon_disabled: Color32::from_rgb(66, 72, 86),
    text_dim: Color32::from_rgb(138, 148, 166),
    text_busy: Color32::from_rgb(188, 196, 210),
    text_error: Color32::from_rgb(232, 112, 112),
};

const LIGHT: Palette = Palette {
    panel_fill: Color32::from_rgb(240, 242, 246),
    viewer_fill: Color32::from_rgb(222, 226, 233),
    row_stripe: Color32::from_rgb(232, 235, 241),
    hover_fill: Color32::from_rgb(208, 214, 226),
    pressed_fill: Color32::from_rgb(220, 225, 235),
    selected_fill: Color32::from_rgb(156, 184, 230),
    drop_hover_fill: Color32::from_rgba_premultiplied(8, 16, 30, 40),
    drop_hover_stroke: Color32::from_rgb(60, 100, 180),
    icon_idle: Color32::from_rgb(52, 60, 76),
    icon_selected: Color32::from_rgb(12, 18, 30),
    icon_disabled: Color32::from_rgb(168, 174, 188),
    text_dim: Color32::from_rgb(96, 106, 124),
    text_busy: Color32::from_rgb(52, 60, 76),
    text_error: Color32::from_rgb(190, 48, 48),
};

static CURRENT: AtomicU8 = AtomicU8::new(0);

impl Kind {
    fn palette(self) -> &'static Palette {
        match self {
            Self::Slate => &SLATE,
            Self::Midnight => &MIDNIGHT,
            Self::Light => &LIGHT,
        }
    }

    fn index(self) -> u8 {
        match self {
            Self::Slate => 0,
            Self::Midnight => 1,
            Self::Light => 2,
        }
    }

    fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Midnight,
            2 => Self::Light,
            _ => Self::Slate,
        }
    }
}

pub fn apply(ctx: &Context, choice: Choice) {
    let preference = match choice {
        Choice::Slate | Choice::Midnight => ThemePreference::Dark,
        Choice::Light => ThemePreference::Light,
        Choice::FollowSystem => ThemePreference::System,
    };
    if ctx.options(|options| options.theme_preference) != preference {
        ctx.set_theme(preference);
    }
    let kind = match choice {
        Choice::Slate => Kind::Slate,
        Choice::Midnight => Kind::Midnight,
        Choice::Light => Kind::Light,
        Choice::FollowSystem => match ctx.theme() {
            Theme::Light => Kind::Light,
            Theme::Dark => Kind::Slate,
        },
    };
    if CURRENT.swap(kind.index(), Ordering::AcqRel) != kind.index() {
        ctx.request_repaint();
    }
}

fn current() -> &'static Palette {
    Kind::from_index(CURRENT.load(Ordering::Acquire)).palette()
}

pub fn panel_fill() -> Color32 {
    current().panel_fill
}

pub fn viewer_fill() -> Color32 {
    current().viewer_fill
}

pub fn row_stripe() -> Color32 {
    current().row_stripe
}

pub fn hover_fill() -> Color32 {
    current().hover_fill
}

pub fn pressed_fill() -> Color32 {
    current().pressed_fill
}

pub fn selected_fill() -> Color32 {
    current().selected_fill
}

pub fn drop_hover_fill() -> Color32 {
    current().drop_hover_fill
}

pub fn drop_hover_stroke() -> Color32 {
    current().drop_hover_stroke
}

pub fn icon_idle() -> Color32 {
    current().icon_idle
}

pub fn icon_selected() -> Color32 {
    current().icon_selected
}

pub fn icon_disabled() -> Color32 {
    current().icon_disabled
}

pub fn text_dim() -> Color32 {
    current().text_dim
}

pub fn text_busy() -> Color32 {
    current().text_busy
}

pub fn text_error() -> Color32 {
    current().text_error
}

pub fn flat_item_style(style: &mut Style) {
    let visuals = &mut style.visuals;
    visuals.selection.bg_fill = selected_fill();
    visuals.selection.stroke = Stroke::new(1.0, icon_selected());
    let states = [
        (
            &mut visuals.widgets.inactive,
            Color32::TRANSPARENT,
            icon_idle(),
        ),
        (&mut visuals.widgets.hovered, hover_fill(), icon_selected()),
        (&mut visuals.widgets.active, pressed_fill(), icon_selected()),
    ];
    for (widget, fill, text) in states {
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0, text);
        widget.corner_radius = CornerRadius::same(ITEM_RADIUS);
        widget.expansion = 0.0;
    }
}

pub fn field_style(style: &mut Style) {
    flat_item_style(style);
    style.spacing.button_padding = vec2(8.0, 3.0);
    style.spacing.interact_size.y = FIELD_HEIGHT;
    let visuals = &mut style.visuals;
    visuals.window_fill = panel_fill();
    visuals.window_stroke = Stroke::new(1.0, hover_fill());
    visuals.extreme_bg_color = viewer_fill();
    let states = [
        (&mut visuals.widgets.inactive, viewer_fill(), hover_fill()),
        (&mut visuals.widgets.hovered, hover_fill(), text_dim()),
        (&mut visuals.widgets.active, pressed_fill(), text_dim()),
        (&mut visuals.widgets.open, pressed_fill(), hover_fill()),
    ];
    for (widget, fill, edge) in states {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, edge);
        widget.corner_radius = CornerRadius::same(ITEM_RADIUS);
        widget.expansion = 0.0;
    }
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, icon_selected());
}
