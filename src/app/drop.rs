use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{Context, Id, LayerId, Order, Rect, StrokeKind};
use eframe::wgpu::rwh::HasWindowHandle;

use crate::app::theme;
use crate::core::ingest::Payload;
use crate::core::loader::LoadRequester;
use crate::error::PlatformError;
use crate::platform::{self, DropEvent};

const HOVER_CORNER_RADIUS: f32 = 4.0;
const HOVER_BORDER_INSET: f32 = 2.0;

pub fn dropped_payload(ctx: &Context) -> Option<Payload> {
    ctx.input(|input| {
        let paths: Vec<_> = input
            .raw
            .dropped_files
            .iter()
            .map(|file| file.path().to_owned())
            .collect();
        (!paths.is_empty()).then_some(Payload::Files(paths))
    })
}

#[derive(Default)]
pub struct NativeDrop {
    attached: bool,
    hover: Arc<AtomicBool>,
}

impl NativeDrop {
    pub fn attach_once(
        &mut self,
        frame: &eframe::Frame,
        ctx: &Context,
        requester: LoadRequester,
    ) -> Result<(), PlatformError> {
        if self.attached {
            return Ok(());
        }
        let Ok(handle) = frame.window_handle() else {
            return Ok(());
        };
        self.attached = true;
        let hover = Arc::clone(&self.hover);
        let ctx = ctx.clone();
        platform::install_drop_target(
            handle.as_raw(),
            Box::new(move |event| {
                match event {
                    DropEvent::Hover(active) => hover.store(active, Ordering::Release),
                    DropEvent::Receiving => {
                        hover.store(false, Ordering::Release);
                        requester.receiving_drop();
                    }
                    DropEvent::Delivered(Ok(payloads)) => requester.ingest(payloads),
                    DropEvent::Delivered(Err(error)) => requester.fail(error.into()),
                }
                ctx.request_repaint();
            }),
        )
    }

    pub fn is_hovering(&self, ctx: &Context) -> bool {
        self.hover.load(Ordering::Acquire) || ctx.input(|input| !input.raw.hovered_files.is_empty())
    }
}

pub fn paint_hover(ctx: &Context, area: Rect) {
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("drop_hover")));
    painter.rect(
        area.shrink(HOVER_BORDER_INSET),
        HOVER_CORNER_RADIUS,
        theme::drop_hover_fill(),
        (2.0, theme::drop_hover_stroke()),
        StrokeKind::Inside,
    );
}
