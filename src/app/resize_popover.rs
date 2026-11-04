use eframe::egui::text_edit::TextEditState;
use eframe::egui::text_selection::CCursorRange;
use eframe::egui::{
    Align, Align2, Button, FontId, Layout, Margin, PopupCloseBehavior, Pos2, Rect, Response,
    RichText, Stroke, TextEdit, Ui, UiBuilder, Vec2, text::CCursor,
};

use crate::app::action::Action;
use crate::app::flat_button::{ButtonState, icon_button};
use crate::app::icons::Icon;
use crate::app::popover;
use crate::app::preferences::ResizeUnit;
use crate::app::theme;
use crate::core::decode::check_decoded_size;

const FIELD_SIZE: Vec2 = Vec2::new(60.0, 30.0);
const FIELD_FONT_SIZE: f32 = 14.0;
const FIELD_MARGIN: Margin = Margin::symmetric(8, 5);
const LABEL_WIDTH: f32 = 48.0;
const BUTTON_RIGHT_GAP: f32 = 6.0;
const MAX_DIGITS: usize = 7;
const FULL_PERCENT: usize = 100;
const GAP: f32 = 8.0;
const ROW_GAP: f32 = 14.0;
const LINK_COLUMN_WIDTH: f32 = 28.0;
const BRACKET_LABEL_GAP: f32 = 6.0;
const BRACKET_BUTTON_GAP: f32 = 4.0;
const LINK_BUTTON_SIDE: f32 = 24.0;
const LINK_ICON_REACH: f32 = 7.0;
const BRACKET_WIDTH: f32 = 1.5;
const RESET_BUTTON_SIDE: f32 = 26.0;
const RESET_ICON_REACH: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Incomplete,
    Unchanged,
    TooLarge,
    Ready([usize; 2]),
}

impl Entry {
    fn hint(self) -> Option<&'static str> {
        match self {
            Self::Incomplete => Some("Enter a width and a height."),
            Self::TooLarge => Some("Larger than the decoded size limit."),
            Self::Unchanged | Self::Ready(_) => None,
        }
    }
}

pub struct ResizeDraft {
    original: [usize; 2],
    unit: ResizeUnit,
    width: String,
    height: String,
    keep_ratio: bool,
    focus_pending: bool,
}

impl ResizeDraft {
    pub fn new(size: [usize; 2], unit: ResizeUnit) -> Self {
        let mut draft = Self {
            original: size,
            unit,
            width: String::new(),
            height: String::new(),
            keep_ratio: true,
            focus_pending: true,
        };
        draft.reset();
        draft
    }

    fn baseline(&self) -> [usize; 2] {
        match self.unit {
            ResizeUnit::Pixels => self.original,
            ResizeUnit::Percent => [FULL_PERCENT; 2],
        }
    }

    fn pixels(&self, value: usize, side: usize) -> usize {
        match self.unit {
            ResizeUnit::Pixels => value,
            ResizeUnit::Percent => scaled(side, value, FULL_PERCENT),
        }
    }

    fn unit_label(&self) -> &'static str {
        match self.unit {
            ResizeUnit::Pixels => "px",
            ResizeUnit::Percent => "%",
        }
    }

    pub fn entry(&self) -> Entry {
        let (Some(width), Some(height)) = (parse(&self.width), parse(&self.height)) else {
            return Entry::Incomplete;
        };
        let width = self.pixels(width, self.original[0]);
        let height = self.pixels(height, self.original[1]);
        if [width, height] == self.original {
            Entry::Unchanged
        } else if check_decoded_size(width, height).is_err() {
            Entry::TooLarge
        } else {
            Entry::Ready([width, height])
        }
    }

    fn is_original(&self) -> bool {
        [parse(&self.width), parse(&self.height)] == self.baseline().map(Some)
    }

    fn reset(&mut self) {
        let [width, height] = self.baseline();
        self.width = width.to_string();
        self.height = height.to_string();
    }

    fn follow_width(&mut self) {
        let Some(width) = parse(&self.width) else {
            return;
        };
        self.height = match self.unit {
            ResizeUnit::Pixels => scaled(width, self.original[1], self.original[0]),
            ResizeUnit::Percent => width,
        }
        .to_string();
    }

    fn follow_height(&mut self) {
        let Some(height) = parse(&self.height) else {
            return;
        };
        self.width = match self.unit {
            ResizeUnit::Pixels => scaled(height, self.original[0], self.original[1]),
            ResizeUnit::Percent => height,
        }
        .to_string();
    }
}

pub fn show(anchor: &Response, draft: &mut ResizeDraft) -> Option<Action> {
    let shown = popover::beside(anchor, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        content(ui, draft)
    })?;
    shown.inner.or_else(|| {
        shown
            .response
            .should_close()
            .then_some(Action::CancelResize)
    })
}

struct Edited {
    width: bool,
    height: bool,
    ratio_toggled: bool,
    right: f32,
}

struct Field {
    response: Response,
    right: f32,
}

fn content(ui: &mut Ui, draft: &mut ResizeDraft) -> Option<Action> {
    let edited = size_rows(ui, draft);
    if draft.keep_ratio {
        if edited.height {
            draft.follow_height();
        } else if edited.width || edited.ratio_toggled {
            draft.follow_width();
        }
    }
    ui.set_max_width(ui.min_rect().width());
    let entry = draft.entry();
    ui.add_space(GAP);
    if let Some(hint) = entry.hint() {
        ui.vertical_centered(|ui| ui.label(RichText::new(hint).small().color(theme::text_dim())));
        ui.add_space(GAP);
    }
    buttons(ui, draft, matches!(entry, Entry::Ready(_)), edited.right)
}

fn size_rows(ui: &mut Ui, draft: &mut ResizeDraft) -> Edited {
    ui.horizontal_top(|ui| {
        let column_left = ui.next_widget_position().x;
        ui.add_space(LINK_COLUMN_WIDTH + BRACKET_LABEL_GAP);
        let (width, height) = ui
            .vertical(|ui| {
                ui.spacing_mut().item_spacing.y = ROW_GAP;
                let unit = draft.unit_label();
                let width = field(
                    ui,
                    "Width:",
                    unit,
                    &mut draft.width,
                    &mut draft.focus_pending,
                );
                let height = field(ui, "Height:", unit, &mut draft.height, &mut false);
                (width, height)
            })
            .inner;
        let rows = [
            width.response.rect.center().y,
            height.response.rect.center().y,
        ];
        Edited {
            width: width.response.changed(),
            height: height.response.changed(),
            ratio_toggled: link(ui, column_left, rows, &mut draft.keep_ratio),
            right: width.right.max(height.right),
        }
    })
    .inner
}

fn link(ui: &mut Ui, column_left: f32, [top, bottom]: [f32; 2], keep_ratio: &mut bool) -> bool {
    let spine = column_left + LINK_COLUMN_WIDTH / 2.0;
    let middle = (top + bottom) / 2.0;
    let button_rect =
        Rect::from_center_size(Pos2::new(spine, middle), Vec2::splat(LINK_BUTTON_SIDE));
    let bracket_color = if *keep_ratio {
        theme::text_dim()
    } else {
        theme::icon_disabled()
    };
    let stroke = Stroke::new(BRACKET_WIDTH, bracket_color);
    let arm_end = column_left + LINK_COLUMN_WIDTH;
    let painter = ui.painter();
    painter.line(
        vec![
            Pos2::new(arm_end, top),
            Pos2::new(spine, top),
            Pos2::new(spine, button_rect.top() - BRACKET_BUTTON_GAP),
        ],
        stroke,
    );
    painter.line(
        vec![
            Pos2::new(spine, button_rect.bottom() + BRACKET_BUTTON_GAP),
            Pos2::new(spine, bottom),
            Pos2::new(arm_end, bottom),
        ],
        stroke,
    );
    let state = if *keep_ratio {
        ButtonState::Selected
    } else {
        ButtonState::Idle
    };
    let toggle = ui
        .scope_builder(UiBuilder::new().max_rect(button_rect), |ui| {
            icon_button(ui, Icon::Link, LINK_BUTTON_SIDE, LINK_ICON_REACH, state)
        })
        .inner
        .on_hover_text("Keep aspect ratio");
    if toggle.clicked() {
        *keep_ratio = !*keep_ratio;
    }
    toggle.clicked()
}

fn field(ui: &mut Ui, label: &str, unit: &str, text: &mut String, take_focus: &mut bool) -> Field {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(LABEL_WIDTH, FIELD_SIZE.y),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(LABEL_WIDTH);
                ui.label(label)
            },
        );
        let mut output = TextEdit::singleline(text)
            .font(FontId::proportional(FIELD_FONT_SIZE))
            .margin(FIELD_MARGIN)
            .min_size(FIELD_SIZE)
            .desired_width(FIELD_SIZE.x)
            .align(Align2::LEFT_CENTER)
            .char_limit(MAX_DIGITS)
            .show(ui);
        let unit = ui.label(unit);
        if std::mem::take(take_focus) {
            output.response.request_focus();
            let everything =
                CCursorRange::two(CCursor::new(0), CCursor::new(output.galley.text().len()));
            output.state.cursor.set_char_range(Some(everything));
            TextEditState::store(output.state, ui.ctx(), output.response.id);
        }
        if output.response.changed() {
            text.retain(|character| character.is_ascii_digit());
        }
        Field {
            response: output.response.response,
            right: unit.rect.right(),
        }
    })
    .inner
}

fn buttons(ui: &mut Ui, draft: &mut ResizeDraft, ready: bool, right: f32) -> Option<Action> {
    let reset_state = if draft.is_original() {
        ButtonState::Disabled
    } else {
        ButtonState::Idle
    };
    let reset = icon_button(
        ui,
        Icon::Reset,
        RESET_BUTTON_SIDE,
        RESET_ICON_REACH,
        reset_state,
    )
    .on_hover_text("Reset to the original size");
    if reset.clicked() {
        draft.reset();
    }
    let bounds = Rect::from_min_max(
        Pos2::new(reset.rect.right(), reset.rect.top()),
        Pos2::new(right - BUTTON_RIGHT_GAP, reset.rect.bottom()),
    );
    let mut apply_row = ui.new_child(
        UiBuilder::new()
            .max_rect(bounds)
            .layout(Layout::right_to_left(Align::Center)),
    );
    apply_row
        .add_enabled(ready, Button::new("Resize"))
        .clicked()
        .then_some(Action::ApplyResize)
}

fn parse(text: &str) -> Option<usize> {
    text.parse().ok().filter(|&value| value > 0)
}

fn scaled(value: usize, numerator: usize, denominator: usize) -> usize {
    let product = value as u128 * numerator as u128;
    let half = denominator as u128 / 2;
    usize::try_from((product + half) / denominator as u128)
        .unwrap_or(usize::MAX)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(size: [usize; 2]) -> ResizeDraft {
        ResizeDraft::new(size, ResizeUnit::Pixels)
    }

    #[test]
    fn a_fresh_draft_shows_the_current_size_and_asks_for_nothing() {
        let fresh = draft([640, 480]);
        assert_eq!(
            (fresh.width.as_str(), fresh.height.as_str()),
            ("640", "480")
        );
        assert!(fresh.keep_ratio);
        assert_eq!(fresh.entry(), Entry::Unchanged);
    }

    #[test]
    fn a_changed_size_is_ready_and_bad_entries_are_not() {
        let mut edited = draft([640, 480]);
        edited.width = "320".to_owned();
        edited.height = "240".to_owned();
        assert_eq!(edited.entry(), Entry::Ready([320, 240]));
        edited.width = String::new();
        assert_eq!(edited.entry(), Entry::Incomplete);
        edited.width = "0".to_owned();
        assert_eq!(edited.entry(), Entry::Incomplete);
        edited.width = "40000".to_owned();
        edited.height = "40000".to_owned();
        assert_eq!(edited.entry(), Entry::TooLarge);
    }

    #[test]
    fn the_other_side_follows_the_original_ratio() {
        let mut edited = draft([640, 480]);
        edited.width = "320".to_owned();
        edited.follow_width();
        assert_eq!(edited.height, "240");
        edited.height = "90".to_owned();
        edited.follow_height();
        assert_eq!(edited.width, "120");
    }

    #[test]
    fn reset_restores_the_original_numbers_and_keeps_the_ratio_toggle() {
        let mut edited = draft([640, 480]);
        assert!(edited.is_original());
        edited.width = "320".to_owned();
        edited.height = String::new();
        edited.keep_ratio = false;
        assert!(!edited.is_original());
        edited.reset();
        assert!(edited.is_original());
        assert_eq!(
            (edited.width.as_str(), edited.height.as_str()),
            ("640", "480")
        );
        assert!(!edited.keep_ratio);
    }

    #[test]
    fn a_followed_side_never_drops_below_one_pixel() {
        let mut edited = draft([1000, 10]);
        edited.width = "5".to_owned();
        edited.follow_width();
        assert_eq!(edited.height, "1");
    }

    #[test]
    fn percent_drafts_start_at_one_hundred_and_resolve_to_pixels() {
        let mut edited = ResizeDraft::new([640, 480], ResizeUnit::Percent);
        assert_eq!(
            (edited.width.as_str(), edited.height.as_str()),
            ("100", "100")
        );
        assert_eq!(edited.entry(), Entry::Unchanged);
        edited.width = "50".to_owned();
        edited.follow_width();
        assert_eq!(edited.height, "50");
        assert_eq!(edited.entry(), Entry::Ready([320, 240]));
        edited.reset();
        assert!(edited.is_original());
    }

    #[test]
    fn following_an_empty_field_leaves_the_other_alone() {
        let mut edited = draft([640, 480]);
        edited.width = String::new();
        edited.follow_width();
        assert_eq!(edited.height, "480");
    }
}
