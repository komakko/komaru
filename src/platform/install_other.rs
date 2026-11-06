use std::path::PathBuf;

use crate::error::InstallError;

const UNSUPPORTED: &str = "use the installer from the releases page on this system";

pub fn install() -> Result<PathBuf, InstallError> {
    Err(InstallError::Unsupported(UNSUPPORTED))
}

pub fn uninstall() -> Result<(), InstallError> {
    Err(InstallError::Unsupported(UNSUPPORTED))
}

#[cfg(windows)]
pub fn is_installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|executable| {
            executable
                .parent()
                .map(|directory| directory.join("unins000.exe"))
        })
        .is_some_and(|uninstaller| uninstaller.is_file())
}

#[cfg(not(windows))]
pub fn is_installed() -> bool {
    false
}
