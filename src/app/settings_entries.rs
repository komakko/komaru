use crate::app::settings::Category;
use crate::app::theme;
use crate::core::limits::{DEFAULT_DECODED_BYTES, DEFAULT_ENCODED_BYTES};
use crate::core::resample::Filter;
use crate::core::rgba::Rgba;

const BYTES_PER_MIB: u64 = 1024 * 1024;
const MAX_LIMIT_MIB: u32 = 65_536;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Variant {
    pub token: &'static str,
    pub label: &'static str,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Control {
    Toggle(bool),
    Choice {
        options: &'static [Variant],
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
    Theme,
    RememberRecent,
    RecentCount,
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
    UpdateChannel,
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
            Self::ResizeUnit => "resize_percent",
            Self::ConfirmDiscard => "confirm_discard",
            Self::FlattenColor => "flatten_color",
            Self::LossyQuality => "lossy_quality",
            Self::WebpMode => "webp_lossy",
            Self::PngCompression => "png_compression",
            Self::LossySources => "lossy_sources",
            Self::PickMode => "pick_one_shot",
            Self::PickCursor => "pick_loupe",
            Self::DisableHashsign => "disable_hashsign",
            Self::EnforceLimits => "enforce_limits",
            Self::MaxEncoded => "max_encoded_mib",
            Self::MaxDecoded => "max_decoded_mib",
            Self::CheckForUpdates => "check_for_updates",
            Self::UpdateChannel => "update_beta",
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

const fn variant(token: &'static str, label: &'static str) -> Variant {
    Variant { token, label }
}

const fn choice(options: &'static [Variant]) -> Control {
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
        control: choice(&[variant("en", "English")]),
    },
    Entry {
        setting: Setting::Theme,
        category: Category::General,
        label: "Theme",
        hint: "Colors of the window",
        control: choice(&[
            variant("slate", "Slate"),
            variant("midnight", "Midnight"),
            variant("light", "Light"),
            variant("system", "Follow system"),
        ]),
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
        setting: Setting::RefitAfterTurn,
        category: Category::Editing,
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
            variant("lanczos3", "Lanczos3"),
            variant("catmull_rom", "Catmull-Rom"),
            variant("mitchell", "Mitchell"),
            variant("hamming", "Hamming"),
            variant("bilinear", "Bilinear"),
            variant("box", "Box"),
        ]),
    },
    Entry {
        setting: Setting::ResizeUnit,
        category: Category::Editing,
        label: "Resize unit",
        hint: "Unit of the width and height fields in Resize",
        control: choice(&[variant("pixels", "Pixels"), variant("percent", "Percent")]),
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
        control: choice(&[variant("lossless", "Lossless"), variant("lossy", "Lossy")]),
    },
    Entry {
        setting: Setting::PngCompression,
        category: Category::Saving,
        label: "PNG compression",
        hint: "Deflate effort when saving PNG, higher is smaller but slower",
        control: Control::Choice {
            options: &[
                variant("fast", "Fast"),
                variant("default", "Default"),
                variant("best", "Best"),
            ],
            selected: 1,
        },
    },
    Entry {
        setting: Setting::LossySources,
        category: Category::Saving,
        label: "Lossy sources",
        hint: "How Copy and Save As treat JPEG and WebP images, which grow a lot as PNG",
        control: choice(&[
            variant("png", "Convert to PNG"),
            variant("source", "Keep source format"),
            variant("warn", "Warn about size first"),
        ]),
    },
    Entry {
        setting: Setting::PickMode,
        category: Category::ColorPicker,
        label: "Pick mode",
        hint: "Sticky keeps the tool armed after a click, one-shot disarms it",
        control: choice(&[variant("sticky", "Sticky"), variant("one_shot", "One-shot")]),
    },
    Entry {
        setting: Setting::PickCursor,
        category: Category::ColorPicker,
        label: "Cursor",
        hint: "Cursor shown while picking",
        control: choice(&[
            variant("ring", "Color ring"),
            variant("loupe", "Magnifier loupe"),
        ]),
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
        hint: "Look for a newer version at startup; off means no network use",
        control: Control::Toggle(false),
    },
    Entry {
        setting: Setting::UpdateChannel,
        category: Category::Updates,
        label: "Update channel",
        hint: "Stable releases only, or early builds too",
        control: choice(&[variant("stable", "Stable"), variant("beta", "Beta")]),
    },
];
