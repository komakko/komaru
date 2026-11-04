use eframe::egui::{
    Align, ComboBox, CornerRadius, DragValue, Label, Layout, PopupCloseBehavior, Pos2, Rect,
    RectAlign, Response, RichText, Sense, Stroke, StrokeKind, TextStyle, Ui, UiBuilder, Vec2, vec2,
};

use crate::app::color_picker::paint::color32;
use crate::app::color_picker::{self, ColorPicker, Parts};
use crate::app::popover;
use crate::app::settings::Category;
use crate::app::settings_entries::{Control, ENTRIES, Entry};
use crate::app::theme;
use crate::core::rgba::Rgba;

const CONTROL_WIDTH: f32 = 180.0;
const ROW_MIN_HEIGHT: f32 = 36.0;
const ROW_PADDING: f32 = 5.0;
const ROW_INSET: f32 = 10.0;
const COLUMN_GAP: f32 = 16.0;
const LINE_GAP: f32 = 1.0;
const TOGGLE_SIZE: Vec2 = vec2(32.0, 16.0);
const TOGGLE_KNOB_INSET: f32 = 3.0;
const SWATCH_SIZE: Vec2 = vec2(40.0, 20.0);
const SWATCH_GAP: f32 = 8.0;
const PICKER_PARTS: Parts = Parts {
    alpha: false,
    ..Parts::ALL
};

#[derive(Default)]
pub struct ColorEditor {
    open: Option<(usize, ColorPicker)>,
}

impl ColorEditor {
    pub fn close(&mut self) {
        self.open = None;
    }

    fn toggle(&mut self, index: usize, seed: Rgba) {
        let already_open = matches!(&self.open, Some((open, _)) if *open == index);
        self.open = (!already_open).then(|| (index, ColorPicker::new(Some(seed))));
    }

    fn picker(&mut self, index: usize) -> Option<&mut ColorPicker> {
        match &mut self.open {
            Some((open, picker)) if *open == index => Some(picker),
            _ => None,
        }
    }
}

pub fn rows(
    ui: &mut Ui,
    category: Category,
    controls: &mut [Control],
    editor: &mut ColorEditor,
) -> bool {
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut changed = false;
    let page = ENTRIES
        .iter()
        .zip(controls)
        .enumerate()
        .filter(|(_, (entry, _))| entry.category == category)
        .enumerate();
    for (position, (index, (entry, control))) in page {
        changed |= row(ui, position, index, entry, control, editor);
    }
    changed
}

fn row(
    ui: &mut Ui,
    position: usize,
    index: usize,
    entry: &Entry,
    control: &mut Control,
    editor: &mut ColorEditor,
) -> bool {
    let width = ui.available_width();
    let text_width = (width - CONTROL_WIDTH - COLUMN_GAP - 2.0 * ROW_INSET).max(0.0);
    let painter = ui.painter();
    let label = painter.layout_no_wrap(
        entry.label.to_owned(),
        TextStyle::Body.resolve(ui.style()),
        theme::icon_idle(),
    );
    let hint = painter.layout(
        entry.hint.to_owned(),
        TextStyle::Small.resolve(ui.style()),
        theme::text_dim(),
        text_width,
    );
    let label_height = label.size().y;
    let text_height = label_height + LINE_GAP + hint.size().y;
    let height = (text_height + 2.0 * ROW_PADDING).max(ROW_MIN_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let fill = if ui.rect_contains_pointer(rect) {
        Some(theme::pressed_fill())
    } else {
        (position % 2 == 1).then_some(theme::row_stripe())
    };
    let left = rect.left() + ROW_INSET;
    let top = rect.center().y - text_height / 2.0;
    let painter = ui.painter();
    if let Some(fill) = fill {
        painter.rect_filled(rect, 0.0, fill);
    }
    painter.galley(Pos2::new(left, top), label, theme::icon_idle());
    painter.galley(
        Pos2::new(left, top + label_height + LINE_GAP),
        hint,
        theme::text_dim(),
    );
    let cell = Rect::from_min_max(
        Pos2::new(rect.right() - ROW_INSET - CONTROL_WIDTH, rect.top()),
        Pos2::new(rect.right() - ROW_INSET, rect.bottom()),
    );
    let mut cell = ui.new_child(
        UiBuilder::new()
            .max_rect(cell)
            .layout(Layout::right_to_left(Align::Center)),
    );
    let before = control.clone();
    show_control(&mut cell, index, control, editor);
    *control != before
}

fn show_control(ui: &mut Ui, index: usize, control: &mut Control, editor: &mut ColorEditor) {
    match control {
        Control::Toggle(on) => toggle(ui, on),
        Control::Choice { options, selected } => choice(ui, index, options, selected),
        Control::Number {
            value,
            min,
            max,
            suffix,
        } => {
            ui.add(DragValue::new(value).range(*min..=*max).suffix(*suffix));
        }
        Control::Color(color) => color_control(ui, index, color, editor),
    }
}

fn choice(ui: &mut Ui, index: usize, options: &[&'static str], selected: &mut usize) {
    let current = options.get(*selected).copied().unwrap_or_default();
    ComboBox::from_id_salt(("setting", index))
        .width(CONTROL_WIDTH)
        .selected_text(current)
        .popup_style(theme::field_style.into())
        .show_ui(ui, |ui| {
            for (position, option) in options.iter().enumerate() {
                ui.selectable_value(selected, position, *option);
            }
        });
}

fn color_control(ui: &mut Ui, index: usize, color: &mut Rgba, editor: &mut ColorEditor) {
    let response = swatch(ui, *color);
    ui.add_space(SWATCH_GAP);
    let hex = RichText::new(color.hex()).color(theme::text_dim());
    ui.add(Label::new(hex).selectable(false));
    let toggled = response.clicked();
    if toggled {
        editor.toggle(index, *color);
    }
    let Some(picker) = editor.picker(index) else {
        return;
    };
    let shown = popover::at(
        &response,
        RectAlign::LEFT_START,
        PopupCloseBehavior::CloseOnClickOutside,
        |ui| color_picker::show(ui, picker, PICKER_PARTS),
    );
    let Some(shown) = shown else {
        return;
    };
    if let Some(picked) = shown.inner {
        *color = picked;
    }
    if shown.response.should_close() && !toggled {
        editor.close();
    }
}

fn swatch(ui: &mut Ui, color: Rgba) -> Response {
    let (rect, response) = ui.allocate_exact_size(SWATCH_SIZE, Sense::click());
    let edge = if response.hovered() {
        theme::text_dim()
    } else {
        theme::hover_fill()
    };
    let radius = CornerRadius::same(2);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, color32(color));
    painter.rect_stroke(rect, radius, Stroke::new(1.0, edge), StrokeKind::Inside);
    response
}

fn toggle(ui: &mut Ui, on: &mut bool) {
    let (rect, response) = ui.allocate_exact_size(TOGGLE_SIZE, Sense::click());
    if response.clicked() {
        *on = !*on;
    }
    let fill = match (*on, response.hovered()) {
        (true, _) => theme::selected_fill(),
        (false, true) => theme::hover_fill(),
        (false, false) => theme::viewer_fill(),
    };
    let radius = rect.height() / 2.0;
    let knob_x = if *on {
        rect.right() - radius
    } else {
        rect.left() + radius
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(radius as u8), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(radius as u8),
        Stroke::new(1.0, theme::hover_fill()),
        StrokeKind::Inside,
    );
    painter.circle_filled(
        Pos2::new(knob_x, rect.center().y),
        radius - TOGGLE_KNOB_INSET,
        if *on {
            theme::icon_selected()
        } else {
            theme::text_dim()
        },
    );
}
