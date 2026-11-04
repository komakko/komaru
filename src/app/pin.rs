use std::time::{Duration, Instant};

use eframe::egui::Context;
use eframe::wgpu::rwh::HasWindowHandle;

use crate::platform::{self, TopmostController, TopmostState};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const SETTLE_DELAY: Duration = Duration::from_millis(120);

pub enum PinView {
    On,
    Off,
    Unavailable(String),
}

impl PinView {
    pub fn is_usable(&self) -> bool {
        !matches!(self, Self::Unavailable(_))
    }
}

enum Backend {
    Detached,
    Attached(Box<dyn TopmostController>),
    Unsupported,
}

pub struct PinControl {
    backend: Backend,
    view: PinView,
    next_poll: Instant,
}

impl PinControl {
    pub fn detached() -> Self {
        Self {
            backend: Backend::Detached,
            view: PinView::Off,
            next_poll: Instant::now(),
        }
    }

    pub fn view(&self) -> &PinView {
        &self.view
    }

    pub fn attach_once(&mut self, frame: &eframe::Frame) {
        if !matches!(self.backend, Backend::Detached) {
            return;
        }
        let Ok(handle) = frame.window_handle() else {
            return;
        };
        self.backend = match platform::controller_for(handle.as_raw()) {
            Ok(controller) => Backend::Attached(controller),
            Err(error) => {
                self.view = PinView::Unavailable(error.to_string());
                Backend::Unsupported
            }
        };
    }

    pub fn poll(&mut self, ctx: &Context) {
        let Backend::Attached(controller) = &self.backend else {
            return;
        };
        let now = Instant::now();
        if now >= self.next_poll {
            self.view = match controller.read() {
                Ok(TopmostState::On) => PinView::On,
                Ok(TopmostState::Off) => PinView::Off,
                Err(error) => PinView::Unavailable(error.to_string()),
            };
            self.next_poll = now + POLL_INTERVAL;
        }
        ctx.request_repaint_after(self.next_poll.saturating_duration_since(now));
    }

    pub fn toggle(&mut self) -> Option<String> {
        let Backend::Attached(controller) = &self.backend else {
            return None;
        };
        let enable = !matches!(self.view, PinView::On);
        let failure = controller.write(enable).err().map(|error| {
            let message = format!("always-on-top request failed: {error}");
            self.view = PinView::Unavailable(error.to_string());
            message
        });
        self.next_poll = Instant::now() + SETTLE_DELAY;
        failure
    }
}
