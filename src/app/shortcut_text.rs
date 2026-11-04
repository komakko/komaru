use eframe::egui::{Key, KeyboardShortcut, Modifiers};
use thiserror::Error;

const SEPARATOR: char = '+';
const COMMAND: &str = "Ctrl";
const ALT: &str = "Alt";
const SHIFT: &str = "Shift";

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{text:?} is not a shortcut")]
pub struct ShortcutTextError {
    text: String,
}

pub fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
    )
}

pub fn held_prefix(held: Modifiers) -> Option<String> {
    let names = modifier_names(held);
    (!names.is_empty()).then(|| format!("{}{SEPARATOR}...", names.join(&SEPARATOR.to_string())))
}

fn modifier_names(modifiers: Modifiers) -> Vec<&'static str> {
    [
        (modifiers.command, COMMAND),
        (modifiers.alt, ALT),
        (modifiers.shift, SHIFT),
    ]
    .into_iter()
    .filter_map(|(is_held, name)| is_held.then_some(name))
    .collect()
}

pub fn format(shortcut: &KeyboardShortcut) -> String {
    let mut parts = modifier_names(shortcut.modifiers);
    parts.push(shortcut.logical_key.name());
    parts.join(&SEPARATOR.to_string())
}

pub fn parse(text: &str) -> Result<KeyboardShortcut, ShortcutTextError> {
    let invalid = || ShortcutTextError {
        text: text.to_owned(),
    };
    let mut parts: Vec<&str> = text.split(SEPARATOR).collect();
    let key = parts
        .pop()
        .and_then(Key::from_name)
        .filter(|key| !is_modifier_key(*key))
        .ok_or_else(invalid)?;
    let mut modifiers = Modifiers::NONE;
    for name in parts {
        modifiers |= match name {
            COMMAND => Modifiers::COMMAND,
            ALT => Modifiers::ALT,
            SHIFT => Modifiers::SHIFT,
            _ => return Err(invalid()),
        };
    }
    Ok(KeyboardShortcut::new(modifiers, key))
}

pub fn from_press(key: Key, held: Modifiers) -> Option<KeyboardShortcut> {
    if is_modifier_key(key) || (held.ctrl && !held.command) {
        return None;
    }
    let mut modifiers = Modifiers {
        alt: held.alt,
        shift: held.shift,
        ..Modifiers::NONE
    };
    if held.command {
        modifiers |= Modifiers::COMMAND;
    }
    Some(KeyboardShortcut::new(modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_modifier_set() -> impl Iterator<Item = Modifiers> {
        (0..8).map(|bits| {
            let mut modifiers = Modifiers::NONE;
            if bits & 1 != 0 {
                modifiers |= Modifiers::COMMAND;
            }
            if bits & 2 != 0 {
                modifiers |= Modifiers::ALT;
            }
            if bits & 4 != 0 {
                modifiers |= Modifiers::SHIFT;
            }
            modifiers
        })
    }

    #[test]
    fn every_key_and_modifier_set_round_trips() {
        for &key in Key::ALL.iter().filter(|key| !is_modifier_key(**key)) {
            for modifiers in every_modifier_set() {
                let shortcut = KeyboardShortcut::new(modifiers, key);
                assert_eq!(parse(&format(&shortcut)), Ok(shortcut), "{shortcut:?}");
            }
        }
    }

    #[test]
    fn text_is_stable_and_portable() {
        let shortcut = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        assert_eq!(format(&shortcut), "Ctrl+Shift+Z");
        assert_eq!(parse("Ctrl+Shift+Z"), Ok(shortcut));
    }

    #[test]
    fn malformed_text_is_rejected() {
        for text in [
            "",
            "+",
            "Ctrl+",
            "Hyper+Z",
            "Ctrl+Nonsense",
            "Ctrl+Z+",
            "ControlLeft",
            "Ctrl+ShiftRight",
        ] {
            assert!(parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn a_press_keeps_only_representable_modifiers() {
        let held = Modifiers::COMMAND | Modifiers::SHIFT;
        assert_eq!(
            from_press(Key::A, held),
            Some(KeyboardShortcut::new(held, Key::A))
        );
        assert_eq!(from_press(Key::A, Modifiers::CTRL), None);
        assert_eq!(from_press(Key::ControlLeft, Modifiers::COMMAND), None);
    }

    #[test]
    fn held_modifiers_preview_as_a_prefix() {
        assert_eq!(held_prefix(Modifiers::NONE), None);
        assert_eq!(
            held_prefix(Modifiers::COMMAND | Modifiers::SHIFT),
            Some("Ctrl+Shift+...".to_owned())
        );
    }
}
