use eframe::egui::{
    Align, Area, Button, Context, Id, Label, Layout, Order, Rect, RichText, ScrollArea, Sense,
    Stroke, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::app::flat_button::{ButtonState, icon_button, outlined_button};
use crate::app::icons::Icon;
use crate::app::settings_entries::{self, Control};
use crate::app::settings_page::{self, ColorEditor};
use crate::app::shortcut_page::ShortcutPage;
use crate::app::shortcuts::ShortcutBus;
use crate::app::theme;
use crate::identity;

const NAV_WIDTH: f32 = 180.0;
const NAV_PADDING: f32 = 8.0;
const NAV_TEXT_INSET: f32 = 10.0;
const NAV_ITEM_HEIGHT: f32 = 26.0;
const CONTENT_PADDING: f32 = 16.0;
const CLOSE_SIDE: f32 = 24.0;
const CLOSE_REACH: f32 = 6.0;
const TITLE_SIZE: f32 = 17.0;
const ROW_SPACING: f32 = 6.0;
const SECTION_GAP: f32 = 12.0;
const UPDATES_STUB: &str = "Not implemented yet. Updating arrives with distribution.";
const RESET_WARNING: &str =
    "Restores every setting on these pages. Keyboard shortcuts have their own reset.";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    General,
    Appearance,
    Viewing,
    Editing,
    Saving,
    ColorPicker,
    Limits,
    Updates,
    Shortcuts,
    About,
}

impl Category {
    pub const ALL: [Self; 10] = [
        Self::General,
        Self::Appearance,
        Self::Viewing,
        Self::Editing,
        Self::Saving,
        Self::ColorPicker,
        Self::Limits,
        Self::Updates,
        Self::Shortcuts,
        Self::About,
    ];

    pub fn section(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Viewing => "viewing",
            Self::Editing => "editing",
            Self::Saving => "saving",
            Self::ColorPicker => "color_picker",
            Self::Limits => "limits",
            Self::Updates => "updates",
            Self::Shortcuts => "shortcuts",
            Self::About => "about",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Viewing => "Viewing",
            Self::Editing => "Editing",
            Self::Saving => "Saving and Copying",
            Self::ColorPicker => "Color Picker",
            Self::Limits => "Limits",
            Self::Updates => "Updates",
            Self::Shortcuts => "Keyboard Shortcuts",
            Self::About => "About",
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct Changed {
    pub shortcuts: bool,
    pub controls: bool,
}

pub struct Settings {
    open: bool,
    category: Category,
    shortcuts: ShortcutPage,
    controls: Vec<Control>,
    colors: ColorEditor,
    confirming_reset: bool,
}

impl Settings {
    pub fn new(controls: Vec<Control>) -> Self {
        Self {
            open: false,
            category: Category::General,
            shortcuts: ShortcutPage::default(),
            controls,
            colors: ColorEditor::default(),
            confirming_reset: false,
        }
    }

    pub fn controls(&self) -> &[Control] {
        &self.controls
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.confirming_reset = false;
        self.colors.close();
        self.shortcuts.cancel();
    }

    pub fn is_recording(&self) -> bool {
        self.open && self.shortcuts.is_recording()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn show(&mut self, ctx: &Context, area: Rect, bus: &mut ShortcutBus) -> Changed {
        let mut changed = Changed::default();
        if !self.open {
            return changed;
        }
        Area::new(Id::unique("settings_overlay"))
            .order(Order::Foreground)
            .fixed_pos(area.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(area.size(), Sense::click_and_drag());
                let nav = Rect::from_min_size(rect.min, vec2(NAV_WIDTH, rect.height()));
                let content = Rect::from_min_max(nav.right_top(), rect.right_bottom());
                ui.painter().rect_filled(rect, 0.0, theme::panel_fill());
                ui.painter().rect_filled(nav, 0.0, theme::viewer_fill());
                ui.painter().vline(
                    nav.right(),
                    nav.y_range(),
                    Stroke::new(1.0, theme::hover_fill()),
                );
                let tabs = nav.shrink(NAV_PADDING);
                self.show_navigation(&mut ui.new_child(UiBuilder::new().max_rect(tabs)));
                changed =
                    self.show_content(&mut ui.new_child(UiBuilder::new().max_rect(content)), bus);
                if close_button(ui, content) {
                    self.close();
                }
            });
        changed
    }

    fn show_navigation(&mut self, ui: &mut Ui) {
        ui.set_clip_rect(ui.max_rect());
        ui.scope(|ui| {
            theme::flat_item_style(ui.style_mut());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.spacing_mut().button_padding.x = NAV_TEXT_INSET;
            ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                ui.set_width(NAV_WIDTH - 2.0 * NAV_PADDING);
                ui.add_space(0.0);
                for category in Category::ALL {
                    let button = Button::new(category.label())
                        .selected(self.category == category)
                        .min_size(vec2(0.0, NAV_ITEM_HEIGHT));
                    if ui.add(button).clicked() {
                        self.category = category;
                        self.confirming_reset = false;
                        self.colors.close();
                        self.shortcuts.cancel();
                    }
                }
            });
        });
    }

    fn show_content(&mut self, ui: &mut Ui, bus: &mut ShortcutBus) -> Changed {
        let content = ui.max_rect();
        let padded = content.shrink(CONTENT_PADDING);
        let mut header = ui.new_child(UiBuilder::new().max_rect(padded));
        header.spacing_mut().item_spacing.y = ROW_SPACING;
        let title = RichText::new(self.category.label())
            .size(TITLE_SIZE)
            .color(theme::icon_selected());
        header.add(Label::new(title).selectable(false));
        let rule = header.min_rect().bottom() + ROW_SPACING;
        ui.painter().hline(
            padded.x_range(),
            rule,
            Stroke::new(1.0, theme::hover_fill()),
        );
        let body = Rect::from_min_max(
            pos2(padded.left(), rule + ROW_SPACING),
            pos2(padded.right(), content.bottom()),
        );
        let mut page = ui.new_child(UiBuilder::new().max_rect(body));
        page.spacing_mut().item_spacing.y = ROW_SPACING;
        match self.category {
            Category::Shortcuts => Changed {
                shortcuts: self.shortcuts.show(&mut page, bus),
                controls: false,
            },
            Category::About => {
                about(&mut page);
                Changed::default()
            }
            category => Changed {
                shortcuts: false,
                controls: self.controls_page(&mut page, category),
            },
        }
    }

    fn controls_page(&mut self, ui: &mut Ui, category: Category) -> bool {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                theme::field_style(ui.style_mut());
                if category == Category::Updates {
                    dim(ui, UPDATES_STUB);
                    ui.add_space(SECTION_GAP / 2.0);
                    return ui
                        .add_enabled_ui(false, |ui| {
                            settings_page::rows(ui, category, &mut self.controls, &mut self.colors)
                        })
                        .inner;
                }
                let mut changed =
                    settings_page::rows(ui, category, &mut self.controls, &mut self.colors);
                if category == Category::General {
                    changed |= self.reset_section(ui);
                }
                changed
            })
            .inner
    }

    fn reset_section(&mut self, ui: &mut Ui) -> bool {
        ui.add_space(SECTION_GAP);
        if !self.confirming_reset {
            if outlined_button(ui, "Reset settings to defaults").clicked() {
                self.confirming_reset = true;
            }
            return false;
        }
        dim(ui, RESET_WARNING);
        ui.add_space(SECTION_GAP / 2.0);
        ui.horizontal(|ui| {
            let mut reset = false;
            if outlined_button(ui, "Reset everything").clicked() {
                self.controls = settings_entries::defaults();
                self.colors.close();
                self.confirming_reset = false;
                reset = true;
            }
            if outlined_button(ui, "Cancel").clicked() {
                self.confirming_reset = false;
            }
            reset
        })
        .inner
    }
}

fn close_button(ui: &mut Ui, content: Rect) -> bool {
    let corner =
        content.right_top() + Vec2::new(-CLOSE_SIDE - CONTENT_PADDING / 2.0, CONTENT_PADDING / 2.0);
    let rect = Rect::from_min_size(corner, Vec2::splat(CLOSE_SIDE));
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
    icon_button(
        &mut child,
        Icon::Close,
        CLOSE_SIDE,
        CLOSE_REACH,
        ButtonState::Idle,
    )
    .on_hover_text("Close (Esc)")
    .clicked()
}

fn dim(ui: &mut Ui, text: impl Into<String>) {
    let text = RichText::new(text.into()).color(theme::text_dim());
    ui.add(Label::new(text).selectable(false));
}

fn bright(ui: &mut Ui, text: impl Into<String>) {
    let text = RichText::new(text.into()).color(theme::icon_idle());
    ui.add(Label::new(text).selectable(false));
}

fn about(ui: &mut Ui) {
    bright(
        ui,
        format!("{} {}", identity::DISPLAY_NAME, identity::VERSION),
    );
    dim(ui, "A fast reference image viewer.");
    dim(ui, "GPL 2.0 License");
}
