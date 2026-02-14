//! Theme system — dark/light mode, color tokens, design tokens.

use crate::color::UiColor;
use serde::{Deserialize, Serialize};

/// Theme mode — controls the overall light/dark appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeMode {
    /// Dark mode (default Liquid Glass).
    Dark,
    /// Light mode (Midday).
    Light,
    /// Follow system preference.
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

/// Color tokens for a UI theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeColors {
    // Surface colors
    pub background: UiColor,
    pub surface: UiColor,
    pub surface_hover: UiColor,
    pub surface_active: UiColor,

    // Text colors
    pub text_primary: UiColor,
    pub text_secondary: UiColor,
    pub text_disabled: UiColor,
    pub text_on_accent: UiColor,

    // Brand / accent
    pub accent: UiColor,
    pub accent_hover: UiColor,
    pub accent_active: UiColor,

    // Semantic
    pub success: UiColor,
    pub warning: UiColor,
    pub error: UiColor,
    pub info: UiColor,

    // Borders
    pub border: UiColor,
    pub border_strong: UiColor,
    pub border_subtle: UiColor,

    // Glass
    pub glass_tint: UiColor,

    // Scrollbar
    pub scrollbar_thumb: UiColor,
    pub scrollbar_track: UiColor,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::liquid_glass_dark()
    }
}

impl ThemeColors {
    /// Liquid Glass Standard dark palette (spec-design.md §2.1).
    pub fn liquid_glass_dark() -> Self {
        Self {
            background: UiColor::new(28, 28, 46, 255),
            surface: UiColor::new(255, 255, 255, 20),
            surface_hover: UiColor::new(255, 255, 255, 31),
            surface_active: UiColor::new(255, 255, 255, 41),
            text_primary: UiColor::new(255, 255, 255, 255),
            text_secondary: UiColor::new(255, 255, 255, 179),
            text_disabled: UiColor::new(255, 255, 255, 77),
            text_on_accent: UiColor::white(),
            accent: UiColor::new(0, 122, 255, 255),
            accent_hover: UiColor::new(30, 142, 255, 255),
            accent_active: UiColor::new(0, 102, 230, 255),
            success: UiColor::new(48, 209, 88, 255),
            warning: UiColor::new(255, 214, 10, 255),
            error: UiColor::new(255, 69, 58, 255),
            info: UiColor::new(100, 210, 255, 255),
            border: UiColor::new(255, 255, 255, 31),
            border_strong: UiColor::new(255, 255, 255, 51),
            border_subtle: UiColor::new(255, 255, 255, 15),
            glass_tint: UiColor::new(30, 30, 50, 179),
            scrollbar_thumb: UiColor::new(255, 255, 255, 51),
            scrollbar_track: UiColor::new(255, 255, 255, 10),
        }
    }

    /// Night OLED dark palette (spec-theme-night.md).
    pub fn night() -> Self {
        Self {
            background: UiColor::new(0, 0, 0, 255),
            surface: UiColor::new(255, 255, 255, 15),
            surface_hover: UiColor::new(255, 255, 255, 26),
            surface_active: UiColor::new(255, 255, 255, 36),
            text_primary: UiColor::new(255, 255, 255, 255),
            text_secondary: UiColor::new(255, 255, 255, 204),
            text_disabled: UiColor::new(255, 255, 255, 77),
            text_on_accent: UiColor::white(),
            accent: UiColor::new(10, 132, 255, 255),
            accent_hover: UiColor::new(40, 152, 255, 255),
            accent_active: UiColor::new(0, 112, 235, 255),
            success: UiColor::new(48, 209, 88, 255),
            warning: UiColor::new(255, 214, 10, 255),
            error: UiColor::new(255, 69, 58, 255),
            info: UiColor::new(100, 210, 255, 255),
            border: UiColor::new(255, 255, 255, 26),
            border_strong: UiColor::new(255, 255, 255, 46),
            border_subtle: UiColor::new(255, 255, 255, 10),
            glass_tint: UiColor::new(10, 10, 10, 224),
            scrollbar_thumb: UiColor::new(255, 255, 255, 41),
            scrollbar_track: UiColor::new(255, 255, 255, 8),
        }
    }

    /// Sunset warm dark palette (spec-theme-sunset.md).
    pub fn sunset() -> Self {
        Self {
            background: UiColor::new(26, 16, 8, 255),
            surface: UiColor::new(255, 200, 120, 15),
            surface_hover: UiColor::new(255, 200, 120, 26),
            surface_active: UiColor::new(255, 200, 120, 36),
            text_primary: UiColor::new(255, 245, 230, 255),
            text_secondary: UiColor::new(255, 245, 230, 179),
            text_disabled: UiColor::new(255, 245, 230, 77),
            text_on_accent: UiColor::new(26, 14, 0, 255),
            accent: UiColor::new(255, 159, 10, 255),
            accent_hover: UiColor::new(255, 179, 60, 255),
            accent_active: UiColor::new(235, 139, 0, 255),
            success: UiColor::new(52, 199, 89, 255),
            warning: UiColor::new(255, 214, 10, 255),
            error: UiColor::new(255, 107, 107, 255),
            info: UiColor::new(255, 179, 64, 255),
            border: UiColor::new(255, 180, 80, 31),
            border_strong: UiColor::new(255, 180, 80, 56),
            border_subtle: UiColor::new(255, 180, 80, 15),
            glass_tint: UiColor::new(32, 22, 10, 184),
            scrollbar_thumb: UiColor::new(255, 200, 120, 46),
            scrollbar_track: UiColor::new(255, 200, 120, 10),
        }
    }

    /// Midday tarnished-white light palette (spec-theme-midday.md).
    pub fn midday() -> Self {
        Self {
            background: UiColor::new(245, 240, 232, 255),
            surface: UiColor::new(28, 27, 24, 10),
            surface_hover: UiColor::new(28, 27, 24, 18),
            surface_active: UiColor::new(28, 27, 24, 28),
            text_primary: UiColor::new(28, 27, 24, 255),
            text_secondary: UiColor::new(28, 27, 24, 158),
            text_disabled: UiColor::new(28, 27, 24, 77),
            text_on_accent: UiColor::white(),
            accent: UiColor::new(0, 113, 179, 255),
            accent_hover: UiColor::new(0, 133, 209, 255),
            accent_active: UiColor::new(0, 93, 149, 255),
            success: UiColor::new(36, 138, 61, 255),
            warning: UiColor::new(178, 80, 0, 255),
            error: UiColor::new(215, 0, 21, 255),
            info: UiColor::new(0, 113, 179, 255),
            border: UiColor::new(28, 27, 24, 26),
            border_strong: UiColor::new(28, 27, 24, 46),
            border_subtle: UiColor::new(28, 27, 24, 13),
            glass_tint: UiColor::new(248, 244, 238, 199),
            scrollbar_thumb: UiColor::new(28, 27, 24, 51),
            scrollbar_track: UiColor::new(28, 27, 24, 10),
        }
    }
}

/// A complete UI theme including colors and design tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiTheme {
    pub name: String,
    pub mode: ThemeMode,
    pub colors: ThemeColors,
    /// Base font size in logical pixels.
    pub font_size: f32,
    /// Base font family.
    pub font_family: String,
    /// Border radius scale.
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub radius_xl: f32,
    pub radius_full: f32,
    /// Spacing scale.
    pub spacing_xs: f32,
    pub spacing_sm: f32,
    pub spacing_md: f32,
    pub spacing_lg: f32,
    pub spacing_xl: f32,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::liquid_glass()
    }
}

impl UiTheme {
    /// Liquid Glass Standard dark theme.
    pub fn liquid_glass() -> Self {
        Self {
            name: "Liquid Glass".into(),
            mode: ThemeMode::Dark,
            colors: ThemeColors::liquid_glass_dark(),
            font_size: 13.0,
            font_family: "Inter".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Night OLED theme.
    pub fn night() -> Self {
        Self {
            name: "Night".into(),
            mode: ThemeMode::Dark,
            colors: ThemeColors::night(),
            font_size: 13.0,
            font_family: "Inter".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Sunset warm dark theme.
    pub fn sunset() -> Self {
        Self {
            name: "Sunset".into(),
            mode: ThemeMode::Dark,
            colors: ThemeColors::sunset(),
            font_size: 13.0,
            font_family: "Inter".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Midday tarnished-white light theme.
    pub fn midday() -> Self {
        Self {
            name: "Midday".into(),
            mode: ThemeMode::Light,
            colors: ThemeColors::midday(),
            font_size: 13.0,
            font_family: "Inter".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Whether this is a dark-mode theme.
    pub fn is_dark(&self) -> bool {
        matches!(self.mode, ThemeMode::Dark)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_modes() {
        assert!(UiTheme::liquid_glass().is_dark());
        assert!(UiTheme::night().is_dark());
        assert!(UiTheme::sunset().is_dark());
        assert!(!UiTheme::midday().is_dark());
    }

    #[test]
    fn test_default_is_liquid_glass() {
        let theme = UiTheme::default();
        assert_eq!(theme.name, "Liquid Glass");
    }
}
