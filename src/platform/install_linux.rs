use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use directories::BaseDirs;

use crate::error::InstallError;
use crate::identity;

const DESKTOP_TEMPLATE: &str = include_str!("../../packaging/linux/komaru.desktop");
const ICON: &[u8] = include_bytes!("../../assets/icon/komaru.png");
const RECORD_NAME: &str = "installed-files";

struct Layout {
    binary: PathBuf,
    desktop: PathBuf,
    icon: PathBuf,
    state: PathBuf,
}

impl Layout {
    fn locate() -> Result<Self, InstallError> {
        let directories = BaseDirs::new().ok_or(InstallError::NoDirectory)?;
        let binaries = directories
            .executable_dir()
            .ok_or(InstallError::NoDirectory)?;
        let data = directories.data_dir();
        Ok(Self {
            binary: binaries.join(identity::NAME),
            desktop: data
                .join("applications")
                .join(format!("{}.desktop", identity::NAME)),
            icon: data
                .join("icons/hicolor/256x256/apps")
                .join(format!("{}.png", identity::NAME)),
            state: data.join(identity::NAME),
        })
    }

    fn record(&self) -> PathBuf {
        self.state.join(RECORD_NAME)
    }
}

fn io_error(path: &Path, source: std::io::Error) -> InstallError {
    InstallError::Io {
        path: path.to_owned(),
        source,
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).map_err(|source| io_error(directory, source))?;
    }
    fs::write(path, bytes).map_err(|source| io_error(path, source))
}

fn desktop_entry(binary: &Path) -> String {
    DESKTOP_TEMPLATE.replace(
        &format!("Exec={} ", identity::NAME),
        &format!("Exec={} ", binary.display()),
    )
}

fn refresh_caches(layout: &Layout) {
    let quiet = |program: &str, argument: &Path, extra: &[&str]| {
        let _ = Command::new(program)
            .args(extra)
            .arg(argument)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    if let Some(directory) = layout.desktop.parent() {
        quiet("update-desktop-database", directory, &[]);
    }
    if let Some(theme) = layout.icon.ancestors().nth(3) {
        quiet("gtk-update-icon-cache", theme, &["-q", "-t"]);
    }
}

pub fn install() -> Result<PathBuf, InstallError> {
    let layout = Layout::locate()?;
    let source = std::env::current_exe().map_err(|source| io_error(Path::new("."), source))?;
    if source != layout.binary {
        let bytes = fs::read(&source).map_err(|error| io_error(&source, error))?;
        write_file(&layout.binary.with_extension("new"), &bytes)?;
        let incoming = layout.binary.with_extension("new");
        set_executable(&incoming)?;
        fs::rename(&incoming, &layout.binary).map_err(|error| io_error(&layout.binary, error))?;
    }
    write_file(&layout.desktop, desktop_entry(&layout.binary).as_bytes())?;
    write_file(&layout.icon, ICON)?;
    let record = [
        &layout.binary,
        &layout.desktop,
        &layout.icon,
        &layout.record(),
    ]
    .map(|path| path.display().to_string())
    .join("\n");
    write_file(&layout.record(), format!("{record}\n").as_bytes())?;
    refresh_caches(&layout);
    Ok(layout.binary)
}

pub fn uninstall() -> Result<(), InstallError> {
    let layout = Layout::locate()?;
    let record = fs::read_to_string(layout.record()).map_err(|error| match error.kind() {
        ErrorKind::NotFound => InstallError::NotInstalled,
        _ => io_error(&layout.record(), error),
    })?;
    for path in record.lines().map(Path::new) {
        match fs::remove_file(path) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                return Err(io_error(path, error));
            }
            _ => {}
        }
    }
    let _ = fs::remove_dir(&layout.state);
    refresh_caches(&layout);
    Ok(())
}

pub fn is_installed() -> bool {
    let Ok(layout) = Layout::locate() else {
        return false;
    };
    std::env::current_exe().is_ok_and(|executable| executable == layout.binary)
        && layout.record().is_file()
}

fn set_executable(path: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|error| io_error(path, error))
}
