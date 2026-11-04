use std::sync::Arc;

use eframe::egui::{
    Align, Button, Color32, Frame, Label, Layout, Panel, Rect, RichText, Ui, UiBuilder, Vec2,
};

use crate::app::canvas::Readout;
use crate::app::flat_button::{ButtonState, icon_button};
use crate::app::icons::Icon;
use crate::app::info_popover;
use crate::app::theme;
use crate::core::info::ImageInfo;
use crate::core::loader::Activity;

const HEIGHT: f32 = 24.0;
const PADDING_X: f32 = 10.0;
const ITEM_SPACING: f32 = 16.0;
const BUTTON_SIDE: f32 = 20.0;
const ICON_REACH: f32 = 6.0;
const ZOOM_BUTTON_WIDTH: f32 = 44.0;
const TEXT_SIZE: f32 = 10.0;
const BYTES_PER_KB: f64 = 1000.0;

pub struct ImageStatus {
    pub width: usize,
    pub height: usize,
    pub file_size: Option<u64>,
    pub info: Arc<ImageInfo>,
    pub readout: Readout,
}

pub enum Notice {
    None,
    Busy(Activity),
    Working(&'static str),
    Done(String),
    Failed(String),
}

#[derive(Default)]
pub struct StatusBarActions {
    pub pixelation_toggled: bool,
    pub actual_size_requested: bool,
    pub cancel_requested: bool,
    pub info_toggled: bool,
    pub info_dismissed: bool,
}

pub fn reserve(ui: &mut Ui) -> Rect {
    Panel::bottom("status_bar")
        .exact_size(HEIGHT)
        .frame(Frame::NONE.fill(theme::panel_fill()))
        .show(ui, |_| {})
        .response
        .rect
}

pub fn show(
    ui: &mut Ui,
    bar: Rect,
    image: Option<&ImageStatus>,
    notice: &Notice,
    pixelated: bool,
    info_open: bool,
) -> StatusBarActions {
    let mut actions = StatusBarActions::default();
    let content = bar.shrink2(Vec2::new(PADDING_X, 0.0));
    let builder = UiBuilder::new()
        .max_rect(content)
        .layout(Layout::right_to_left(Align::Center));
    ui.scope_builder(builder, |ui| {
        ui.spacing_mut().item_spacing.x = ITEM_SPACING;
        if let Some(image) = image {
            let state = if info_open {
                ButtonState::Selected
            } else {
                ButtonState::Idle
            };
            let button = icon_button(ui, Icon::Info, BUTTON_SIDE, ICON_REACH, state)
                .on_hover_text("Image info");
            actions.info_toggled = button.clicked();
            if info_open && info_popover::show(&button, image) && !actions.info_toggled {
                actions.info_dismissed = true;
            }
        }
        let pixelation_state = if pixelated {
            ButtonState::Selected
        } else {
            ButtonState::Idle
        };
        actions.pixelation_toggled =
            icon_button(ui, Icon::Pixels, BUTTON_SIDE, ICON_REACH, pixelation_state)
                .on_hover_text("Sharp pixels when zoomed in")
                .clicked();
        if let Some(image) = image {
            actions.actual_size_requested = zoom_button(ui, image.readout.zoom_percent);
            status_text(ui, pointer_text(image.readout), theme::text_dim());
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            if let Some(image) = image {
                status_text(
                    ui,
                    format!("{} x {}", image.width, image.height),
                    theme::text_dim(),
                );
                if let Some(bytes) = image.file_size {
                    status_text(ui, format_size(bytes), theme::text_dim());
                }
            }
            actions.cancel_requested = show_notice(ui, notice);
        });
    });
    actions
}

fn show_notice(ui: &mut Ui, notice: &Notice) -> bool {
    match notice {
        Notice::None => false,
        Notice::Busy(activity) => {
            let cancellable = activity.is_cancellable();
            let reserved = if cancellable {
                BUTTON_SIDE + ui.spacing().item_spacing.x
            } else {
                0.0
            };
            ui.scope(|ui| {
                ui.set_max_width((ui.available_width() - reserved).max(0.0));
                truncated_text(ui, activity_text(activity), theme::text_busy());
            });
            cancellable && cancel_button(ui)
        }
        Notice::Working(text) => {
            truncated_text(ui, (*text).to_owned(), theme::text_busy());
            false
        }
        Notice::Done(text) => {
            truncated_text(ui, text.clone(), theme::text_dim());
            false
        }
        Notice::Failed(message) => {
            let text = format!("Failed: {message}");
            truncated_text(ui, text.clone(), theme::text_error()).on_hover_text(text);
            false
        }
    }
}

fn cancel_button(ui: &mut Ui) -> bool {
    icon_button(ui, Icon::Close, BUTTON_SIDE, ICON_REACH, ButtonState::Idle)
        .on_hover_text("Cancel")
        .clicked()
}

fn activity_text(activity: &Activity) -> String {
    match activity {
        Activity::ReadingClipboard => "Reading clipboard...".to_owned(),
        Activity::ReceivingDrop => "Receiving dropped data...".to_owned(),
        Activity::ReadingFile { title } => format!("Reading {title}..."),
        Activity::Decoding { title } => format!("Decoding {title}..."),
        Activity::Downloading {
            title,
            received,
            total: Some(total),
        } => format!(
            "Downloading {title}  {} / {}",
            format_size(*received),
            format_size(*total)
        ),
        Activity::Downloading {
            title,
            received,
            total: None,
        } => format!("Downloading {title}  {}", format_size(*received)),
    }
}

fn zoom_button(ui: &mut Ui, zoom_percent: u32) -> bool {
    let text = RichText::new(format!("{zoom_percent}%"))
        .size(TEXT_SIZE)
        .monospace()
        .color(theme::text_dim());
    ui.add(
        Button::new(text)
            .frame(false)
            .min_size(Vec2::new(ZOOM_BUTTON_WIDTH, 0.0)),
    )
    .on_hover_text("Actual size (100%)")
    .clicked()
}

fn status_text(ui: &mut Ui, text: String, color: Color32) {
    ui.add(Label::new(styled(text, color)).selectable(false));
}

fn truncated_text(ui: &mut Ui, text: String, color: Color32) -> eframe::egui::Response {
    ui.add(Label::new(styled(text, color)).truncate().selectable(false))
}

fn styled(text: String, color: Color32) -> RichText {
    RichText::new(text).size(TEXT_SIZE).monospace().color(color)
}

fn pointer_text(readout: Readout) -> String {
    let (x, y) = match readout.pointer_pixel {
        Some([x, y]) => (x.to_string(), y.to_string()),
        None => ("-".to_owned(), "-".to_owned()),
    };
    format!("x: {x:>5}  y: {y:>5}")
}

pub fn format_size(bytes: u64) -> String {
    let value = bytes as f64;
    if value < BYTES_PER_KB {
        format!("{bytes} B")
    } else if value < BYTES_PER_KB * BYTES_PER_KB {
        format!("{:.1} KB", value / BYTES_PER_KB)
    } else {
        format!("{:.1} MB", value / (BYTES_PER_KB * BYTES_PER_KB))
    }
}
