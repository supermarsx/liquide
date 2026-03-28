//! Visual configuration for context menu rendering.
//!
//! [`MenuTheme`] controls colors, sizing, and decoration of menu panels and
//! items independently of the scene-graph builder.

use serde::{Deserialize, Serialize};

/// Pack RGBA components into a single `u32` (0xRRGGBBAA).
#[must_use]
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | a as u32
}

/// Extract RGBA components from a packed `u32`.
#[must_use]
pub const fn unpack_rgba(c: u32) -> (u8, u8, u8, u8) {
    (
        (c >> 24) as u8,
        ((c >> 16) & 0xFF) as u8,
        ((c >> 8) & 0xFF) as u8,
        (c & 0xFF) as u8,
    )
}

/// Complete visual style for a context menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuTheme {
    /// Background fill of the menu panel.
    pub background_color: u32,
    /// Highlight fill for the currently hovered item.
    pub hover_color: u32,
    /// Default text color for labels.
    pub text_color: u32,
    /// Text color for disabled / greyed-out items.
    pub disabled_color: u32,
    /// Color of separator lines.
    pub separator_color: u32,
    /// Text color for destructive ("danger") items.
    pub danger_color: u32,
    /// Text color for keyboard shortcut hints (right-aligned).
    pub shortcut_color: u32,
    /// Color for check marks and radio dots.
    pub check_color: u32,
    /// Color for submenu arrow indicators.
    pub arrow_color: u32,
    /// Height of a regular menu item row, in logical pixels.
    pub item_height: f32,
    /// Height of a separator row, in logical pixels.
    pub separator_height: f32,
    /// Icon rendering size (width = height).
    pub icon_size: f32,
    /// Horizontal padding inside the menu panel.
    pub padding: f32,
    /// Vertical padding at the top and bottom of the menu.
    pub vertical_padding: f32,
    /// Corner radius of the panel background.
    pub border_radius: f32,
    /// Font size for item labels.
    pub font_size: f32,
    /// Font size for shortcut hint text.
    pub shortcut_font_size: f32,
    /// Whether the panel draws a drop shadow.
    pub shadow: bool,
    /// Shadow blur radius (if `shadow` is true).
    pub shadow_blur: f32,
    /// Shadow color (if `shadow` is true).
    pub shadow_color: u32,
    /// Minimum width of the menu panel.
    pub min_width: f32,
    /// Maximum width of the menu panel.
    pub max_width: f32,
}

impl MenuTheme {
    /// A dark theme suitable for dark desktop environments.
    #[must_use]
    pub fn dark_theme() -> Self {
        Self {
            background_color: rgba(30, 30, 34, 240),
            hover_color: rgba(60, 60, 68, 255),
            text_color: rgba(230, 230, 235, 255),
            disabled_color: rgba(110, 110, 118, 255),
            separator_color: rgba(70, 70, 78, 180),
            danger_color: rgba(235, 70, 70, 255),
            shortcut_color: rgba(140, 140, 150, 255),
            check_color: rgba(80, 160, 255, 255),
            arrow_color: rgba(160, 160, 170, 255),
            item_height: 32.0,
            separator_height: 9.0,
            icon_size: 18.0,
            padding: 12.0,
            vertical_padding: 6.0,
            border_radius: 8.0,
            font_size: 13.0,
            shortcut_font_size: 12.0,
            shadow: true,
            shadow_blur: 16.0,
            shadow_color: rgba(0, 0, 0, 120),
            min_width: 180.0,
            max_width: 360.0,
        }
    }

    /// A light theme suitable for light desktop environments.
    #[must_use]
    pub fn default_theme() -> Self {
        Self {
            background_color: rgba(248, 248, 250, 245),
            hover_color: rgba(0, 100, 220, 30),
            text_color: rgba(30, 30, 36, 255),
            disabled_color: rgba(160, 160, 168, 255),
            separator_color: rgba(200, 200, 208, 200),
            danger_color: rgba(210, 50, 50, 255),
            shortcut_color: rgba(120, 120, 130, 255),
            check_color: rgba(0, 110, 220, 255),
            arrow_color: rgba(120, 120, 130, 255),
            item_height: 32.0,
            separator_height: 9.0,
            icon_size: 18.0,
            padding: 12.0,
            vertical_padding: 6.0,
            border_radius: 8.0,
            font_size: 13.0,
            shortcut_font_size: 12.0,
            shadow: true,
            shadow_blur: 12.0,
            shadow_color: rgba(0, 0, 0, 60),
            min_width: 180.0,
            max_width: 360.0,
        }
    }
}

impl Default for MenuTheme {
    fn default() -> Self {
        Self::default_theme()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_round_trip() {
        let c = rgba(10, 20, 30, 40);
        assert_eq!(unpack_rgba(c), (10, 20, 30, 40));
    }

    #[test]
    fn rgba_extremes() {
        assert_eq!(unpack_rgba(rgba(0, 0, 0, 0)), (0, 0, 0, 0));
        assert_eq!(unpack_rgba(rgba(255, 255, 255, 255)), (255, 255, 255, 255));
    }

    #[test]
    fn default_theme_has_sane_values() {
        let t = MenuTheme::default_theme();
        assert!(t.item_height > 0.0);
        assert!(t.font_size > 0.0);
        assert!(t.padding > 0.0);
        assert!(t.min_width > 0.0);
        assert!(t.min_width <= t.max_width);
    }

    #[test]
    fn dark_theme_differs_from_default() {
        let light = MenuTheme::default_theme();
        let dark = MenuTheme::dark_theme();
        assert_ne!(light.background_color, dark.background_color);
        assert_ne!(light.text_color, dark.text_color);
    }
}
