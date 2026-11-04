use eframe::egui::containers::menu::MenuBar;
use eframe::egui::{
    Align, Color32, CornerRadius, Frame, Label, Layout, Margin, Mesh, Panel, Pos2, Rect, Response,
    RichText, Sense, Shape, Stroke, StrokeKind, Style, Ui, UiBuilder, Vec2, ViewportCommand, vec2,
};

use crate::app::action::{Action, Availability};
use crate::app::icons::{self, Icon};
use crate::app::menu_bar::{self, MenuView};
use crate::app::pin::PinView;
use crate::app::shortcuts::ShortcutBus;
use crate::core::recents::Recents;

const MARGIN_X: i8 = 6;
const MARGIN_Y: i8 = 4;
const BUTTON_SIZE: f32 = 22.0;
const BUTTON_RADIUS: u8 = 3;
const WINDOW_GLYPH_REACH: f32 = 4.5;
const MENU_BUTTON_PADDING: [f32; 2] = [8.0, 2.0];
const TITLE_GAP: f32 = 8.0;
const WINDOW_CONTROL_COUNT: f32 = 4.0;

const BAR_TOP: Color32 = Color32::from_rgb(52, 68, 98);
const BAR_BOTTOM: Color32 = Color32::from_rgb(38, 50, 74);
const BAR_BORDER: Color32 = Color32::from_rgb(16, 22, 34);
const BUTTON_TOP: Color32 = Color32::from_rgb(92, 110, 144);
const BUTTON_BOTTOM: Color32 = Color32::from_rgb(62, 78, 108);
const BUTTON_HOVER_TOP: Color32 = Color32::from_rgb(122, 142, 178);
const BUTTON_BORDER: Color32 = Color32::from_rgb(18, 26, 40);
const ICON_COLOR: Color32 = Color32::from_rgb(228, 234, 244);
const PIN_ACTIVE_COLOR: Color32 = Color32::from_rgb(240, 96, 96);

#[derive(Clone, Copy)]
pub struct ImageTitle<'a> {
    pub name: &'a str,
    pub edited: bool,
}

pub fn show(
    ui: &mut Ui,
    pin: &PinView,
    shown: Option<ImageTitle>,
    availability: Availability,
    pixelated: bool,
    shortcuts: &ShortcutBus,
    recents: &Recents,
) -> Option<Action> {
    let mut action = None;
    let frame = Frame::NONE
        .fill(BAR_BOTTOM)
        .inner_margin(Margin::symmetric(MARGIN_X, MARGIN_Y));
    Panel::top("toolbar").frame(frame).show(ui, |ui| {
        let bar_rect = ui
            .max_rect()
            .expand2(Vec2::new(f32::from(MARGIN_X), f32::from(MARGIN_Y)));
        paint_bar_background(ui, bar_rect);

        let drag_response = ui.interact(
            bar_rect,
            ui.scope_id().with("drag"),
            Sense::click_and_drag(),
        );
        handle_drag(ui, &drag_response);

        let row = Rect::from_min_size(ui.max_rect().min, vec2(ui.max_rect().width(), BUTTON_SIZE));
        let controls_width = window_controls_width(ui);
        let controls_row = Rect::from_min_max(
            Pos2::new((row.right() - controls_width).max(row.left()), row.top()),
            row.right_bottom(),
        );
        let menu_row = Rect::from_min_max(
            row.left_top(),
            Pos2::new(controls_row.left() - TITLE_GAP, row.bottom()),
        );
        let controls = UiBuilder::new()
            .max_rect(controls_row)
            .layout(Layout::right_to_left(Align::Center));
        action = ui
            .scope_builder(controls, |ui| window_controls(ui, pin, shortcuts))
            .inner;
        let menus = UiBuilder::new()
            .max_rect(menu_row)
            .layout(Layout::left_to_right(Align::Center));
        let view = MenuView {
            availability,
            pixelated,
            pin,
            shortcuts,
            recents,
        };
        let menu_action = ui
            .scope_builder(menus, |ui| menus_and_title(ui, &view, shown))
            .inner;
        action = action.or(menu_action);
    });
    action
}

fn menus_and_title(ui: &mut Ui, view: &MenuView, shown: Option<ImageTitle>) -> Option<Action> {
    ui.spacing_mut().interact_size.y = BUTTON_SIZE;
    MenuBar::new()
        .style(bar_style)
        .config(menu_bar::dropdown_config())
        .ui(ui, |ui| {
            let action = menu_bar::show(ui, view);
            ui.add_space(TITLE_GAP);
            title(ui, shown);
            action
        })
        .inner
}

fn bar_style(style: &mut Style) {
    style.spacing.button_padding = vec2(MENU_BUTTON_PADDING[0], MENU_BUTTON_PADDING[1]);
    style.spacing.item_spacing.x = 2.0;
    let widgets = &mut style.visuals.widgets;
    let states = [
        (&mut widgets.inactive, Color32::TRANSPARENT),
        (&mut widgets.hovered, BUTTON_BOTTOM),
        (&mut widgets.active, BUTTON_TOP),
        (&mut widgets.open, BUTTON_TOP),
    ];
    for (widget, fill) in states {
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0, ICON_COLOR);
        widget.corner_radius = CornerRadius::same(BUTTON_RADIUS);
        widget.expansion = 0.0;
    }
}

fn title(ui: &mut Ui, shown: Option<ImageTitle>) {
    let Some(title) = shown else {
        return;
    };
    let name = if title.edited {
        format!("{} *", title.name)
    } else {
        title.name.to_owned()
    };
    let text = RichText::new(name).color(ICON_COLOR);
    ui.add(Label::new(text).truncate().selectable(false));
}

fn paint_bar_background(ui: &Ui, rect: Rect) {
    let painter = ui.painter();
    painter.add(gradient_shape(rect, BAR_TOP, BAR_BOTTOM));
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, BAR_BORDER),
    );
}

fn gradient_shape(rect: Rect, top: Color32, bottom: Color32) -> Shape {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    Shape::mesh(mesh)
}

fn handle_drag(ui: &Ui, drag_response: &Response) {
    if drag_response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if drag_response.double_clicked() {
        toggle_maximized(ui);
    }
}

fn window_controls_width(ui: &Ui) -> f32 {
    WINDOW_CONTROL_COUNT * BUTTON_SIZE + (WINDOW_CONTROL_COUNT - 1.0) * ui.spacing().item_spacing.x
}

fn window_controls(ui: &mut Ui, pin: &PinView, shortcuts: &ShortcutBus) -> Option<Action> {
    if window_button(ui, Icon::Close)
        .on_hover_text("Close")
        .clicked()
    {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }
    let (maximize_icon, maximize_tooltip) = if is_maximized(ui) {
        (Icon::Restore, "Restore")
    } else {
        (Icon::Maximize, "Maximize")
    };
    if window_button(ui, maximize_icon)
        .on_hover_text(maximize_tooltip)
        .clicked()
    {
        toggle_maximized(ui);
    }
    if window_button(ui, Icon::Minimize)
        .on_hover_text("Minimize")
        .clicked()
    {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
    pin_button(ui, pin, shortcuts)
        .clicked()
        .then_some(Action::TogglePin)
}

fn pin_button(ui: &mut Ui, pin: &PinView, shortcuts: &ShortcutBus) -> Response {
    let pinned = matches!(pin, PinView::On);
    let available = !matches!(pin, PinView::Unavailable(_));
    let response = glyph_button(ui, Icon::Pin { pinned }, available);
    let tooltip = match pin {
        PinView::On => shortcuts.hinted(ui.ctx(), "Unpin", Action::TogglePin),
        PinView::Off => shortcuts.hinted(ui.ctx(), "Keep on top", Action::TogglePin),
        PinView::Unavailable(reason) => reason.clone(),
    };
    response.on_hover_text(tooltip)
}

fn window_button(ui: &mut Ui, icon: Icon) -> Response {
    glyph_button(ui, icon, true)
}

fn glyph_button(ui: &mut Ui, icon: Icon, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(BUTTON_SIZE), sense);
    let painter = ui.painter();
    let (top, bottom) = button_colors(&response, enabled);
    let radius = CornerRadius::same(BUTTON_RADIUS);
    painter.rect_filled(rect, radius, bottom);
    painter.rect_filled(
        Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y)),
        CornerRadius {
            nw: BUTTON_RADIUS,
            ne: BUTTON_RADIUS,
            sw: 0,
            se: 0,
        },
        top,
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, BUTTON_BORDER),
        StrokeKind::Inside,
    );
    icons::paint(
        painter,
        rect.center(),
        WINDOW_GLYPH_REACH,
        icon,
        icon_color(icon, enabled),
    );
    response
}

fn button_colors(response: &Response, enabled: bool) -> (Color32, Color32) {
    if !enabled {
        return (
            BUTTON_TOP.gamma_multiply(0.5),
            BUTTON_BOTTOM.gamma_multiply(0.5),
        );
    }
    if response.is_pointer_button_down_on() {
        return (BUTTON_BOTTOM, BUTTON_BOTTOM.gamma_multiply(0.85));
    }
    if response.hovered() {
        return (BUTTON_HOVER_TOP, BUTTON_TOP);
    }
    (BUTTON_TOP, BUTTON_BOTTOM)
}

fn icon_color(icon: Icon, enabled: bool) -> Color32 {
    let base = match icon {
        Icon::Pin { pinned: true } => PIN_ACTIVE_COLOR,
        _ => ICON_COLOR,
    };
    if enabled {
        base
    } else {
        base.gamma_multiply(0.4)
    }
}

fn is_maximized(ui: &Ui) -> bool {
    ui.input(|input| input.viewport().maximized.unwrap_or(false))
}

fn toggle_maximized(ui: &Ui) {
    ui.ctx()
        .send_viewport_cmd(ViewportCommand::Maximized(!is_maximized(ui)));
}
