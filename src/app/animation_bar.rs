use eframe::egui::{
    Align2, Area, Button, Context, CornerRadius, Frame, Id, Label, Margin, Order, Pos2, Rect,
    RichText, Stroke, Ui,
};

use crate::app::action::{Action, Availability};
use crate::app::animation::Animation;
use crate::app::flat_button::{ButtonState, icon_button};
use crate::app::icons::Icon;
use crate::app::shortcuts::ShortcutBus;
use crate::app::theme;

const BAR_ID: &str = "animation_bar";
const BAR_BOTTOM_GAP: f32 = 16.0;
const BAR_RADIUS: u8 = 3;
const BAR_MARGIN: Margin = Margin::symmetric(10, 6);
const BAR_GAP: f32 = 8.0;
const BUTTON_SIDE: f32 = 26.0;
const ICON_REACH: f32 = 6.0;
const SAVE_FRAME: &str = "Save Frame";

pub fn show(
    ctx: &Context,
    viewer: Rect,
    animation: &Animation,
    availability: Availability,
    shortcuts: &ShortcutBus,
) -> Option<Action> {
    let position = Pos2::new(viewer.center().x, viewer.bottom() - BAR_BOTTOM_GAP);
    Area::new(Id::unique(BAR_ID))
        .order(Order::Foreground)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(position)
        .show(ctx, |ui| {
            Frame::NONE
                .fill(theme::panel_fill())
                .stroke(Stroke::new(1.0, theme::hover_fill()))
                .corner_radius(CornerRadius::same(BAR_RADIUS))
                .inner_margin(BAR_MARGIN)
                .show(ui, |ui| content(ui, animation, availability, shortcuts))
                .inner
        })
        .inner
}

fn content(
    ui: &mut Ui,
    animation: &Animation,
    availability: Availability,
    shortcuts: &ShortcutBus,
) -> Option<Action> {
    theme::flat_item_style(ui.style_mut());
    ui.spacing_mut().item_spacing.x = BAR_GAP;
    ui.horizontal(|ui| {
        let (play_icon, play_text) = if animation.is_playing() {
            (Icon::Pause, "Pause")
        } else {
            (Icon::Play, "Play")
        };
        let back = control(
            ui,
            Icon::StepBack,
            "Previous frame",
            Action::PreviousFrame,
            availability,
            shortcuts,
        );
        let toggle = control(
            ui,
            play_icon,
            play_text,
            Action::TogglePlayback,
            availability,
            shortcuts,
        );
        let forward = control(
            ui,
            Icon::StepForward,
            "Next frame",
            Action::NextFrame,
            availability,
            shortcuts,
        );
        let counter = format!("{} / {}", animation.index() + 1, animation.len());
        let counter = RichText::new(counter).color(theme::text_dim());
        ui.add(Label::new(counter).selectable(false));
        let save = ui
            .add(Button::new(SAVE_FRAME))
            .on_hover_text("Save the shown frame as an image")
            .clicked();
        back.or(toggle)
            .or(forward)
            .or(save.then_some(Action::SaveAs))
    })
    .inner
}

fn control(
    ui: &mut Ui,
    icon: Icon,
    tooltip: &str,
    action: Action,
    availability: Availability,
    shortcuts: &ShortcutBus,
) -> Option<Action> {
    let state = if action.is_enabled(availability) {
        ButtonState::Idle
    } else {
        ButtonState::Disabled
    };
    let hint = shortcuts.hinted(ui.ctx(), tooltip, action);
    icon_button(ui, icon, BUTTON_SIDE, ICON_REACH, state)
        .on_hover_text(hint)
        .clicked()
        .then_some(action)
}
