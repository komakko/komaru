#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Open,
    OpenRecent(usize),
    ClearRecents,
    Paste,
    CopyImage,
    SaveAs,
    Clear,
    Quit,
    OpenSettings,
    CloseSettings,
    FitToWindow,
    ActualSize,
    TogglePixelated,
    TogglePin,
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
    OpenResize,
    ApplyResize,
    CancelResize,
    OpenCrop,
    ApplyCrop,
    CancelCrop,
    OpenColorPicker,
    CloseColorPicker,
    Undo,
    Redo,
    TogglePlayback,
    PreviousFrame,
    NextFrame,
    AcceptPrompt,
    DeclinePrompt,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Viewer,
    Prompt,
    Settings,
    Resize,
    Crop,
    ColorPicker,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Anywhere,
    Only(Screen),
}

#[derive(Clone, Copy)]
pub struct Availability {
    pub image_shown: bool,
    pub pin_usable: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_play: bool,
    pub can_step: bool,
}

impl Scope {
    pub fn contains(self, screen: Screen) -> bool {
        match self {
            Self::Anywhere => true,
            Self::Only(only) => only == screen,
        }
    }

    pub fn overlaps(self, other: Self) -> bool {
        match (self, other) {
            (Self::Only(first), Self::Only(second)) => first == second,
            _ => true,
        }
    }
}

impl Action {
    pub fn scope(self) -> Scope {
        match self {
            Self::Quit | Self::TogglePin => Scope::Anywhere,
            Self::Open
            | Self::OpenRecent(_)
            | Self::ClearRecents
            | Self::Paste
            | Self::CopyImage
            | Self::SaveAs
            | Self::Clear
            | Self::OpenSettings
            | Self::FitToWindow
            | Self::ActualSize
            | Self::TogglePixelated
            | Self::RotateLeft
            | Self::RotateRight
            | Self::FlipHorizontal
            | Self::FlipVertical
            | Self::OpenResize
            | Self::OpenCrop
            | Self::OpenColorPicker
            | Self::Undo
            | Self::Redo
            | Self::TogglePlayback
            | Self::PreviousFrame
            | Self::NextFrame => Scope::Only(Screen::Viewer),
            Self::CloseSettings => Scope::Only(Screen::Settings),
            Self::ApplyResize | Self::CancelResize => Scope::Only(Screen::Resize),
            Self::ApplyCrop | Self::CancelCrop => Scope::Only(Screen::Crop),
            Self::CloseColorPicker => Scope::Only(Screen::ColorPicker),
            Self::AcceptPrompt | Self::DeclinePrompt => Scope::Only(Screen::Prompt),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::OpenRecent(_) => "Open Recent",
            Self::ClearRecents => "Clear Recent List",
            Self::Paste => "Paste",
            Self::CopyImage => "Copy Image",
            Self::SaveAs => "Save As",
            Self::Clear => "Clear",
            Self::Quit => "Quit",
            Self::OpenSettings => "Settings",
            Self::CloseSettings => "Close Settings",
            Self::FitToWindow => "Fit to Window",
            Self::ActualSize => "Actual Size",
            Self::TogglePixelated => "Sharp Pixels",
            Self::TogglePin => "Always on Top",
            Self::RotateLeft => "Rotate Left",
            Self::RotateRight => "Rotate Right",
            Self::FlipHorizontal => "Flip Horizontal",
            Self::FlipVertical => "Flip Vertical",
            Self::OpenResize => "Resize",
            Self::ApplyResize => "Apply Resize",
            Self::CancelResize => "Cancel Resize",
            Self::OpenCrop => "Crop",
            Self::ApplyCrop => "Apply Crop",
            Self::CancelCrop => "Cancel Crop",
            Self::OpenColorPicker => "Color Picker",
            Self::CloseColorPicker => "Close Color Picker",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::TogglePlayback => "Play or Pause Animation",
            Self::PreviousFrame => "Previous Frame",
            Self::NextFrame => "Next Frame",
            Self::AcceptPrompt => "Accept Prompt",
            Self::DeclinePrompt => "Decline Prompt",
        }
    }

    pub fn is_enabled(self, availability: Availability) -> bool {
        match self {
            Self::CopyImage
            | Self::SaveAs
            | Self::Clear
            | Self::FitToWindow
            | Self::ActualSize
            | Self::RotateLeft
            | Self::RotateRight
            | Self::FlipHorizontal
            | Self::FlipVertical
            | Self::OpenResize
            | Self::OpenCrop
            | Self::OpenColorPicker => availability.image_shown,
            Self::TogglePin => availability.pin_usable,
            Self::Undo => availability.can_undo,
            Self::Redo => availability.can_redo,
            Self::TogglePlayback => availability.can_play,
            Self::PreviousFrame | Self::NextFrame => availability.can_step,
            Self::Open
            | Self::OpenRecent(_)
            | Self::ClearRecents
            | Self::Paste
            | Self::Quit
            | Self::OpenSettings
            | Self::CloseSettings
            | Self::ApplyResize
            | Self::CancelResize
            | Self::ApplyCrop
            | Self::CancelCrop
            | Self::CloseColorPicker
            | Self::AcceptPrompt
            | Self::DeclinePrompt
            | Self::TogglePixelated => true,
        }
    }
}
