#[cfg(all(unix, not(target_os = "macos")))]
#[path = "install_linux.rs"]
mod system;

#[cfg(not(all(unix, not(target_os = "macos"))))]
#[path = "install_other.rs"]
mod system;

pub use system::{install, is_installed, uninstall};
