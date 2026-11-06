use crate::app::color_tool::{PickCursor, PickMode, ToolStyle};
use crate::app::settings_entries::{Control, ENTRIES, FILTERS, Setting, THEMES};
use crate::app::theme;
use crate::core::encode::{EncodeOptions, PngLevel, WebpMode};
use crate::core::limits::Limits;
use crate::core::resample::Filter;
use crate::core::rgba::Rgba;
use crate::core::update::Channel;

const BYTES_PER_MIB: u64 = 1024 * 1024;
const PNG_LEVELS: [PngLevel; 3] = [PngLevel::Fast, PngLevel::Default, PngLevel::Best];
const WEBP_MODES: [WebpMode; 2] = [WebpMode::Lossless, WebpMode::Lossy];
const RESIZE_UNITS: [ResizeUnit; 2] = [ResizeUnit::Pixels, ResizeUnit::Percent];
const LOSSY_POLICIES: [LossyPolicy; 3] = [
    LossyPolicy::ConvertToPng,
    LossyPolicy::KeepSourceFormat,
    LossyPolicy::WarnFirst,
];
const PICK_MODES: [PickMode; 2] = [PickMode::Sticky, PickMode::OneShot];
const PICK_CURSORS: [PickCursor; 2] = [PickCursor::Ring, PickCursor::Loupe];
const CHANNELS: [Channel; 2] = [Channel::Stable, Channel::Beta];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ResizeUnit {
    #[default]
    Pixels,
    Percent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LossyPolicy {
    #[default]
    ConvertToPng,
    KeepSourceFormat,
    WarnFirst,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Preferences {
    pub remember_recent: bool,
    pub recent_count: usize,
    pub theme: theme::Choice,
    pub refit_after_turn: bool,
    pub resize_filter: Filter,
    pub resize_unit: ResizeUnit,
    pub confirm_discard: bool,
    pub encode: EncodeOptions,
    pub lossy_sources: LossyPolicy,
    pub picker: ToolStyle,
    pub limits: Limits,
    pub check_updates: bool,
    pub update_channel: Channel,
}

impl Preferences {
    pub fn from_controls(controls: &[Control]) -> Self {
        let reader = Reader(controls);
        let mebibytes = |setting| u64::from(reader.number(setting)) * BYTES_PER_MIB;
        Self {
            remember_recent: reader.toggle(Setting::RememberRecent),
            recent_count: reader.number(Setting::RecentCount) as usize,
            theme: reader.pick(Setting::Theme, &THEMES),
            refit_after_turn: reader.toggle(Setting::RefitAfterTurn),
            resize_filter: reader.pick(Setting::ResizeFilter, &FILTERS),
            resize_unit: reader.pick(Setting::ResizeUnit, &RESIZE_UNITS),
            confirm_discard: reader.toggle(Setting::ConfirmDiscard),
            encode: EncodeOptions {
                lossy_quality: reader.number(Setting::LossyQuality).clamp(1, 100) as u8,
                webp: reader.pick(Setting::WebpMode, &WEBP_MODES),
                png: reader.pick(Setting::PngCompression, &PNG_LEVELS),
                flatten: reader.color(Setting::FlattenColor),
            },
            lossy_sources: reader.pick(Setting::LossySources, &LOSSY_POLICIES),
            picker: ToolStyle {
                mode: reader.pick(Setting::PickMode, &PICK_MODES),
                cursor: reader.pick(Setting::PickCursor, &PICK_CURSORS),
                hash_sign: !reader.toggle(Setting::DisableHashsign),
            },
            limits: Limits {
                encoded: mebibytes(Setting::MaxEncoded),
                decoded: mebibytes(Setting::MaxDecoded),
            }
            .enforced(reader.toggle(Setting::EnforceLimits)),
            check_updates: reader.toggle(Setting::CheckForUpdates),
            update_channel: reader.pick(Setting::UpdateChannel, &CHANNELS),
        }
    }
}

struct Reader<'a>(&'a [Control]);

impl Reader<'_> {
    fn find(&self, setting: Setting) -> Option<&Control> {
        ENTRIES
            .iter()
            .zip(self.0)
            .find(|(entry, _)| entry.setting == setting)
            .map(|(_, control)| control)
    }

    fn toggle(&self, setting: Setting) -> bool {
        matches!(self.find(setting), Some(Control::Toggle(true)))
    }

    fn number(&self, setting: Setting) -> u32 {
        match self.find(setting) {
            Some(Control::Number { value, .. }) => *value,
            _ => 0,
        }
    }

    fn color(&self, setting: Setting) -> Rgba {
        match self.find(setting) {
            Some(Control::Color(color)) => color.opaque(),
            _ => Rgba::OPAQUE_BLACK,
        }
    }

    fn pick<T: Copy>(&self, setting: Setting, all: &[T]) -> T {
        let index = match self.find(setting) {
            Some(Control::Choice { selected, .. }) => *selected,
            _ => 0,
        };
        all[index.min(all.len() - 1)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::settings_entries::defaults;

    #[test]
    fn default_controls_give_the_built_in_behaviour() {
        let preferences = Preferences::from_controls(&defaults());
        assert_eq!(preferences.encode, EncodeOptions::default());
        assert_eq!(preferences.limits, Limits::default());
        assert_eq!(preferences.resize_filter, Filter::default());
        assert_eq!(preferences.theme, theme::Choice::Slate);
        assert_eq!(
            preferences.picker,
            ToolStyle {
                hash_sign: true,
                ..ToolStyle::default()
            }
        );
        assert_eq!(preferences.lossy_sources, LossyPolicy::ConvertToPng);
        assert!(preferences.remember_recent);
        assert_eq!(preferences.recent_count, 10);
        assert!(!preferences.check_updates);
        assert_eq!(preferences.update_channel, Channel::Stable);
    }

    #[test]
    fn every_choice_has_a_typed_value_for_each_label() {
        let sizes = [
            (Setting::Theme, THEMES.len()),
            (Setting::ResizeFilter, FILTERS.len()),
            (Setting::ResizeUnit, RESIZE_UNITS.len()),
            (Setting::WebpMode, WEBP_MODES.len()),
            (Setting::PngCompression, PNG_LEVELS.len()),
            (Setting::LossySources, LOSSY_POLICIES.len()),
            (Setting::PickMode, PICK_MODES.len()),
            (Setting::PickCursor, PICK_CURSORS.len()),
            (Setting::UpdateChannel, CHANNELS.len()),
        ];
        for (setting, typed) in sizes {
            let entry = ENTRIES.iter().find(|entry| entry.setting == setting);
            let Some(Control::Choice { options, .. }) = entry.map(|entry| &entry.control) else {
                panic!("{setting:?} is not a choice");
            };
            assert_eq!(options.len(), typed, "{setting:?}");
        }
    }

    #[test]
    fn turning_enforcement_off_removes_the_limits() {
        let mut controls = defaults();
        for (entry, control) in ENTRIES.iter().zip(&mut controls) {
            if entry.setting == Setting::EnforceLimits {
                *control = Control::Toggle(false);
            }
        }
        assert_eq!(
            Preferences::from_controls(&controls).limits,
            Limits::UNLIMITED
        );
    }
}
