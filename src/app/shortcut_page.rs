use eframe::egui::{
    Align2, Color32, Context, CornerRadius, Event, Id, Key, Label, Pos2, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, TextStyle, Ui, UiBuilder, vec2,
};

use crate::app::commands::{COMMANDS, Command, MAX_SHORTCUTS};
use crate::app::flat_button::{ButtonState, icon_button, outlined_button};
use crate::app::icons::{self, Icon};
use crate::app::shortcut_text;
use crate::app::shortcuts::{BindError, ShortcutBus};
use crate::app::theme;

const ROW_HEIGHT: f32 = 34.0;
const CHIP_HEIGHT: f32 = 24.0;
const CHIP_RADIUS: u8 = 3;
const CHIP_TEXT_INSET: f32 = 10.0;
const CHIP_WIDTH: f32 = 170.0;
const CLEAR_SIDE: f32 = 18.0;
const CLEAR_REACH: f32 = 3.5;
const CLEAR_INSET: f32 = 3.0;
const RESET_SIDE: f32 = 24.0;
const RESET_REACH: f32 = 5.0;
const GAP: f32 = 8.0;
const ROW_INSET: f32 = 10.0;
const END_PADDING: f32 = 32.0;
const SECTION_GAP: f32 = 12.0;
const RESET_ALL_TEXT: &str = "Reset all to defaults";
const RECORDING_TEXT: &str = "Press keys...";
const UNBOUND_TEXT: &str = "Unbound";
const CONTROL_MESSAGE: &str = "Control on its own cannot be used; combine it with another key";
const SLOT_TITLES: [&str; MAX_SHORTCUTS] = ["Primary", "Secondary"];

#[derive(Clone, Copy)]
struct Recording {
    command: &'static Command,
    slot: usize,
}

enum Intent {
    Record(Recording),
    Clear(&'static Command, usize),
    Reset(&'static Command),
    ResetAll,
}

#[derive(Default)]
pub struct ShortcutPage {
    recording: Option<Recording>,
    message: Option<String>,
}

enum Face {
    Bound(String),
    Unbound,
    Recording(String),
}

impl ShortcutPage {
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn cancel(&mut self) {
        self.recording = None;
        self.message = None;
    }

    pub fn show(&mut self, ui: &mut Ui, bus: &mut ShortcutBus) -> bool {
        let captured = self.capture(ui.ctx(), bus);
        if let Some(message) = &self.message {
            let text = RichText::new(message).color(theme::text_error());
            ui.add(Label::new(text).selectable(false));
        }
        let intent = ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.table(ui, bus))
            .inner;
        let clicked_away = self.recording.is_some()
            && intent.is_none()
            && ui.input(|input| input.pointer.primary_clicked());
        if clicked_away {
            self.recording = None;
        }
        let applied = intent.is_some_and(|intent| self.apply(bus, intent));
        captured || applied
    }

    fn capture(&mut self, ctx: &Context, bus: &mut ShortcutBus) -> bool {
        let Some(recording) = self.recording else {
            return false;
        };
        let pressed = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    repeat: false,
                    ..
                } if !shortcut_text::is_modifier_key(*key) => Some((*key, *modifiers)),
                _ => None,
            })
        });
        let Some((key, held)) = pressed else {
            return false;
        };
        self.recording = None;
        if key == Key::Escape && held.is_none() {
            return false;
        }
        let Some(shortcut) = shortcut_text::from_press(key, held) else {
            self.message = Some(CONTROL_MESSAGE.to_owned());
            return false;
        };
        let action = recording.command.action;
        let mut list: Vec<_> = bus.shortcuts_of(action).collect();
        match list.get_mut(recording.slot) {
            Some(bound) => *bound = shortcut,
            None => list.push(shortcut),
        }
        let outcome = bus.set(action, &list);
        self.settle(outcome, || format!("{} is", ctx.format_shortcut(&shortcut)))
    }

    fn apply(&mut self, bus: &mut ShortcutBus, intent: Intent) -> bool {
        self.recording = None;
        self.message = None;
        match intent {
            Intent::Record(recording) => {
                self.recording = Some(recording);
                false
            }
            Intent::Clear(command, slot) => {
                let mut list: Vec<_> = bus.shortcuts_of(command.action).collect();
                if slot < list.len() {
                    list.remove(slot);
                }
                let outcome = bus.set(command.action, &list);
                self.settle(outcome, || String::from("Cannot clear:"))
            }
            Intent::Reset(command) => {
                let outcome = bus.reset(command.action);
                self.settle(outcome, || {
                    format!("Cannot restore {}, a default is", command.action.label())
                })
            }
            Intent::ResetAll => {
                let outcome = ShortcutBus::with_defaults().map(|defaults| *bus = defaults);
                self.settle(outcome, || String::from("Cannot restore defaults:"))
            }
        }
    }

    fn settle(&mut self, outcome: Result<(), BindError>, lead: impl FnOnce() -> String) -> bool {
        match outcome {
            Ok(()) => {
                self.message = None;
                true
            }
            Err(error) => {
                self.message = Some(format!("{} {error}", lead()));
                false
            }
        }
    }

    fn table(&self, ui: &mut Ui, bus: &ShortcutBus) -> Option<Intent> {
        ui.spacing_mut().item_spacing.y = 0.0;
        header(ui);
        let live = ui
            .ctx()
            .input(|input| shortcut_text::held_prefix(input.modifiers))
            .unwrap_or_else(|| RECORDING_TEXT.to_owned());
        let mut intent = None;
        for (index, command) in COMMANDS.iter().enumerate() {
            let found = self.row(ui, bus, command, index, &live);
            intent = intent.or(found);
        }
        ui.add_space(SECTION_GAP);
        if outlined_button(ui, RESET_ALL_TEXT).clicked() {
            intent = intent.or(Some(Intent::ResetAll));
        }
        ui.add_space(END_PADDING);
        intent
    }

    fn row(
        &self,
        ui: &mut Ui,
        bus: &ShortcutBus,
        command: &'static Command,
        index: usize,
        live: &str,
    ) -> Option<Intent> {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
        let fill = if ui.rect_contains_pointer(rect) {
            Some(theme::pressed_fill())
        } else {
            (index % 2 == 1).then_some(theme::row_stripe())
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(rect, 0.0, fill);
        }
        let cells = Cells::in_row(rect);
        ui.painter().with_clip_rect(cells.action).text(
            cells.action.left_center(),
            Align2::LEFT_CENTER,
            command.action.label(),
            TextStyle::Body.resolve(ui.style()),
            theme::icon_idle(),
        );
        let bound: Vec<_> = bus.shortcuts_of(command.action).collect();
        let mut intent = None;
        for (slot, cell) in cells.slots.into_iter().enumerate() {
            let recording = self
                .recording
                .is_some_and(|active| active.command.id == command.id && active.slot == slot);
            let face = match (recording, bound.get(slot)) {
                (true, _) => Face::Recording(live.to_owned()),
                (false, Some(shortcut)) => Face::Bound(ui.ctx().format_shortcut(shortcut)),
                (false, None) => Face::Unbound,
            };
            let id = ui.scope_id().with((command.id, slot));
            let found = keycap(ui, id, cell, &face).map(|clicked| match clicked {
                Clicked::Body => Intent::Record(Recording { command, slot }),
                Clicked::Clear => Intent::Clear(command, slot),
            });
            intent = intent.or(found);
        }
        if !bus.is_default(command) {
            let mut cell = ui.new_child(UiBuilder::new().max_rect(cells.reset));
            let reset = icon_button(
                &mut cell,
                Icon::Reset,
                RESET_SIDE,
                RESET_REACH,
                ButtonState::Idle,
            );
            if reset.on_hover_text("Restore default").clicked() {
                intent = intent.or(Some(Intent::Reset(command)));
            }
        }
        intent
    }
}

struct Cells {
    action: Rect,
    slots: [Rect; MAX_SHORTCUTS],
    reset: Rect,
}

impl Cells {
    fn in_row(row: Rect) -> Self {
        let inner = row.shrink2(vec2(ROW_INSET, 0.0));
        let centre = inner.center().y;
        let reset = Rect::from_center_size(
            Pos2::new(inner.right() - RESET_SIDE / 2.0, centre),
            vec2(RESET_SIDE, RESET_SIDE),
        );
        let slot_count = MAX_SHORTCUTS as f32;
        let slots_width = slot_count * CHIP_WIDTH + (slot_count - 1.0) * GAP;
        let slots_left = reset.left() - GAP - slots_width;
        let slots = std::array::from_fn(|slot| {
            Rect::from_center_size(
                Pos2::new(
                    slots_left + CHIP_WIDTH / 2.0 + slot as f32 * (CHIP_WIDTH + GAP),
                    centre,
                ),
                vec2(CHIP_WIDTH, CHIP_HEIGHT),
            )
        });
        let action_right = (slots_left - GAP).max(inner.left());
        Self {
            action: Rect::from_min_max(inner.min, Pos2::new(action_right, inner.bottom())),
            slots,
            reset,
        }
    }
}

enum Clicked {
    Body,
    Clear,
}

fn keycap(ui: &mut Ui, id: Id, rect: Rect, face: &Face) -> Option<Clicked> {
    let body = ui.interact(rect, id.with("body"), Sense::click());
    let clear_rect = Rect::from_center_size(
        Pos2::new(
            rect.right() - CLEAR_INSET - CLEAR_SIDE / 2.0,
            rect.center().y,
        ),
        vec2(CLEAR_SIDE, CLEAR_SIDE),
    );
    let clear = matches!(face, Face::Bound(_))
        .then(|| ui.interact(clear_rect, id.with("clear"), Sense::click()));
    let painter = ui.painter();
    let (fill, stroke, text, color) = match face {
        Face::Bound(text) => (
            if body.hovered() {
                theme::hover_fill()
            } else {
                theme::viewer_fill()
            },
            Stroke::new(1.0, theme::hover_fill()),
            text.as_str(),
            theme::icon_idle(),
        ),
        Face::Unbound => (
            if body.hovered() {
                theme::hover_fill()
            } else {
                Color32::TRANSPARENT
            },
            Stroke::new(1.0, theme::pressed_fill()),
            UNBOUND_TEXT,
            theme::icon_disabled(),
        ),
        Face::Recording(text) => (
            theme::selected_fill(),
            Stroke::new(1.0, theme::drop_hover_stroke()),
            text.as_str(),
            theme::icon_selected(),
        ),
    };
    let radius = CornerRadius::same(CHIP_RADIUS);
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
    painter.text(
        Pos2::new(rect.left() + CHIP_TEXT_INSET, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        TextStyle::Body.resolve(ui.style()),
        color,
    );
    if let Some(clear) = &clear {
        if clear.hovered() {
            painter.rect_filled(clear_rect, radius, theme::pressed_fill());
        }
        let tone = if clear.hovered() {
            theme::icon_selected()
        } else {
            theme::text_dim()
        };
        icons::paint(painter, clear_rect.center(), CLEAR_REACH, Icon::Close, tone);
    }
    let clear_clicked = clear.is_some_and(|clear| clear.on_hover_text("Clear").clicked());
    if clear_clicked {
        Some(Clicked::Clear)
    } else {
        body.clicked().then_some(Clicked::Body)
    }
}

fn header(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let cells = Cells::in_row(rect);
    let font = TextStyle::Body.resolve(ui.style());
    let titles = [("Action", cells.action.left_center())].into_iter().chain(
        SLOT_TITLES.iter().zip(cells.slots).map(|(title, cell)| {
            (
                *title,
                Pos2::new(cell.left() + CHIP_TEXT_INSET, cell.center().y),
            )
        }),
    );
    for (title, at) in titles {
        ui.painter().text(
            at,
            Align2::LEFT_CENTER,
            title,
            font.clone(),
            theme::text_dim(),
        );
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, theme::hover_fill()),
    );
}
