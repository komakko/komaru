use eframe::egui::{
    InnerResponse, Popup, PopupCloseBehavior, RectAlign, Response, Stroke, Style, Ui, Vec2,
};

use crate::app::theme;

const GAP: f32 = 6.0;
const BUTTON_PADDING: Vec2 = Vec2::new(12.0, 4.0);

pub fn beside<R>(
    anchor: &Response,
    close_behavior: PopupCloseBehavior,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>> {
    at(anchor, RectAlign::RIGHT_START, close_behavior, content)
}

pub fn at<R>(
    anchor: &Response,
    align: RectAlign,
    close_behavior: PopupCloseBehavior,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>> {
    Popup::from_response(anchor)
        .open(true)
        .align(align)
        .gap(GAP)
        .close_behavior(close_behavior)
        .style(style)
        .show(content)
}

fn style(style: &mut Style) {
    theme::flat_item_style(style);
    let visuals = &mut style.visuals;
    visuals.window_fill = theme::panel_fill();
    visuals.window_stroke = Stroke::new(1.0, theme::hover_fill());
    visuals.extreme_bg_color = theme::viewer_fill();
    style.spacing.button_padding = BUTTON_PADDING;
}
