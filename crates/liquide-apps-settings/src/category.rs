//! Setting categories and their metadata.

use std::fmt;

/// A category of related settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Display,
    Input,
    Audio,
    Network,
    Appearance,
    Privacy,
    Users,
    System,
}

impl Category {
    /// All available categories in display order.
    pub const ALL: &'static [Category] = &[
        Category::Display,
        Category::Input,
        Category::Audio,
        Category::Network,
        Category::Appearance,
        Category::Privacy,
        Category::Users,
        Category::System,
    ];

    /// Human-readable label for the category.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::Input => "Input",
            Self::Audio => "Audio",
            Self::Network => "Network",
            Self::Appearance => "Appearance",
            Self::Privacy => "Privacy",
            Self::Users => "Users & Accounts",
            Self::System => "System",
        }
    }

    /// Icon name for the category.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Input => "input-keyboard",
            Self::Audio => "audio-volume",
            Self::Network => "network-wired",
            Self::Appearance => "preferences-desktop-theme",
            Self::Privacy => "security-high",
            Self::Users => "system-users",
            Self::System => "preferences-system",
        }
    }

    /// Description of the category.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::Display => "Resolution, scaling, refresh rate, and multi-monitor layout",
            Self::Input => "Keyboard layout, mouse speed, touchpad, and accessibility",
            Self::Audio => "Output devices, volume, input devices, and sound effects",
            Self::Network => "Wi-Fi, Ethernet, VPN, proxy, and firewall settings",
            Self::Appearance => "Theme, wallpaper, fonts, icons, and cursor",
            Self::Privacy => "Location, camera, microphone, and screen sharing",
            Self::Users => "User accounts, login options, and admin access",
            Self::System => "Date & time, language, region, power, and updates",
        }
    }

    /// Parse a category from its string ID.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Category> {
        match id {
            "display" => Some(Self::Display),
            "input" => Some(Self::Input),
            "audio" => Some(Self::Audio),
            "network" => Some(Self::Network),
            "appearance" => Some(Self::Appearance),
            "privacy" => Some(Self::Privacy),
            "users" => Some(Self::Users),
            "system" => Some(Self::System),
            _ => None,
        }
    }

    /// String ID for the category.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Input => "input",
            Self::Audio => "audio",
            Self::Network => "network",
            Self::Appearance => "appearance",
            Self::Privacy => "privacy",
            Self::Users => "users",
            Self::System => "system",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Metadata about a category, including the number of entries it contains.
#[derive(Debug, Clone)]
pub struct CategoryInfo {
    pub category: Category,
    pub entry_count: usize,
    pub has_pending_changes: bool,
}

impl CategoryInfo {
    #[must_use]
    pub fn new(category: Category) -> Self {
        Self {
            category,
            entry_count: 0,
            has_pending_changes: false,
        }
    }
}
