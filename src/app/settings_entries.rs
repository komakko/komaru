use crate::app::settings::Category;
use crate::app::theme;
use crate::core::limits::{DEFAULT_DECODED_BYTES, DEFAULT_ENCODED_BYTES};
use crate::core::resample::Filter;
use crate::core::rgba::Rgba;

const BYTES_PER_MIB: u64 = 1024 * 1024;
const MAX_LIMIT_MIB: u32 = 65_536;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Control {
    Toggle(bool),
    Choice {
        options: &'static [&'static str],
        selected: usize,
    },
    Number {
        value: u32,
        min: u32,
        max: u32,
        suffix: &'static str,
    },
    Color(Rgba),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setting {
    Language,
    RememberRecent,
    RecentCount,
    Theme,
    RefitAfterTurn,
    ResizeFilter,
    ResizeUnit,
    ConfirmDiscard,
    FlattenColor,
    LossyQuality,
    WebpMode,
    PngCompression,
    LossySources,
    PickMode,
    PickCursor,
    DisableHashsign,
    EnforceLimits,
    MaxEncoded,
    MaxDecoded,
    CheckForUpdates,
    CheckFrequency,
    InstallAutomatically,
    UpdateChannel,
    DisableUpdates,
}

impl Setting {
    pub fn key(self) -> &'static str {
        match self {
            Self::Language => "language",
            Self::RememberRecent => "remember_recent",
            Self::RecentCount => "recent_count",
            Self::Theme => "theme",
            Self::RefitAfterTurn => "refit_after_turn",
            Self::ResizeFilter => "resize_filter",
            Self::ResizeUnit => "resize_unit",
            Self::ConfirmDiscard => "confirm_discard",
            Self::FlattenColor => "flatten_color",
            Self::LossyQuality => "lossy_quality",
            Self::WebpMode => "webp_mode",
            Self::PngCompression => "png_compression",
            Self::LossySources => "lossy_sources",
            Self::PickMode => "pick_mode",
            Self::PickCursor => "pick_cursor",
            Self::DisableHashsign => "disable_hashsign",
            Self::EnforceLimits => "enforce_limits",
            Self::MaxEncoded => "max_encoded_mib",
            Self::MaxDecoded => "max_decoded_mib",
            Self::CheckForUpdates => "check_for_updates",
            Self::CheckFrequency => "check_frequency",
            Self::InstallAutomatically => "install_automatically",
            Self::UpdateChannel => "update_channel",
            Self::DisableUpdates => "disable_updates",
        }
    }
}

pub struct Entry {
    pub setting: Setting,
    pub category: Category,
    pub label: &'static str,
    pub hint: &'static str,
    pub control: Control,
}

const fn choice(options: &'static [&'static str]) -> Control {
    Control::Choice {
        options,
        selected: 0,
    }
}

const fn number(value: u32, min: u32, max: u32, suffix: &'static str) -> Control {
    Control::Number {
        value,
        min,
        max,
        suffix,
    }
}

pub const THEMES: [theme::Choice; 4] = [
    theme::Choice::Slate,
    theme::Choice::Midnight,
    theme::Choice::Light,
    theme::Choice::FollowSystem,
];
pub const FILTERS: [Filter; 6] = [
    Filter::Lanczos3,
    Filter::CatmullRom,
    Filter::Mitchell,
    Filter::Hamming,
    Filter::Bilinear,
    Filter::Box,
];

pub fn defaults() -> Vec<Control> {
    ENTRIES.iter().map(|entry| entry.control.clone()).collect()
}

pub static ENTRIES: &[Entry] = &[
    Entry {
        setting: Setting::Language,
        category: Category::General,
        label: "Language",
        hint: "Language of menus, labels and messages",
        control: choice(&["English"]),
    },
    Entry {
        setting: Setting::RememberRecent,
        category: Category::General,
        label: "Remember recent files",
        hint: "Keep a list of recently opened files and links",
        control: Control::Toggle(true),
    },
    Entry {
        setting: Setting::RecentCount,
        category: Category::General,
        label: "Recent items to keep",
        hint: "How many entries the recent list holds",
        control: number(10, 1, 50, ""),
    },
    Entry {
        setting: Setting::Theme,
        category: Category::Appearance,
        label: "Theme",
        hint: "Colors of the window",
        control: choice(&["Slate", "Midnight", "Light", "Follow system"]),
    },
    Entry {
        setting: Setting::RefitAfterTurn,
        category: Category::Viewing,
        label: "Refit after rotate or flip",
        hint: "Reset zoom and pan to fit the window after turning the image",
        control: Control::Toggle(false),
    },
    Entry {
        setting: Setting::ResizeFilter,
        category: Category::Editing,
        label: "Resize filter",
        hint: "Resampling method used by Resize",
        control: choice(&[
            "Lanczos3",
            "Catmull-Rom",
            "Mitchell",
            "Hamming",
            "Bilinear",
            "Box",
        ]),
    },
    Entry {
        setting: Setting::ResizeUnit,
        category: Category::Editing,
        label: "Resize unit",
        hint: "Unit of the width and height fields in Resize",
        control: choice(&["Pixels", "Percent"]),
    },
    Entry {
        setting: Setting::ConfirmDiscard,
        category: Category::Editing,
        label: "Confirm before discarding edits",
        hint: "Ask before open, paste, clear or quit throws away edits",
        control: Control::Toggle(false),
    },
    Entry {
        setting: Setting::FlattenColor,
        category: Category::Saving,
        label: "Flatten color",
        hint: "Background for transparent pixels when saving JPEG or BMP",
        control: Control::Color(Rgba::new(255, 255, 255, 255)),
    },
    Entry {
        setting: Setting::LossyQuality,
        category: Category::Saving,
        label: "Lossy quality",
        hint: "Quality used when saving JPEG and lossy WebP, 1 to 100",
        control: number(90, 1, 100, ""),
    },
    Entry {
        setting: Setting::WebpMode,
        category: Category::Saving,
        label: "WebP mode",
        hint: "Lossless keeps every pixel, lossy is smaller",
        control: choice(&["Lossless", "Lossy"]),
    },
    Entry {
        setting: Setting::PngCompression,
        category: Category::Saving,
        label: "PNG compression",
        hint: "Deflate effort when saving PNG, higher is smaller but slower",
        control: Control::Choice {
            options: &["Fast", "Default", "Best"],
            selected: 1,
        },
    },
    Entry {
        setting: Setting::LossySources,
        category: Category::Saving,
        label: "Lossy sources",
        hint: "How Copy and Save As treat JPEG and WebP images, which grow a lot as PNG",
        control: choice(&[
            "Convert to PNG",
            "Keep source format",
            "Warn about size first",
        ]),
    },
    Entry {
        setting: Setting::PickMode,
        category: Category::ColorPicker,
        label: "Pick mode",
        hint: "Sticky keeps the tool armed after a click, one-shot disarms it",
        control: choice(&["Sticky", "One-shot"]),
    },
    Entry {
        setting: Setting::PickCursor,
        category: Category::ColorPicker,
        label: "Cursor",
        hint: "Cursor shown while picking",
        control: choice(&["Color ring", "Magnifier loupe"]),
    },
    Entry {
        setting: Setting::DisableHashsign,
        category: Category::ColorPicker,
        label: "Disable hashsign",
        hint: "Copy hex colors without the leading #",
        control: Control::Toggle(false),
    },
    Entry {
        setting: Setting::EnforceLimits,
        category: Category::Limits,
        label: "Enforce size limits",
        hint: "Refuse files and images that exceed the limits below",
        control: Control::Toggle(true),
    },
    Entry {
        setting: Setting::MaxEncoded,
        category: Category::Limits,
        label: "Largest encoded file",
        hint: "Bigger files are refused before decoding",
        control: number(
            (DEFAULT_ENCODED_BYTES / BYTES_PER_MIB) as u32,
            1,
            MAX_LIMIT_MIB,
            " MiB",
        ),
    },
    Entry {
        setting: Setting::MaxDecoded,
        category: Category::Limits,
        label: "Largest decoded image",
        hint: "Bigger images are refused from their header",
        control: number(
            (DEFAULT_DECODED_BYTES / BYTES_PER_MIB) as u32,
            1,
            MAX_LIMIT_MIB,
            " MiB",
        ),
    },
    Entry {
        setting: Setting::CheckForUpdates,
        category: Category::Updates,
        label: "Check for updates",
        hint: "Look for a newer version in the background",
        control: Control::Toggle(true),
    },
    Entry {
        setting: Setting::CheckFrequency,
        category: Category::Updates,
        label: "Check frequency",
        hint: "How often the background check runs",
        control: choice(&["On startup", "Daily", "Weekly"]),
    },
    Entry {
        setting: Setting::InstallAutomatically,
        category: Category::Updates,
        label: "Install updates automatically",
        hint: "Download and install new versions without asking",
        control: Control::Toggle(false),
    },
    Entry {
        setting: Setting::UpdateChannel,
        category: Category::Updates,
        label: "Update channel",
        hint: "Stable releases only, or early builds too",
        control: choice(&["Stable", "Beta"]),
    },
    Entry {
        setting: Setting::DisableUpdates,
        category: Category::Updates,
        label: "Disable all updates",
        hint: "Never check, download or install anything",
        control: Control::Toggle(false),
    },
];
