use eframe::egui::{Context, Event, Key, KeyboardShortcut, Modifiers};
use thiserror::Error;

use crate::app::action::{Action, Availability, Screen};
use crate::app::commands::{COMMANDS, Command, MAX_SHORTCUTS, MODAL_BINDINGS, command_for};

pub type Binding = (KeyboardShortcut, Action);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BindError {
    #[error("already bound to {}", .existing.label())]
    Conflict { existing: Action },
    #[error("a command can have at most {MAX_SHORTCUTS} shortcuts")]
    TooMany,
}

#[derive(Default)]
pub struct ShortcutBus {
    bindings: Vec<Binding>,
}

impl ShortcutBus {
    pub fn new(bindings: &[Binding]) -> Result<Self, BindError> {
        let mut bus = Self::default();
        for &(shortcut, action) in bindings {
            bus.bind(shortcut, action)?;
        }
        Ok(bus)
    }

    pub fn with_defaults() -> Result<Self, BindError> {
        let mut bus = Self::new(MODAL_BINDINGS)?;
        for command in COMMANDS {
            for &shortcut in command.defaults {
                bus.bind(shortcut, command.action)?;
            }
        }
        Ok(bus)
    }

    pub fn take(
        &self,
        ctx: &Context,
        screen: Screen,
        availability: Availability,
    ) -> Option<Action> {
        if self.bindings.is_empty() {
            return None;
        }
        let typing = ctx.text_edit_focused();
        ctx.input_mut(|input| self.take_from(&mut input.events, screen, availability, typing))
    }

    pub fn bind(&mut self, shortcut: KeyboardShortcut, action: Action) -> Result<(), BindError> {
        let clash = self
            .bindings
            .iter()
            .find(|(bound, other)| *bound == shortcut && other.scope().overlaps(action.scope()));
        if let Some(&(_, existing)) = clash {
            return Err(BindError::Conflict { existing });
        }
        self.bindings.push((shortcut, action));
        Ok(())
    }

    pub fn unbind_all(&mut self, action: Action) {
        self.bindings.retain(|&(_, bound)| bound != action);
    }

    pub fn set(&mut self, action: Action, shortcuts: &[KeyboardShortcut]) -> Result<(), BindError> {
        if shortcuts.len() > MAX_SHORTCUTS {
            return Err(BindError::TooMany);
        }
        let backup = self.bindings.clone();
        self.unbind_all(action);
        let result = shortcuts
            .iter()
            .try_for_each(|&shortcut| self.bind(shortcut, action));
        if result.is_err() {
            self.bindings = backup;
        }
        result
    }

    pub fn reset(&mut self, action: Action) -> Result<(), BindError> {
        let defaults = command_for(action).map_or(&[][..], |command| command.defaults);
        self.set(action, defaults)
    }

    pub fn shortcuts_of(&self, action: Action) -> impl Iterator<Item = KeyboardShortcut> + '_ {
        self.bindings
            .iter()
            .filter(move |&&(_, bound)| bound == action)
            .map(|&(shortcut, _)| shortcut)
    }

    pub fn is_default(&self, command: &Command) -> bool {
        let current: Vec<_> = self.shortcuts_of(command.action).collect();
        current.len() == command.defaults.len()
            && command
                .defaults
                .iter()
                .all(|default| current.contains(default))
    }

    pub fn hint(&self, ctx: &Context, action: Action) -> Option<String> {
        self.shortcuts_of(action)
            .next()
            .map(|shortcut| ctx.format_shortcut(&shortcut))
    }

    pub fn hinted(&self, ctx: &Context, text: &str, action: Action) -> String {
        match self.hint(ctx, action) {
            Some(hint) => format!("{text} ({hint})"),
            None => text.to_owned(),
        }
    }

    fn take_from(
        &self,
        events: &mut Vec<Event>,
        screen: Screen,
        availability: Availability,
        typing: bool,
    ) -> Option<Action> {
        let mut index = 0;
        while index < events.len() {
            if let Event::Key {
                key,
                modifiers,
                pressed: true,
                repeat,
                ..
            } = events[index]
                && (!typing || ends_text_entry(key))
                && let Some(action) = self.resolve(key, modifiers, screen)
            {
                events.remove(index);
                if !repeat && action.is_enabled(availability) {
                    return Some(action);
                }
            } else {
                index += 1;
            }
        }
        None
    }

    fn resolve(&self, key: Key, held: Modifiers, screen: Screen) -> Option<Action> {
        self.bindings
            .iter()
            .filter(|(shortcut, action)| {
                shortcut.logical_key == key
                    && held.matches_logically(shortcut.modifiers)
                    && action.scope().contains(screen)
            })
            .max_by_key(|(shortcut, _)| specificity(shortcut.modifiers))
            .map(|&(_, action)| action)
    }
}

fn ends_text_entry(key: Key) -> bool {
    matches!(key, Key::Enter | Key::Escape)
}

fn specificity(modifiers: Modifiers) -> usize {
    [
        modifiers.alt,
        modifiers.ctrl,
        modifiers.shift,
        modifiers.mac_cmd,
        modifiers.command,
    ]
    .into_iter()
    .filter(|&held| held)
    .count()
}

#[cfg(test)]
mod tests {
    use eframe::egui::RawInput;

    use super::*;

    const ENABLED: Availability = Availability {
        image_shown: true,
        pin_usable: true,
        can_undo: true,
        can_redo: true,
        can_play: true,
        can_step: true,
    };
    const EMPTY_VIEWER: Availability = Availability {
        image_shown: false,
        pin_usable: true,
        can_undo: false,
        can_redo: false,
        can_play: false,
        can_step: false,
    };

    fn shortcut(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
        KeyboardShortcut::new(modifiers, key)
    }

    fn key_event(key: Key, modifiers: Modifiers, pressed: bool, repeat: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat,
            modifiers,
        }
    }

    fn press(key: Key, modifiers: Modifiers) -> Event {
        key_event(key, modifiers, true, false)
    }

    fn bus(bindings: &[Binding]) -> ShortcutBus {
        ShortcutBus::new(bindings).expect("test bindings do not conflict")
    }

    #[test]
    fn bound_press_fires_and_is_consumed() {
        let bus = bus(&[(shortcut(Modifiers::COMMAND, Key::O), Action::Open)]);
        let mut events = vec![press(Key::O, Modifiers::COMMAND)];
        let fired = bus.take_from(&mut events, Screen::Viewer, ENABLED, false);
        assert_eq!(fired, Some(Action::Open));
        assert!(events.is_empty());
    }

    #[test]
    fn unbound_press_is_left_for_other_widgets() {
        let bus = bus(&[(shortcut(Modifiers::COMMAND, Key::O), Action::Open)]);
        let mut events = vec![press(Key::P, Modifiers::COMMAND)];
        assert_eq!(
            bus.take_from(&mut events, Screen::Viewer, ENABLED, false),
            None
        );
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn release_does_not_fire() {
        let bus = bus(&[(shortcut(Modifiers::NONE, Key::F), Action::FitToWindow)]);
        let mut events = vec![key_event(Key::F, Modifiers::NONE, false, false)];
        assert_eq!(
            bus.take_from(&mut events, Screen::Viewer, ENABLED, false),
            None
        );
    }

    #[test]
    fn most_specific_binding_wins() {
        let plain = shortcut(Modifiers::COMMAND, Key::O);
        let with_shift = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::O);
        let bus = bus(&[(plain, Action::Open), (with_shift, Action::Clear)]);
        let held = Modifiers {
            shift: true,
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            bus.resolve(Key::O, held, Screen::Viewer),
            Some(Action::Clear)
        );
        assert_eq!(
            bus.resolve(Key::O, Modifiers::COMMAND, Screen::Viewer),
            Some(Action::Open)
        );
    }

    #[test]
    fn command_press_does_not_fire_a_plain_binding() {
        let bus = bus(&[(shortcut(Modifiers::NONE, Key::F), Action::FitToWindow)]);
        assert_eq!(
            bus.resolve(Key::F, Modifiers::COMMAND, Screen::Viewer),
            None
        );
    }

    #[test]
    fn scope_limits_where_a_binding_fires() {
        let bus = bus(&[(
            shortcut(Modifiers::NONE, Key::Escape),
            Action::CloseSettings,
        )]);
        let mut events = vec![press(Key::Escape, Modifiers::NONE)];
        assert_eq!(
            bus.take_from(&mut events, Screen::Viewer, ENABLED, false),
            None
        );
        assert_eq!(events.len(), 1);
        assert_eq!(
            bus.take_from(&mut events, Screen::Settings, ENABLED, false),
            Some(Action::CloseSettings)
        );
    }

    #[test]
    fn disabled_action_is_swallowed_and_later_presses_still_fire() {
        let bus = bus(&[
            (shortcut(Modifiers::NONE, Key::Delete), Action::Clear),
            (shortcut(Modifiers::COMMAND, Key::O), Action::Open),
        ]);
        let mut events = vec![
            press(Key::Delete, Modifiers::NONE),
            press(Key::O, Modifiers::COMMAND),
        ];
        let fired = bus.take_from(&mut events, Screen::Viewer, EMPTY_VIEWER, false);
        assert_eq!(fired, Some(Action::Open));
        assert!(events.is_empty());
    }

    #[test]
    fn repeat_is_swallowed_without_firing() {
        let bus = bus(&[(shortcut(Modifiers::NONE, Key::P), Action::TogglePin)]);
        let mut events = vec![key_event(Key::P, Modifiers::NONE, true, true)];
        assert_eq!(
            bus.take_from(&mut events, Screen::Viewer, ENABLED, false),
            None
        );
        assert!(events.is_empty());
    }

    #[test]
    fn several_presses_in_one_frame_come_out_in_order() {
        let bus = bus(&[
            (shortcut(Modifiers::NONE, Key::P), Action::TogglePin),
            (shortcut(Modifiers::NONE, Key::S), Action::TogglePixelated),
        ]);
        let mut events = vec![
            press(Key::P, Modifiers::NONE),
            press(Key::S, Modifiers::NONE),
        ];
        let first = bus.take_from(&mut events, Screen::Viewer, ENABLED, false);
        let second = bus.take_from(&mut events, Screen::Viewer, ENABLED, false);
        let third = bus.take_from(&mut events, Screen::Viewer, ENABLED, false);
        assert_eq!(first, Some(Action::TogglePin));
        assert_eq!(second, Some(Action::TogglePixelated));
        assert_eq!(third, None);
    }

    #[test]
    fn one_action_may_have_several_shortcuts() {
        let bus = bus(&[
            (shortcut(Modifiers::NONE, Key::F), Action::FitToWindow),
            (shortcut(Modifiers::COMMAND, Key::Num0), Action::FitToWindow),
        ]);
        assert_eq!(
            bus.resolve(Key::Num0, Modifiers::COMMAND, Screen::Viewer),
            Some(Action::FitToWindow)
        );
        assert_eq!(
            bus.resolve(Key::F, Modifiers::NONE, Screen::Viewer),
            Some(Action::FitToWindow)
        );
    }

    #[test]
    fn same_shortcut_conflicts_when_scopes_overlap() {
        let escape = shortcut(Modifiers::NONE, Key::Escape);
        let result = ShortcutBus::new(&[(escape, Action::Quit), (escape, Action::Clear)]);
        assert_eq!(
            result.err(),
            Some(BindError::Conflict {
                existing: Action::Quit
            })
        );
    }

    #[test]
    fn same_shortcut_is_allowed_on_different_screens() {
        let escape = shortcut(Modifiers::NONE, Key::Escape);
        let bus = bus(&[(escape, Action::Clear), (escape, Action::CloseSettings)]);
        assert_eq!(
            bus.resolve(Key::Escape, Modifiers::NONE, Screen::Viewer),
            Some(Action::Clear)
        );
        assert_eq!(
            bus.resolve(Key::Escape, Modifiers::NONE, Screen::Settings),
            Some(Action::CloseSettings)
        );
    }

    #[test]
    fn typing_keeps_every_key_but_enter_and_escape_from_the_bus() {
        let bus = bus(&[
            (shortcut(Modifiers::NONE, Key::F), Action::FitToWindow),
            (shortcut(Modifiers::NONE, Key::Enter), Action::ApplyResize),
            (shortcut(Modifiers::NONE, Key::Escape), Action::CancelResize),
        ]);
        let mut events = vec![press(Key::F, Modifiers::NONE)];
        assert_eq!(
            bus.take_from(&mut events, Screen::Resize, ENABLED, true),
            None
        );
        assert_eq!(events.len(), 1);
        events.push(press(Key::Enter, Modifiers::NONE));
        assert_eq!(
            bus.take_from(&mut events, Screen::Resize, ENABLED, true),
            Some(Action::ApplyResize)
        );
        events.push(press(Key::Escape, Modifiers::NONE));
        assert_eq!(
            bus.take_from(&mut events, Screen::Resize, ENABLED, true),
            Some(Action::CancelResize)
        );
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn resize_keys_fire_only_on_the_resize_screen() {
        let bus = bus(MODAL_BINDINGS);
        assert_eq!(
            bus.resolve(Key::Enter, Modifiers::NONE, Screen::Resize),
            Some(Action::ApplyResize)
        );
        assert_eq!(
            bus.resolve(Key::Escape, Modifiers::NONE, Screen::Resize),
            Some(Action::CancelResize)
        );
        assert_eq!(
            bus.resolve(Key::Enter, Modifiers::NONE, Screen::Viewer),
            None
        );
        assert_eq!(
            bus.resolve(Key::Escape, Modifiers::NONE, Screen::Settings),
            Some(Action::CloseSettings)
        );
    }

    #[test]
    fn crop_keys_fire_only_on_the_crop_screen() {
        let bus = bus(MODAL_BINDINGS);
        assert_eq!(
            bus.resolve(Key::Enter, Modifiers::NONE, Screen::Crop),
            Some(Action::ApplyCrop)
        );
        assert_eq!(
            bus.resolve(Key::Escape, Modifiers::NONE, Screen::Crop),
            Some(Action::CancelCrop)
        );
        assert_eq!(
            bus.resolve(Key::Enter, Modifiers::NONE, Screen::Viewer),
            None
        );
    }

    #[test]
    fn default_bindings_are_valid() {
        assert!(ShortcutBus::with_defaults().is_ok());
    }

    #[test]
    fn every_command_starts_at_its_defaults() {
        let bus = ShortcutBus::with_defaults().expect("defaults are valid");
        assert!(COMMANDS.iter().all(|command| bus.is_default(command)));
    }

    #[test]
    fn set_replaces_an_actions_shortcuts() {
        let mut bus = ShortcutBus::with_defaults().expect("defaults are valid");
        let moved = shortcut(Modifiers::COMMAND, Key::K);
        assert_eq!(bus.set(Action::Open, &[moved]), Ok(()));
        assert_eq!(bus.shortcuts_of(Action::Open).collect::<Vec<_>>(), [moved]);
        assert_eq!(bus.reset(Action::Open), Ok(()));
        assert!(command_for(Action::Open).is_some_and(|command| bus.is_default(command)));
    }

    #[test]
    fn a_conflicting_set_changes_nothing() {
        let mut bus = ShortcutBus::with_defaults().expect("defaults are valid");
        let taken = shortcut(Modifiers::COMMAND, Key::W);
        let before: Vec<_> = bus.shortcuts_of(Action::Open).collect();
        assert_eq!(
            bus.set(Action::Open, &[taken]),
            Err(BindError::Conflict {
                existing: Action::Clear
            })
        );
        assert_eq!(bus.shortcuts_of(Action::Open).collect::<Vec<_>>(), before);
    }

    #[test]
    fn reset_conflicting_with_a_user_binding_changes_nothing() {
        let mut bus = ShortcutBus::with_defaults().expect("defaults are valid");
        let open_default = shortcut(Modifiers::COMMAND, Key::O);
        assert_eq!(bus.set(Action::Open, &[]), Ok(()));
        assert_eq!(bus.set(Action::OpenCrop, &[open_default]), Ok(()));
        assert_eq!(
            bus.reset(Action::Open),
            Err(BindError::Conflict {
                existing: Action::OpenCrop
            })
        );
        assert_eq!(bus.shortcuts_of(Action::Open).count(), 0);
    }

    fn frame(ctx: &Context, bus: &ShortcutBus, events: Vec<Event>) -> Vec<Option<Action>> {
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut fired = Vec::new();
        let mut output = ctx.run_ui(input, |_| {
            while let Some(action) = bus.take(ctx, Screen::Viewer, ENABLED) {
                fired.push(Some(action));
            }
        });
        output.textures_delta.clear();
        fired
    }

    #[test]
    fn live_context_delivers_a_press_once_and_ignores_held_repeats() {
        let ctx = Context::default();
        let bus = bus(&[(shortcut(Modifiers::COMMAND, Key::O), Action::Open)]);
        let press_open = || press(Key::O, Modifiers::COMMAND);
        assert_eq!(frame(&ctx, &bus, vec![press_open()]), [Some(Action::Open)]);
        assert_eq!(frame(&ctx, &bus, vec![press_open()]), []);
        let release = key_event(Key::O, Modifiers::COMMAND, false, false);
        assert_eq!(frame(&ctx, &bus, vec![release]), []);
        assert_eq!(frame(&ctx, &bus, vec![press_open()]), [Some(Action::Open)]);
    }
}
