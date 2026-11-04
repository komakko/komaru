use std::io;

use eframe::wgpu::rwh::Win32WindowHandle;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongW, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowPos, WS_EX_TOPMOST,
};

use crate::error::PlatformError;
use crate::platform::{TopmostController, TopmostState};

pub struct WindowsTopmost {
    window: HWND,
}

impl WindowsTopmost {
    pub fn new(handle: Win32WindowHandle) -> Self {
        Self {
            window: handle.hwnd.get() as HWND,
        }
    }
}

impl TopmostController for WindowsTopmost {
    fn read(&self) -> Result<TopmostState, PlatformError> {
        let style = unsafe { GetWindowLongW(self.window, GWL_EXSTYLE) } as u32;
        Ok(if style & WS_EX_TOPMOST != 0 {
            TopmostState::On
        } else {
            TopmostState::Off
        })
    }

    fn write(&self, enabled: bool) -> Result<(), PlatformError> {
        let insert_after = if enabled {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        let applied = unsafe {
            SetWindowPos(
                self.window,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
        if applied == 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }
}
