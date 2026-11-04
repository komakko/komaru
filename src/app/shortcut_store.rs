use eframe::egui::KeyboardShortcut;
use thiserror::Error;
use toml::{Table, Value};

use crate::app::commands::{COMMANDS, Command, MAX_SHORTCUTS};
use crate::app::shortcut_text::{self, ShortcutTextError};
use crate::app::shortcuts::{BindError, ShortcutBus};

pub const SECTION: &str = "shortcuts";

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("default shortcuts clash: {0}")]
    Defaults(#[from] BindError),
    #[error("[shortcuts] has an unknown command {id:?}")]
    UnknownCommand { id: String },
    #[error("[shortcuts] {id} must be a list of shortcut texts")]
    NotAList { id: String },
    #[error("[shortcuts] {id} can have at most {MAX_SHORTCUTS} shortcuts")]
    TooMany { id: String },
    #[error("[shortcuts] {id}: {source}")]
    Text {
        id: String,
        #[source]
        source: ShortcutTextError,
    },
    #[error("[shortcuts] {id}: {text} is {source}")]
    Conflict {
        id: String,
        text: String,
        #[source]
        source: BindError,
    },
}

struct Override {
    command: &'static Command,
    shortcuts: Vec<(String, KeyboardShortcut)>,
}

pub fn build(section: Option<&Table>) -> (ShortcutBus, Vec<StoreError>) {
    let mut bus = match ShortcutBus::with_defaults() {
        Ok(bus) => bus,
        Err(error) => return (ShortcutBus::default(), vec![error.into()]),
    };
    let mut problems = Vec::new();
    let mut overrides = Vec::new();
    for (id, value) in section.into_iter().flatten() {
        match parse_entry(id, value) {
            Ok(parsed) => overrides.push(parsed),
            Err(error) => problems.push(error),
        }
    }
    for entry in &overrides {
        bus.unbind_all(entry.command.action);
    }
    for entry in overrides {
        for (text, shortcut) in entry.shortcuts {
            if let Err(source) = bus.bind(shortcut, entry.command.action) {
                problems.push(StoreError::Conflict {
                    id: entry.command.id.to_owned(),
                    text,
                    source,
                });
            }
        }
    }
    (bus, problems)
}

pub fn overrides(bus: &ShortcutBus) -> Table {
    COMMANDS
        .iter()
        .filter(|command| !bus.is_default(command))
        .map(|command| {
            let texts = bus
                .shortcuts_of(command.action)
                .map(|shortcut| Value::String(shortcut_text::format(&shortcut)))
                .collect();
            (command.id.to_owned(), Value::Array(texts))
        })
        .collect()
}

pub fn summary(problems: &[StoreError]) -> Option<String> {
    let (first, rest) = problems.split_first()?;
    Some(match rest.len() {
        0 => first.to_string(),
        more => format!("{first} (and {more} more)"),
    })
}

fn parse_entry(id: &str, value: &Value) -> Result<Override, StoreError> {
    let command = COMMANDS
        .iter()
        .find(|command| command.id == id)
        .ok_or_else(|| StoreError::UnknownCommand { id: id.to_owned() })?;
    let not_a_list = || StoreError::NotAList { id: id.to_owned() };
    let texts = value.as_array().ok_or_else(not_a_list)?;
    if texts.len() > MAX_SHORTCUTS {
        return Err(StoreError::TooMany { id: id.to_owned() });
    }
    let shortcuts = texts
        .iter()
        .map(|text| {
            let text = text.as_str().ok_or_else(not_a_list)?;
            let shortcut = shortcut_text::parse(text).map_err(|source| StoreError::Text {
                id: id.to_owned(),
                source,
            })?;
            Ok((text.to_owned(), shortcut))
        })
        .collect::<Result<_, StoreError>>()?;
    Ok(Override { command, shortcuts })
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};

    use super::*;
    use crate::app::action::Action;

    fn table(text: &str) -> Table {
        text.parse().expect("test table is valid TOML")
    }

    fn shortcuts_of(bus: &ShortcutBus, action: Action) -> Vec<KeyboardShortcut> {
        bus.shortcuts_of(action).collect()
    }

    #[test]
    fn defaults_store_nothing() {
        let bus = ShortcutBus::with_defaults().expect("defaults are valid");
        assert!(overrides(&bus).is_empty());
    }

    #[test]
    fn overrides_round_trip() {
        let mut bus = ShortcutBus::with_defaults().expect("defaults are valid");
        let moved = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, Key::K);
        bus.set(Action::Open, &[moved]).expect("free shortcut");
        bus.set(Action::Clear, &[]).expect("unbinding is fine");
        let stored = overrides(&bus);
        assert_eq!(stored.len(), 2);
        let (restored, problems) = build(Some(&stored));
        assert!(problems.is_empty());
        assert_eq!(shortcuts_of(&restored, Action::Open), [moved]);
        assert!(shortcuts_of(&restored, Action::Clear).is_empty());
    }

    #[test]
    fn two_commands_can_swap_shortcuts() {
        let stored = table("open = [\"Ctrl+W\"]\nclear = [\"Ctrl+O\"]");
        let (bus, problems) = build(Some(&stored));
        assert!(problems.is_empty());
        let open = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
        let close = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);
        assert_eq!(shortcuts_of(&bus, Action::Open), [close]);
        assert_eq!(shortcuts_of(&bus, Action::Clear), [open]);
    }

    #[test]
    fn bad_entries_are_reported_and_the_rest_applies() {
        let stored = table(
            "nonsense = [\"Ctrl+Q\"]\npaste = \"Ctrl+V\"\ncopy_image = [\"Hyper+C\"]\nopen = [\"Ctrl+K\"]\nundo = [\"Ctrl+K\"]",
        );
        let (bus, problems) = build(Some(&stored));
        assert_eq!(problems.len(), 4);
        let (_, crowded) = build(Some(&table("open = [\"Ctrl+A\", \"Ctrl+B\", \"Ctrl+D\"]")));
        assert!(matches!(crowded.as_slice(), [StoreError::TooMany { .. }]));
        assert_eq!(shortcuts_of(&bus, Action::Open).len(), 1);
        assert!(summary(&problems).is_some_and(|text| text.contains("more")));
    }
}
