use eframe::egui::containers::menu::{MenuButton, MenuConfig, SubMenuButton};
use eframe::egui::{Button, CornerRadius, Margin, Stroke, Style, Ui, vec2};

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
        let response = self.ui.add_enabled_ui(!recents.items().is_empty(), |ui| {
            SubMenuButton::new("Open Recent").ui(ui, |ui| {
                let mut chosen = None;
                for (index, recent) in recents.items().iter().enumerate() {
                    let item = ui.button(recent.title()).on_hover_text(recent.location());
                    if item.clicked() {
                        chosen = Some(Action::OpenRecent(index));
                    }
                }
                ui.separator();
                if ui.button("Clear List").clicked() {
                    chosen = Some(Action::ClearRecents);
                }
                chosen
            })
        });
        if let Some(chosen) = response.inner.1.and_then(|shown| shown.inner) {
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
