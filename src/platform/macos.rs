use std::ffi::c_void;
use std::ptr::NonNull;

use eframe::wgpu::rwh::AppKitWindowHandle;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSFloatingWindowLevel, NSNormalWindowLevel, NSView, NSWindow};

use crate::error::PlatformError;
use crate::platform::{TopmostController, TopmostState};

pub struct MacosTopmost {
    view: NonNull<c_void>,
}

impl MacosTopmost {
    pub fn new(handle: AppKitWindowHandle) -> Self {
        Self {
            view: handle.ns_view,
        }
    }

    fn window(&self) -> Result<Retained<NSWindow>, PlatformError> {
        MainThreadMarker::new().ok_or(PlatformError::WindowUnavailable(
            "window level needs the main thread",
        ))?;
        let view = unsafe { self.view.cast::<NSView>().as_ref() };
        view.window().ok_or(PlatformError::WindowUnavailable(
            "view is not attached to a window",
        ))
    }
}

impl TopmostController for MacosTopmost {
    fn read(&self) -> Result<TopmostState, PlatformError> {
        let above = self.window()?.level() >= NSFloatingWindowLevel;
        Ok(if above {
            TopmostState::On
        } else {
            TopmostState::Off
        })
    }

    fn write(&self, enabled: bool) -> Result<(), PlatformError> {
        let level = if enabled {
            NSFloatingWindowLevel
        } else {
            NSNormalWindowLevel
        };
        self.window()?.setLevel(level);
        Ok(())
    }
}
