use std::time::Duration;

use eframe::egui::containers::menu::{MenuButton, MenuConfig, SubMenu, SubMenuButton};
use eframe::egui::{Button, CornerRadius, Margin, Response, RichText, Stroke, Style, Ui, vec2};

use crate::app::action::{Action, Availability};
use crate::app::pin::PinView;
use crate::app::shortcuts::ShortcutBus;
use crate::app::theme;
use crate::core::recents::Recents;

const DROPDOWN_WIDTH: f32 = 240.0;
const DROPDOWN_MARGIN: i8 = 3;
const DROPDOWN_RADIUS: u8 = 3;
const ITEM_PADDING: [f32; 2] = [12.0, 4.0];
const STUB_TOOLTIP: &str = "Not implemented yet";
const RECENT_HOVER_DELAY: f64 = 0.3;
const RECENT_MIN_WIDTH: f32 = 120.0;
const RECENT_MAX_WIDTH: f32 = 320.0;
const RECENT_EDGE_SPACE: f32 = 24.0;

#[derive(Clone, Copy, Default)]
struct RecentHover {
    since: Option<f64>,
    armed: bool,
    frame: u64,
}

pub struct MenuView<'a> {
    pub availability: Availability,
    pub pixelated: bool,
    pub pin: &'a PinView,
    pub shortcuts: &'a ShortcutBus,
    pub recents: &'a Recents,
}

struct Dropdown<'a> {
    ui: &'a mut Ui,
    chosen: Option<Action>,
    availability: Availability,
    shortcuts: &'a ShortcutBus,
}

impl Dropdown<'_> {
    fn button<'b>(&self, label: &'b str, action: Action) -> Button<'b> {
        let button = Button::new(label);
        match self.shortcuts.hint(self.ui.ctx(), action) {
            Some(hint) => button.shortcut_text(hint),
            None => button,
        }
    }

    fn command(&mut self, label: &str, action: Action) {
        let enabled = action.is_enabled(self.availability);
        let button = self.button(label, action);
        let response = self.ui.add_enabled(enabled, button);
        if response.clicked() {
            self.chosen = Some(action);
        }
    }

    fn toggle(&mut self, label: &str, action: Action, on: bool, unavailable: Option<&str>) {
        let enabled = action.is_enabled(self.availability);
        let button = self.button(label, action).selected(on);
        let response = self.ui.add_enabled(enabled, button);
        if response.clicked() {
            self.chosen = Some(action);
        }
        if let Some(reason) = unavailable {
            response.on_disabled_hover_text(reason);
        }
    }

    fn recent_files(&mut self, recents: &Recents) {
        let enabled = !recents.items().is_empty();
        let chosen = self
            .ui
            .add_enabled_ui(enabled, |ui| open_recent(ui, recents))
            .inner;
        if let Some(chosen) = chosen {
            self.chosen = Some(chosen);
            self.ui.close();
        }
    }

    fn stub(&mut self, label: &str) {
        self.ui
            .add_enabled(false, Button::new(label))
            .on_disabled_hover_text(STUB_TOOLTIP);
    }

    fn separator(&mut self) {
        self.ui.separator();
    }
}

fn open_recent(ui: &mut Ui, recents: &Recents) -> Option<Action> {
    let id = ui.next_auto_id();
    let frame = ui.ctx().cumulative_frame_nr();
    let mut hover: RecentHover = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    if hover.frame + 1 < frame {
        hover = RecentHover::default();
    }
    hover.frame = frame;
    let response = recent_button(ui, hover.armed);
    hover.track(ui, &response);
    let mut chosen = None;
    if hover.armed {
        match SubMenu::new().show(ui, &response, |ui| recent_list(ui, recents)) {
            Some(shown) => chosen = shown.inner,
            None => {
                hover = RecentHover {
                    frame,
                    ..RecentHover::default()
                }
            }
        }
    }
    ui.data_mut(|data| data.insert_temp(id, hover));
    chosen
}

fn recent_button(ui: &mut Ui, open: bool) -> Response {
    let inactive = ui.style().visuals.widgets.inactive;
    if open {
        ui.style_mut().visuals.widgets.inactive = ui.style().visuals.widgets.open;
    }
    let button = Button::new("Open Recent").right_text(SubMenuButton::arrow_atom(None));
    let response = ui.add(button);
    ui.style_mut().visuals.widgets.inactive = inactive;
    response
}

impl RecentHover {
    fn track(&mut self, ui: &Ui, response: &Response) {
        let now = ui.input(|input| input.time);
        if response.hovered() {
            self.since.get_or_insert(now);
        } else {
            self.since = None;
        }
        if self.armed {
            return;
        }
        let waited = self.since.map_or(0.0, |since| now - since);
        if waited >= RECENT_HOVER_DELAY || response.clicked() || response.has_focus() {
            self.armed = true;
        } else if self.since.is_some() {
            let remaining = Duration::from_secs_f64(RECENT_HOVER_DELAY - waited);
            ui.ctx().request_repaint_after(remaining);
        }
    }
}

fn recent_list(ui: &mut Ui, recents: &Recents) -> Option<Action> {
    let room = ui.ctx().content_rect().width() - DROPDOWN_WIDTH - RECENT_EDGE_SPACE;
    ui.set_max_width(room.clamp(RECENT_MIN_WIDTH, RECENT_MAX_WIDTH));
    let mut chosen = None;
    for (index, recent) in recents.items().iter().enumerate() {
        let item = ui
            .add(Button::new(RichText::new(recent.title())).truncate())
            .on_hover_text(recent.location());
        if item.clicked() {
            chosen = Some(Action::OpenRecent(index));
        }
    }
    ui.separator();
    if ui.button("Clear List").clicked() {
        chosen = Some(Action::ClearRecents);
    }
    chosen
}

pub fn dropdown_config() -> MenuConfig {
    MenuConfig::new().style(dropdown_style)
}

pub fn show(ui: &mut Ui, view: &MenuView) -> Option<Action> {
    let availability = view.availability;
    let file = dropdown(ui, "File", availability, view.shortcuts, |menu| {
        menu.command("Open...", Action::Open);
        menu.recent_files(view.recents);
        menu.stub("Take Screenshot");
        menu.command("Save As...", Action::SaveAs);
        menu.separator();
        menu.command("Settings...", Action::OpenSettings);
        menu.separator();
        menu.command("Quit", Action::Quit);
    });
    let edit = dropdown(ui, "Edit", availability, view.shortcuts, |menu| {
        menu.command("Undo", Action::Undo);
        menu.command("Redo", Action::Redo);
        menu.separator();
        menu.command("Copy Image", Action::CopyImage);
        menu.command("Paste", Action::Paste);
        menu.command("Clear", Action::Clear);
    });
    let view_menu = dropdown(ui, "View", availability, view.shortcuts, |menu| {
        menu.command("Fit to Window", Action::FitToWindow);
        menu.command("Actual Size", Action::ActualSize);
        menu.separator();
        menu.toggle(
            "Sharp Pixels",
            Action::TogglePixelated,
            view.pixelated,
            None,
        );
        menu.toggle(
            "Always on Top",
            Action::TogglePin,
            matches!(view.pin, PinView::On),
            unavailable_reason(view.pin),
        );
        menu.separator();
        menu.command("Play or Pause Animation", Action::TogglePlayback);
        menu.command("Previous Frame", Action::PreviousFrame);
        menu.command("Next Frame", Action::NextFrame);
    });
    file.or(edit).or(view_menu)
}

fn dropdown(
    ui: &mut Ui,
    title: &str,
    availability: Availability,
    shortcuts: &ShortcutBus,
    content: impl FnOnce(&mut Dropdown),
) -> Option<Action> {
    let (_, opened) = MenuButton::new(title).ui(ui, |ui| {
        ui.set_min_width(DROPDOWN_WIDTH);
        let mut menu = Dropdown {
            ui,
            chosen: None,
            availability,
            shortcuts,
        };
        content(&mut menu);
        menu.chosen
    });
    opened.and_then(|shown| shown.inner)
}

fn unavailable_reason(pin: &PinView) -> Option<&str> {
    match pin {
        PinView::Unavailable(reason) => Some(reason.as_str()),
        PinView::On | PinView::Off => None,
    }
}

fn dropdown_style(style: &mut Style) {
    style.spacing.button_padding = vec2(ITEM_PADDING[0], ITEM_PADDING[1]);
    style.spacing.item_spacing = vec2(0.0, 0.0);
    style.spacing.menu_margin = Margin::same(DROPDOWN_MARGIN);
    let visuals = &mut style.visuals;
    visuals.window_fill = theme::panel_fill();
    visuals.window_stroke = Stroke::new(1.0, theme::hover_fill());
    visuals.menu_corner_radius = CornerRadius::same(DROPDOWN_RADIUS);
    theme::flat_item_style(style);
}
