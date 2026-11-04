use eframe::egui::{
    Grid, Label, PopupCloseBehavior, RectAlign, Response, RichText, ScrollArea, Ui,
};

use crate::app::popover;
use crate::app::status_bar::{ImageStatus, format_size};
use crate::app::theme;

const WIDTH: f32 = 380.0;
const LABEL_WIDTH: f32 = 130.0;
const MAX_HEIGHT: f32 = 380.0;
const COLUMN_GAP: f32 = 16.0;
const ROW_GAP: f32 = 3.0;
const SECTION_GAP: f32 = 8.0;

pub fn show(anchor: &Response, status: &ImageStatus) -> bool {
    popover::at(
        anchor,
        RectAlign::TOP_END,
        PopupCloseBehavior::CloseOnClickOutside,
        |ui| content(ui, status),
    )
    .is_some_and(|shown| shown.response.should_close())
}

fn content(ui: &mut Ui, status: &ImageStatus) {
    ui.set_width(WIDTH);
    ScrollArea::vertical()
        .max_height(MAX_HEIGHT)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            table(ui, "image_info_summary", &summary(status));
            if !status.info.metadata.is_empty() {
                ui.add_space(SECTION_GAP);
                let heading = RichText::new("Metadata").color(theme::icon_selected());
                ui.add(Label::new(heading).selectable(false));
                ui.add_space(ROW_GAP);
                table(ui, "image_info_metadata", &status.info.metadata);
            }
        });
}

fn summary(status: &ImageStatus) -> Vec<(String, String)> {
    let info = &status.info;
    let rows = [
        ("Format", info.format.map(str::to_owned)),
        (
            "Dimensions",
            Some(format!("{} x {} px", status.width, status.height)),
        ),
        ("File size", status.file_size.map(format_size)),
        ("Color", info.color.clone()),
        (
            "Color profile",
            Some(
                info.profile
                    .clone()
                    .unwrap_or_else(|| "None (treated as sRGB)".to_owned()),
            ),
        ),
        ("Orientation", info.orientation.map(str::to_owned)),
    ];
    rows.into_iter()
        .filter_map(|(label, value)| Some((label.to_owned(), value?)))
        .collect()
}

fn table(ui: &mut Ui, id: &str, rows: &[(String, String)]) {
    Grid::new(id)
        .num_columns(2)
        .min_col_width(LABEL_WIDTH)
        .spacing([COLUMN_GAP, ROW_GAP])
        .show(ui, |ui| {
            for (label, value) in rows {
                let label = RichText::new(label).color(theme::text_dim());
                ui.scope(|ui| {
                    ui.set_max_width(LABEL_WIDTH);
                    ui.add(Label::new(label).selectable(false));
                });
                let value = RichText::new(value).color(theme::icon_idle());
                ui.add(Label::new(value).wrap());
                ui.end_row();
            }
        });
}
