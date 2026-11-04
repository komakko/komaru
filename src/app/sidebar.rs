use eframe::egui::{Frame, Margin, Panel, PopupCloseBehavior, Ui};

use crate::app::action::{Action, Availability};
use crate::app::background::ViewerBackground;
use crate::app::color_picker::{self, Parts};
use crate::app::color_tool::ColorTool;
use crate::app::flat_button::{ButtonState, icon_button};
use crate::app::icons::Icon;
use crate::app::popover;
use crate::app::resize_popover::{self, ResizeDraft};
use crate::app::shortcuts::ShortcutBus;
use crate::app::theme;

const WIDTH: f32 = 40.0;
const MARGIN_X: i8 = 4;
const MARGIN_Y: i8 = 6;
const BUTTON_SIDE: f32 = 32.0;
const ICON_REACH: f32 = 8.0;

const RESIZE_TOOLTIP: &str = "Resize";

const BACKGROUND_TOOLTIP: &str = "Viewer background and opacity (right-click to reset)";
const CROP_TOOLTIP: &str = "Crop";
const COLOR_PICKER_TOOLTIP: &str = "Color Picker";

pub fn show(
    ui: &mut Ui,
    availability: Availability,
    resize: Option<&mut ResizeDraft>,
    crop_open: bool,
    color_tool: Option<&mut ColorTool>,
    background: &mut ViewerBackground,
    shortcuts: &ShortcutBus,
) -> Option<Action> {
    let frame = Frame::NONE
        .fill(theme::panel_fill())
        .inner_margin(Margin::symmetric(MARGIN_X, MARGIN_Y));
    Panel::left("sidebar")
        .exact_size(WIDTH)
        .resizable(false)
        .frame(frame)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let picker = color_picker_tool(ui, availability, color_tool, shortcuts);
                let crop = crop_tool(ui, availability, crop_open, shortcuts);
                let resize = resize_tool(ui, availability, resize, shortcuts);
                let rotate = two_way_tool(
                    ui,
                    Icon::Rotate,
                    "Rotate: left-click turns left, right-click turns right",
                    [Action::RotateLeft, Action::RotateRight],
                    availability,
                );
                let flip = two_way_tool(
                    ui,
                    Icon::Flip,
                    "Flip: left-click vertical, right-click horizontal",
                    [Action::FlipVertical, Action::FlipHorizontal],
                    availability,
                );
                background_tool(ui, background);
                picker.or(crop).or(resize).or(rotate).or(flip)
            })
            .inner
        })
        .inner
}

fn background_tool(ui: &mut Ui, background: &mut ViewerBackground) {
    let state = if background.is_open() {
        ButtonState::Selected
    } else {
        ButtonState::Idle
    };
    let response = icon_button(ui, Icon::Opacity, BUTTON_SIDE, ICON_REACH, state)
        .on_hover_text(BACKGROUND_TOOLTIP);
    let toggled = response.clicked();
    if toggled {
        background.toggle();
    }
    if response.secondary_clicked() {
        background.reset();
    }
    background.show_editor(&response, toggled);
}

fn disabled_tool(ui: &mut Ui, icon: Icon, tooltip: &str) {
    icon_button(ui, icon, BUTTON_SIDE, ICON_REACH, ButtonState::Disabled).on_hover_text(tooltip);
}

fn color_picker_tool(
    ui: &mut Ui,
    availability: Availability,
    tool: Option<&mut ColorTool>,
    shortcuts: &ShortcutBus,
) -> Option<Action> {
    let tooltip = shortcuts.hinted(ui.ctx(), COLOR_PICKER_TOOLTIP, Action::OpenColorPicker);
    if !Action::OpenColorPicker.is_enabled(availability) {
        disabled_tool(ui, Icon::ColorPicker, &tooltip);
        return None;
    }
    let open = tool.is_some();
    let state = if open {
        ButtonState::Selected
    } else {
        ButtonState::Idle
    };
    let response =
        icon_button(ui, Icon::ColorPicker, BUTTON_SIDE, ICON_REACH, state).on_hover_text(tooltip);
    if let Some(picker) = tool.and_then(ColorTool::picker_mut) {
        popover::beside(&response, PopupCloseBehavior::IgnoreClicks, |ui| {
            color_picker::show(ui, picker, Parts::ALL)
        });
    }
    response.clicked().then_some(if open {
        Action::CloseColorPicker
    } else {
        Action::OpenColorPicker
    })
}

fn crop_tool(
    ui: &mut Ui,
    availability: Availability,
    open: bool,
    shortcuts: &ShortcutBus,
) -> Option<Action> {
    let tooltip = shortcuts.hinted(ui.ctx(), CROP_TOOLTIP, Action::OpenCrop);
    if !Action::OpenCrop.is_enabled(availability) {
        disabled_tool(ui, Icon::Crop, &tooltip);
        return None;
    }
    let state = if open {
        ButtonState::Selected
    } else {
        ButtonState::Idle
    };
    let response =
        icon_button(ui, Icon::Crop, BUTTON_SIDE, ICON_REACH, state).on_hover_text(tooltip);
    response.clicked().then_some(if open {
        Action::CancelCrop
    } else {
        Action::OpenCrop
    })
}

fn resize_tool(
    ui: &mut Ui,
    availability: Availability,
    draft: Option<&mut ResizeDraft>,
    shortcuts: &ShortcutBus,
) -> Option<Action> {
    let tooltip = shortcuts.hinted(ui.ctx(), RESIZE_TOOLTIP, Action::OpenResize);
    if !Action::OpenResize.is_enabled(availability) {
        disabled_tool(ui, Icon::Resize, &tooltip);
        return None;
    }
    let open = draft.is_some();
    let state = if open {
        ButtonState::Selected
    } else {
        ButtonState::Idle
    };
    let response =
        icon_button(ui, Icon::Resize, BUTTON_SIDE, ICON_REACH, state).on_hover_text(tooltip);
    let from_popover = draft.and_then(|draft| resize_popover::show(&response, draft));
    let from_button = response.clicked().then_some(if open {
        Action::CancelResize
    } else {
        Action::OpenResize
    });
    from_popover.or(from_button)
}

fn two_way_tool(
    ui: &mut Ui,
    icon: Icon,
    tooltip: &str,
    [primary, secondary]: [Action; 2],
    availability: Availability,
) -> Option<Action> {
    if !primary.is_enabled(availability) {
        disabled_tool(ui, icon, tooltip);
        return None;
    }
    let response =
        icon_button(ui, icon, BUTTON_SIDE, ICON_REACH, ButtonState::Idle).on_hover_text(tooltip);
    if response.clicked() {
        Some(primary)
    } else if response.secondary_clicked() {
        Some(secondary)
    } else {
        None
    }
}
