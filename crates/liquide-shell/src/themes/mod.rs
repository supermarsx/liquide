//! Theme presets for the LiquiDE desktop environment.

pub mod liquid_glass;
pub mod midday;
pub mod night;
pub mod sunset;

/// Available theme preset IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemePreset {
    /// Standard Liquid Glass dark theme.
    LiquidGlass,
    /// OLED-optimized dark theme with true black backgrounds (default).
    Night,
    /// Warm dark theme with amber/orange tones.
    Sunset,
    /// Tarnished white light theme.
    Midday,
}

impl ThemePreset {
    /// Get the CSS string for this theme preset.
    pub fn css(&self) -> &'static str {
        match self {
            ThemePreset::LiquidGlass => liquid_glass::CSS,
            ThemePreset::Night => night::CSS,
            ThemePreset::Sunset => sunset::CSS,
            ThemePreset::Midday => midday::CSS,
        }
    }

    /// Parse a theme ID string to a preset.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "liquid-glass" | "standard" => Some(ThemePreset::LiquidGlass),
            "night" | "default" => Some(ThemePreset::Night),
            "sunset" => Some(ThemePreset::Sunset),
            "midday" => Some(ThemePreset::Midday),
            _ => None,
        }
    }

    /// The theme preset ID string.
    pub fn id(&self) -> &'static str {
        match self {
            ThemePreset::LiquidGlass => "liquid-glass",
            ThemePreset::Night => "night",
            ThemePreset::Sunset => "sunset",
            ThemePreset::Midday => "midday",
        }
    }
}
