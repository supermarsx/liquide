//! Theme definitions and presets for the management UI.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A complete visual theme for the management UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme display name.
    pub name: String,
    /// Primary brand colour (CSS hex).
    pub primary_color: String,
    /// Accent / highlight colour (CSS hex).
    pub accent_color: String,
    /// Page background colour (CSS hex).
    pub background: String,
    /// Card / surface colour (CSS hex).
    pub surface: String,
    /// Default text colour (CSS hex).
    pub text: String,
    /// Error / danger colour (CSS hex).
    pub error: String,
    /// Success colour (CSS hex).
    pub success: String,
    /// Warning colour (CSS hex).
    pub warning: String,
    /// Sidebar width in pixels.
    pub sidebar_width: u32,
    /// Header height in pixels.
    pub header_height: u32,
    /// Border radius for cards and buttons in pixels.
    pub border_radius: u32,
    /// Font family stack.
    pub font_family: String,
    /// Base font size in pixels.
    pub font_size: u32,
}

impl Theme {
    /// Create a new theme with the given name and sensible placeholder values.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            primary_color: "#1976d2".to_string(),
            accent_color: "#00bcd4".to_string(),
            background: "#fafafa".to_string(),
            surface: "#ffffff".to_string(),
            text: "#212121".to_string(),
            error: "#d32f2f".to_string(),
            success: "#388e3c".to_string(),
            warning: "#f57c00".to_string(),
            sidebar_width: 240,
            header_height: 56,
            border_radius: 4,
            font_family: "Inter, system-ui, sans-serif".to_string(),
            font_size: 14,
        }
    }
}

/// Built-in theme presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemePreset {
    /// Default translucent "glass" theme.
    LiquidGlass,
    /// Dark mode.
    Dark,
    /// Light mode.
    Light,
    /// High-contrast accessibility mode.
    HighContrast,
}

/// All available presets.
pub const PRESET_ALL: &[ThemePreset] = &[
    ThemePreset::LiquidGlass,
    ThemePreset::Dark,
    ThemePreset::Light,
    ThemePreset::HighContrast,
];

impl ThemePreset {
    /// Convert this preset into a fully populated [`Theme`].
    #[must_use]
    pub fn to_theme(self) -> Theme {
        match self {
            Self::LiquidGlass => Theme {
                name: "liquid-glass".to_string(),
                primary_color: "#0a84ff".to_string(),
                accent_color: "#30d158".to_string(),
                background: "#1c1c1e".to_string(),
                surface: "rgba(255,255,255,0.08)".to_string(),
                text: "#f5f5f7".to_string(),
                error: "#ff453a".to_string(),
                success: "#30d158".to_string(),
                warning: "#ffd60a".to_string(),
                sidebar_width: 260,
                header_height: 52,
                border_radius: 12,
                font_family: "Inter, system-ui, sans-serif".to_string(),
                font_size: 14,
            },
            Self::Dark => Theme {
                name: "dark".to_string(),
                primary_color: "#90caf9".to_string(),
                accent_color: "#80deea".to_string(),
                background: "#121212".to_string(),
                surface: "#1e1e1e".to_string(),
                text: "#e0e0e0".to_string(),
                error: "#ef5350".to_string(),
                success: "#66bb6a".to_string(),
                warning: "#ffa726".to_string(),
                sidebar_width: 240,
                header_height: 56,
                border_radius: 8,
                font_family: "Inter, system-ui, sans-serif".to_string(),
                font_size: 14,
            },
            Self::Light => Theme {
                name: "light".to_string(),
                primary_color: "#1976d2".to_string(),
                accent_color: "#00bcd4".to_string(),
                background: "#fafafa".to_string(),
                surface: "#ffffff".to_string(),
                text: "#212121".to_string(),
                error: "#d32f2f".to_string(),
                success: "#388e3c".to_string(),
                warning: "#f57c00".to_string(),
                sidebar_width: 240,
                header_height: 56,
                border_radius: 4,
                font_family: "Inter, system-ui, sans-serif".to_string(),
                font_size: 14,
            },
            Self::HighContrast => Theme {
                name: "high-contrast".to_string(),
                primary_color: "#ffff00".to_string(),
                accent_color: "#00ffff".to_string(),
                background: "#000000".to_string(),
                surface: "#1a1a1a".to_string(),
                text: "#ffffff".to_string(),
                error: "#ff0000".to_string(),
                success: "#00ff00".to_string(),
                warning: "#ffff00".to_string(),
                sidebar_width: 260,
                header_height: 60,
                border_radius: 2,
                font_family: "monospace".to_string(),
                font_size: 16,
            },
        }
    }
}

impl fmt::Display for ThemePreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiquidGlass => write!(f, "liquid-glass"),
            Self::Dark => write!(f, "dark"),
            Self::Light => write!(f, "light"),
            Self::HighContrast => write!(f, "high-contrast"),
        }
    }
}
