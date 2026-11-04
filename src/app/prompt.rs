use eframe::egui::{
    Align2, Area, Button, Context, CornerRadius, Frame, Id, Label, Margin, Order, Pos2, Rect,
    Stroke,
};

use crate::app::theme;

const BAR_ID: &str = "prompt_bar";
const BAR_BOTTOM_GAP: f32 = 16.0;
const BAR_RADIUS: u8 = 3;
const BAR_MARGIN: Margin = Margin::symmetric(10, 6);
const BAR_GAP: f32 = 8.0;
const MAX_TEXT_WIDTH: f32 = 420.0;

pub struct Question {
    pub text: String,
    pub accept: &'static str,
    pub decline: &'static str,
}

pub fn show(ctx: &Context, viewer: Rect, question: &Question) -> Option<bool> {
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
                .show(ui, |ui| {
                    theme::flat_item_style(ui.style_mut());
                    ui.spacing_mut().item_spacing.x = BAR_GAP;
                    ui.horizontal(|ui| {
                        ui.scope(|ui| {
                            ui.set_max_width(MAX_TEXT_WIDTH.min(viewer.width() / 2.0));
                            ui.add(Label::new(question.text.as_str()).selectable(false));
                        });
                        let declined = ui.add(Button::new(question.decline)).clicked();
                        let accepted = ui.add(Button::new(question.accept)).clicked();
                        if accepted {
                            Some(true)
                        } else if declined {
                            Some(false)
                        } else {
                            None
                        }
                    })
                    .inner
                })
                .inner
        })
        .inner
}
