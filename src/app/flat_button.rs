use eframe::egui::{Button, Color32, CornerRadius, Response, Sense, Stroke, Ui, Vec2, vec2};

use crate::app::icons::{self, Icon};
use crate::app::theme;

const RADIUS: u8 = 3;
const OUTLINED_HEIGHT: f32 = 30.0;
const OUTLINED_PADDING: f32 = 16.0;
const OUTLINED_INSET: f32 = 10.0;

#[derive(Clone, Copy)]
pub enum ButtonState {
    Idle,
    Selected,
    Disabled,
}

pub fn icon_button(ui: &mut Ui, icon: Icon, side: f32, reach: f32, state: ButtonState) -> Response {
    let sense = match state {
        ButtonState::Disabled => Sense::hover(),
        ButtonState::Idle | ButtonState::Selected => Sense::click(),
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), sense);
    let painter = ui.painter();
    if let Some(fill) = background(&response, state) {
        painter.rect_filled(rect, CornerRadius::same(RADIUS), fill);
    }
    icons::paint(painter, rect.center(), reach, icon, foreground(state));
    response
}

fn background(response: &Response, state: ButtonState) -> Option<Color32> {
    match state {
        ButtonState::Disabled => None,
        _ if response.is_pointer_button_down_on() => Some(theme::pressed_fill()),
        ButtonState::Selected => Some(theme::selected_fill()),
        ButtonState::Idle if response.hovered() => Some(theme::hover_fill()),
        ButtonState::Idle => None,
    }
}

fn foreground(state: ButtonState) -> Color32 {
    match state {
        ButtonState::Idle => theme::icon_idle(),
        ButtonState::Selected => theme::icon_selected(),
        ButtonState::Disabled => theme::icon_disabled(),
    }
}

pub fn outlined_button(ui: &mut Ui, text: &str) -> Response {
    ui.scope(|ui| {
        theme::flat_item_style(ui.style_mut());
        ui.spacing_mut().button_padding = vec2(OUTLINED_PADDING, 0.0);
        ui.horizontal(|ui| {
            ui.add_space(OUTLINED_INSET);
            let button = Button::new(text)
                .stroke(Stroke::new(1.0, theme::hover_fill()))
                .min_size(vec2(0.0, OUTLINED_HEIGHT));
            ui.add(button)
        })
        .inner
    })
    .inner
}
