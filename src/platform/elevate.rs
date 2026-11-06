#[cfg(windows)]
#[path = "elevate_windows.rs"]
mod system;

#[cfg(not(windows))]
#[path = "elevate_other.rs"]
mod system;

pub use system::replace_executable;
