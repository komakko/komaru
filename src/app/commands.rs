use eframe::egui::{Key, KeyboardShortcut, Modifiers};

use crate::app::action::Action;

pub const MAX_SHORTCUTS: usize = 2;

pub struct Command {
    pub id: &'static str,
    pub action: Action,
    pub defaults: &'static [KeyboardShortcut],
}

const fn plain(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::NONE, key)
}

const fn command(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

const fn command_shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), key)
}

const fn entry(id: &'static str, action: Action, defaults: &'static [KeyboardShortcut]) -> Command {
    Command {
        id,
        action,
        defaults,
    }
}

pub const COMMANDS: &[Command] = &[
    entry("open", Action::Open, &[command(Key::O)]),
    entry("paste", Action::Paste, &[command(Key::V)]),
    entry("copy_image", Action::CopyImage, &[command(Key::C)]),
    entry("save_as", Action::SaveAs, &[command_shift(Key::S)]),
    entry("clear", Action::Clear, &[command(Key::W)]),
    entry("quit", Action::Quit, &[command(Key::Q)]),
    entry(
        "open_settings",
        Action::OpenSettings,
        &[command(Key::Comma)],
    ),
    entry("undo", Action::Undo, &[command(Key::Z)]),
    entry(
        "redo",
        Action::Redo,
        &[command_shift(Key::Z), command(Key::Y)],
    ),
    entry("fit_to_window", Action::FitToWindow, &[command(Key::Num0)]),
    entry("actual_size", Action::ActualSize, &[command(Key::Num1)]),
    entry(
        "toggle_pixelated",
        Action::TogglePixelated,
        &[command_shift(Key::P)],
    ),
    entry("toggle_pin", Action::TogglePin, &[command_shift(Key::T)]),
    entry(
        "rotate_left",
        Action::RotateLeft,
        &[command(Key::ArrowLeft)],
    ),
    entry(
        "rotate_right",
        Action::RotateRight,
        &[command(Key::ArrowRight)],
    ),
    entry(
        "flip_horizontal",
        Action::FlipHorizontal,
        &[command_shift(Key::H)],
    ),
    entry(
        "flip_vertical",
        Action::FlipVertical,
        &[command_shift(Key::V)],
    ),
    entry(
        "toggle_playback",
        Action::TogglePlayback,
        &[plain(Key::Space)],
    ),
    entry(
        "previous_frame",
        Action::PreviousFrame,
        &[plain(Key::Comma)],
    ),
    entry("next_frame", Action::NextFrame, &[plain(Key::Period)]),
    entry("open_resize", Action::OpenResize, &[plain(Key::R)]),
    entry("open_crop", Action::OpenCrop, &[plain(Key::C)]),
    entry(
        "open_color_picker",
        Action::OpenColorPicker,
        &[plain(Key::I)],
    ),
];

pub const MODAL_BINDINGS: &[(KeyboardShortcut, Action)] = &[
    (plain(Key::Escape), Action::DeclinePrompt),
    (plain(Key::Enter), Action::AcceptPrompt),
    (plain(Key::Escape), Action::CloseSettings),
    (plain(Key::Escape), Action::CancelResize),
    (plain(Key::Enter), Action::ApplyResize),
    (plain(Key::Escape), Action::CancelCrop),
    (plain(Key::Enter), Action::ApplyCrop),
    (plain(Key::Escape), Action::CloseColorPicker),
];

pub fn command_for(action: Action) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.action == action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_actions_are_unique() {
        for (index, command) in COMMANDS.iter().enumerate() {
            for other in &COMMANDS[index + 1..] {
                assert_ne!(command.id, other.id);
                assert_ne!(command.action, other.action);
            }
        }
    }

    #[test]
    fn defaults_fit_the_shortcut_limit() {
        for command in COMMANDS {
            assert!(command.defaults.len() <= MAX_SHORTCUTS, "{}", command.id);
        }
    }

    #[test]
    fn commands_never_use_a_modal_scope() {
        for command in COMMANDS {
            assert!(
                matches!(
                    command.action.scope(),
                    crate::app::action::Scope::Anywhere
                        | crate::app::action::Scope::Only(crate::app::action::Screen::Viewer)
                ),
                "{}",
                command.id
            );
        }
    }
}
