use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use toml::Table;

use crate::error::ConfigError;
use crate::identity;

const FILE_NAME: &str = "settings.toml";
const PARTIAL_EXTENSION: &str = "toml.part";
const REJECTED_EXTENSION: &str = "toml.invalid";

pub struct Config {
    path: Option<PathBuf>,
    table: Table,
    unreadable: bool,
}

impl Config {
    pub fn load() -> (Self, Option<ConfigError>) {
        let path = ProjectDirs::from("", "", identity::NAME)
            .map(|directories| directories.config_dir().join(FILE_NAME));
        match path.as_deref().map(read) {
            Some(Err(error)) => (Self::new(path, Table::new(), true), Some(error)),
            Some(Ok(table)) => (Self::new(path, table, false), None),
            None => (
                Self::new(None, Table::new(), false),
                Some(ConfigError::NoDirectory),
            ),
        }
    }

    pub fn section(&self, name: &str) -> Option<&Table> {
        self.table.get(name).and_then(toml::Value::as_table)
    }

    pub fn replace_section(&mut self, name: &str, section: Table) {
        if section.is_empty() {
            self.table.remove(name);
        } else {
            self.table.insert(name.to_owned(), section.into());
        }
    }

    pub fn save(&mut self) -> Result<(), ConfigError> {
        let path = self.path.as_deref().ok_or(ConfigError::NoDirectory)?;
        let text = toml::to_string(&self.table)?;
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory).map_err(|source| io_error(directory, source))?;
        }
        if self.unreadable {
            set_aside(path)?;
            self.unreadable = false;
        }
        let partial = path.with_extension(PARTIAL_EXTENSION);
        fs::write(&partial, text).map_err(|source| io_error(&partial, source))?;
        fs::rename(&partial, path).map_err(|source| io_error(path, source))
    }

    fn new(path: Option<PathBuf>, table: Table, unreadable: bool) -> Self {
        Self {
            path,
            table,
            unreadable,
        }
    }
}

fn read(path: &Path) -> Result<Table, ConfigError> {
    match fs::read_to_string(path) {
        Ok(text) => text.parse().map_err(|source| ConfigError::Parse {
            path: path.to_owned(),
            source,
        }),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(Table::new()),
        Err(source) => Err(io_error(path, source)),
    }
}

fn set_aside(path: &Path) -> Result<(), ConfigError> {
    match fs::rename(path, path.with_extension(REJECTED_EXTENSION)) {
        Err(source) if source.kind() != ErrorKind::NotFound => Err(io_error(path, source)),
        _ => Ok(()),
    }
}

fn io_error(path: &Path, source: std::io::Error) -> ConfigError {
    ConfigError::Io {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_in(directory: &Path) -> Config {
        Config::new(Some(directory.join(FILE_NAME)), Table::new(), false)
    }

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("komaru-config-{name}"));
        fs::remove_dir_all(&directory).ok();
        directory
    }

    fn section_with(key: &str, value: &str) -> Table {
        let mut section = Table::new();
        section.insert(key.to_owned(), value.into());
        section
    }

    #[test]
    fn saved_sections_read_back() {
        let directory = scratch("roundtrip");
        let mut config = config_in(&directory);
        config.replace_section("shortcuts", section_with("open", "Ctrl+O"));
        config.save().expect("save succeeds");
        let table = read(&directory.join(FILE_NAME)).expect("file reads back");
        assert_eq!(table["shortcuts"]["open"].as_str(), Some("Ctrl+O"));
        assert!(!directory.join("settings.toml.part").exists());
    }

    #[test]
    fn unrelated_sections_survive_a_save() {
        let directory = scratch("preserve");
        let mut config = config_in(&directory);
        config.replace_section("limits", section_with("mode", "strict"));
        config.replace_section("shortcuts", section_with("open", "Ctrl+O"));
        config.replace_section("shortcuts", Table::new());
        config.save().expect("save succeeds");
        let table = read(&directory.join(FILE_NAME)).expect("file reads back");
        assert!(table.contains_key("limits"));
        assert!(!table.contains_key("shortcuts"));
    }

    #[test]
    fn missing_file_is_an_empty_table() {
        let directory = scratch("missing");
        let table = read(&directory.join(FILE_NAME)).expect("missing is fine");
        assert!(table.is_empty());
    }

    #[test]
    fn invalid_file_is_set_aside_on_the_next_save() {
        let directory = scratch("invalid");
        fs::create_dir_all(&directory).expect("directory");
        let path = directory.join(FILE_NAME);
        fs::write(&path, "shortcuts = [").expect("write invalid file");
        assert!(matches!(read(&path), Err(ConfigError::Parse { .. })));
        let mut config = Config::new(Some(path.clone()), Table::new(), true);
        config.replace_section("shortcuts", section_with("open", "Ctrl+O"));
        config.save().expect("save succeeds");
        assert_eq!(
            fs::read_to_string(directory.join("settings.toml.invalid")).expect("kept"),
            "shortcuts = ["
        );
        assert!(read(&path).is_ok());
    }

    #[test]
    fn saving_without_a_directory_fails() {
        let mut config = Config::new(None, Table::new(), false);
        assert!(matches!(config.save(), Err(ConfigError::NoDirectory)));
    }
}
