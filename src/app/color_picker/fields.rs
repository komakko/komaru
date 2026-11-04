use eframe::egui::text_edit::{TextEditOutput, TextEditState};
use eframe::egui::text_selection::CCursorRange;
use eframe::egui::{
    Align, Align2, FontId, Id, Layout, Margin, RichText, TextEdit, Ui, Vec2, text::CCursor,
};

use super::{GAP, surface};
use crate::app::theme;
use crate::core::rgba::Rgba;

pub const NUMBER_WIDTH: f32 = (surface::FULL_WIDTH - 3.0 * GAP) / 4.0;
pub const FIELD_HEIGHT: f32 = 26.0;
const LABEL_HEIGHT: f32 = 18.0;
const FONT_SIZE: f32 = 13.0;
const MARGIN: Margin = Margin::symmetric(4, 4);
const NUMBER_DIGITS: usize = 3;
const HEX_CHARACTERS: usize = 9;

pub struct Draft {
    id: Id,
    text: String,
}

pub fn number(
    ui: &mut Ui,
    draft: &mut Option<Draft>,
    label: &str,
    value: u32,
    max: u32,
) -> Option<u32> {
    let id = ui.scope_id().with(label);
    let size = Vec2::new(NUMBER_WIDTH, FIELD_HEIGHT + LABEL_HEIGHT);
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Center), |ui| {
        ui.label(RichText::new(label).small().color(theme::text_dim()));
        let spec = Spec {
            width: NUMBER_WIDTH,
            limit: NUMBER_DIGITS,
            allowed: |character| character.is_ascii_digit(),
        };
        edit(ui, draft, id, value.to_string(), &spec)
    })
    .inner
    .and_then(|text| text.parse::<u32>().ok())
    .map(|parsed| parsed.min(max))
}

pub fn hex(ui: &mut Ui, draft: &mut Option<Draft>, shown: Rgba, width: f32) -> Option<Rgba> {
    let id = ui.scope_id().with("hex");
    let spec = Spec {
        width,
        limit: HEX_CHARACTERS,
        allowed: |character| character == '#' || character.is_ascii_hexdigit(),
    };
    edit(ui, draft, id, shown.hex(), &spec).and_then(|text| Rgba::parse_hex(&text))
}

struct Spec {
    width: f32,
    limit: usize,
    allowed: fn(char) -> bool,
}

fn edit(
    ui: &mut Ui,
    draft: &mut Option<Draft>,
    id: Id,
    shown: String,
    spec: &Spec,
) -> Option<String> {
    let mut text = match draft {
        Some(open) if open.id == id => open.text.clone(),
        _ => shown,
    };
    let TextEditOutput {
        response,
        galley,
        mut state,
        ..
    } = TextEdit::singleline(&mut text)
        .id(id)
        .font(FontId::proportional(FONT_SIZE))
        .margin(MARGIN)
        .min_size(Vec2::new(spec.width, FIELD_HEIGHT))
        .desired_width(spec.width)
        .align(Align2::CENTER_CENTER)
        .char_limit(spec.limit)
        .show(ui);
    if response.gained_focus() {
        let everything = CCursorRange::two(CCursor::new(0), CCursor::new(galley.text().len()));
        state.cursor.set_char_range(Some(everything));
        TextEditState::store(state, ui.ctx(), id);
    }
    let edited = response.changed().then(|| {
        text.retain(spec.allowed);
        text
    });
    match &edited {
        Some(text) => {
            *draft = Some(Draft {
                id,
                text: text.clone(),
            });
        }
        None if !response.has_focus() && draft.as_ref().is_some_and(|open| open.id == id) => {
            *draft = None;
        }
        None => {}
    }
    edited
}
