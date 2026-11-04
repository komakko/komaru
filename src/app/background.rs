use eframe::egui::{Color32, PopupCloseBehavior, RectAlign, Response};

use crate::app::color_picker::paint::color32;
use crate::app::color_picker::{self, ColorPicker, Parts};
use crate::app::popover;
use crate::app::theme;
use crate::core::rgba::Rgba;

const PARTS: Parts = Parts::ALL;
const FADE_IMAGE: &str = "Apply opacity to the image too";

#[derive(Default)]
pub struct ViewerBackground {
    color: Option<Rgba>,
    fade_image: bool,
    editor: Option<ColorPicker>,
}

impl ViewerBackground {
    pub fn fill(&self) -> Color32 {
        self.color.map_or_else(theme::viewer_fill, color32)
    }

    pub fn image_tint(&self) -> Color32 {
        match self.color {
            Some(color) if self.fade_image => Color32::from_white_alpha(color.a),
            _ => Color32::WHITE,
        }
    }

    pub fn reset(&mut self) {
        self.color = None;
        self.fade_image = false;
        if let Some(picker) = &mut self.editor {
            picker.reseed(themed());
        }
    }

    pub fn is_open(&self) -> bool {
        self.editor.is_some()
    }

    pub fn toggle(&mut self) {
        self.editor = if self.editor.is_some() {
            None
        } else {
            Some(ColorPicker::new(Some(self.color.unwrap_or_else(themed))))
        };
    }

    pub fn show_editor(&mut self, anchor: &Response, toggled: bool) {
        let Some(picker) = &mut self.editor else {
            return;
        };
        let fade_image = &mut self.fade_image;
        let shown = popover::at(
            anchor,
            RectAlign::RIGHT_START,
            PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                let picked = color_picker::show(ui, picker, PARTS);
                ui.checkbox(fade_image, FADE_IMAGE);
                picked
            },
        );
        let Some(shown) = shown else {
            return;
        };
        if shown.inner.is_some() {
            self.color = shown.inner;
        }
        if shown.response.should_close() && !toggled {
            self.editor = None;
        }
    }
}

fn themed() -> Rgba {
    let fill = theme::viewer_fill();
    Rgba::new(fill.r(), fill.g(), fill.b(), u8::MAX)
}
