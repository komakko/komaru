pub mod clipboard;
mod display_server;
mod file_uri;
mod graphics;
#[cfg(target_os = "macos")]
mod macos;
pub mod trace;
#[cfg(any(windows, test))]
mod win_formats;
mod window_icon;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_drop;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11_drop;

use eframe::wgpu::rwh::RawWindowHandle;

pub use display_server::event_loop_hook;
pub use file_uri::path_from_file_uri;
pub use graphics::wgpu_configuration;
pub use window_icon::window_icon;

use crate::core::ingest::Payload;
use crate::error::PlatformError;

pub enum DropEvent {
    Hover(bool),
    Receiving,
    Delivered(Result<Vec<Payload>, PlatformError>),
}

pub type DropSink = Box<dyn Fn(DropEvent) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopmostState {
    On,
    Off,
}

pub trait TopmostController {
    fn read(&self) -> Result<TopmostState, PlatformError>;
    fn write(&self, enabled: bool) -> Result<(), PlatformError>;
}

pub fn controller_for(
    window: RawWindowHandle,
) -> Result<Box<dyn TopmostController>, PlatformError> {
    match window {
        #[cfg(windows)]
        RawWindowHandle::Win32(handle) => Ok(Box::new(windows::WindowsTopmost::new(handle))),
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(handle) => Ok(Box::new(macos::MacosTopmost::new(handle))),
        #[cfg(all(unix, not(target_os = "macos")))]
        RawWindowHandle::Xlib(handle) => {
            let window_id = u32::try_from(handle.window)
                .map_err(|_| PlatformError::Unsupported("X11 window id out of range"))?;
            Ok(Box::new(x11::X11Topmost::connect(window_id)?))
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        RawWindowHandle::Xcb(handle) => {
            Ok(Box::new(x11::X11Topmost::connect(handle.window.get())?))
        }
        RawWindowHandle::Wayland(_) => Err(PlatformError::Unsupported(
            "Wayland offers no standard way to keep a window on top",
        )),
        _ => Err(PlatformError::Unsupported(
            "keeping a window on top is not supported on this window system",
        )),
    }
}

pub fn install_drop_target(window: RawWindowHandle, sink: DropSink) -> Result<(), PlatformError> {
    match window {
        #[cfg(windows)]
        RawWindowHandle::Win32(handle) => windows_drop::install(handle.hwnd.get(), sink),
        #[cfg(all(unix, not(target_os = "macos")))]
        RawWindowHandle::Xlib(handle) => {
            let window_id = u32::try_from(handle.window)
                .map_err(|_| PlatformError::Unsupported("X11 window id out of range"))?;
            x11_drop::install(window_id, sink)
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        RawWindowHandle::Xcb(handle) => x11_drop::install(handle.window.get(), sink),
        RawWindowHandle::Wayland(_) => Err(PlatformError::Unsupported(
            "dragging from browsers and other apps is not available on native Wayland yet",
        )),
        _ => Err(PlatformError::Unsupported(
            "dragging from browsers and other apps is not available on this window system yet",
        )),
    }
}
