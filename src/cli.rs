use std::ffi::OsString;
use std::path::PathBuf;

use crate::platform::install;

pub enum Launch {
    Viewer(Vec<PathBuf>),
    Install,
    Uninstall,
}

pub fn parse(arguments: impl Iterator<Item = OsString>) -> Launch {
    let arguments: Vec<OsString> = arguments.collect();
    match arguments.first().and_then(|first| first.to_str()) {
        Some("--install") => Launch::Install,
        Some("--uninstall") => Launch::Uninstall,
        _ => Launch::Viewer(arguments.into_iter().map(PathBuf::from).collect()),
    }
}

pub fn install() -> i32 {
    match install::install() {
        Ok(binary) => {
            println!("komaru installed to {}", binary.display());
            0
        }
        Err(error) => failure(&error),
    }
}

pub fn uninstall() -> i32 {
    match install::uninstall() {
        Ok(()) => {
            println!("komaru removed; settings were kept");
            0
        }
        Err(error) => failure(&error),
    }
}

fn failure(error: &dyn std::error::Error) -> i32 {
    eprintln!("komaru: {error}");
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(arguments: &[&str]) -> Launch {
        parse(arguments.iter().map(OsString::from))
    }

    #[test]
    fn flags_select_maintenance_and_anything_else_opens_files() {
        assert!(matches!(launch(&["--install"]), Launch::Install));
        assert!(matches!(launch(&["--uninstall"]), Launch::Uninstall));
        assert!(matches!(launch(&[]), Launch::Viewer(files) if files.is_empty()));
        assert!(
            matches!(launch(&["a.png", "b.png"]), Launch::Viewer(files) if files == [PathBuf::from("a.png"), PathBuf::from("b.png")])
        );
    }
}
