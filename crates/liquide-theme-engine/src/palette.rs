use crate::color::Color;

/// Complete color palette for a theme.
///
/// Contains semantic color slots that cover every common UI need: primary
/// actions, surfaces, text tones, feedback colors, and interactive states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorPalette {
    // ── Brand / accent ──
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,

    // ── Surfaces ──
    pub background: Color,
    pub surface: Color,

    // ── Feedback ──
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub info: Color,

    // ── Text ──
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,

    // ── Separators ──
    pub border: Color,
    pub divider: Color,
    pub shadow: Color,

    // ── Selection ──
    pub selection_bg: Color,
    pub selection_fg: Color,

    // ── Links ──
    pub link: Color,
    pub link_visited: Color,
}

impl ColorPalette {
    /// Linearly interpolate every color slot between two palettes.
    pub fn lerp(&self, other: &ColorPalette, t: f32) -> ColorPalette {
        ColorPalette {
            primary: self.primary.lerp(&other.primary, t),
            secondary: self.secondary.lerp(&other.secondary, t),
            accent: self.accent.lerp(&other.accent, t),
            background: self.background.lerp(&other.background, t),
            surface: self.surface.lerp(&other.surface, t),
            error: self.error.lerp(&other.error, t),
            warning: self.warning.lerp(&other.warning, t),
            success: self.success.lerp(&other.success, t),
            info: self.info.lerp(&other.info, t),
            text_primary: self.text_primary.lerp(&other.text_primary, t),
            text_secondary: self.text_secondary.lerp(&other.text_secondary, t),
            text_disabled: self.text_disabled.lerp(&other.text_disabled, t),
            border: self.border.lerp(&other.border, t),
            divider: self.divider.lerp(&other.divider, t),
            shadow: self.shadow.lerp(&other.shadow, t),
            selection_bg: self.selection_bg.lerp(&other.selection_bg, t),
            selection_fg: self.selection_fg.lerp(&other.selection_fg, t),
            link: self.link.lerp(&other.link, t),
            link_visited: self.link_visited.lerp(&other.link_visited, t),
        }
    }
}

impl Default for ColorPalette {
    /// A neutral dark palette (used as fallback).
    fn default() -> Self {
        Self {
            primary: Color::rgb(10, 132, 255),
            secondary: Color::rgb(100, 100, 110),
            accent: Color::rgb(10, 132, 255),
            background: Color::rgb(0, 0, 0),
            surface: Color::rgb(28, 28, 30),
            error: Color::rgb(255, 69, 58),
            warning: Color::rgb(255, 214, 10),
            success: Color::rgb(48, 209, 88),
            info: Color::rgb(100, 210, 255),
            text_primary: Color::rgb(255, 255, 255),
            text_secondary: Color::rgba(255, 255, 255, 153),
            text_disabled: Color::rgba(255, 255, 255, 77),
            border: Color::rgba(255, 255, 255, 26),
            divider: Color::rgba(255, 255, 255, 20),
            shadow: Color::rgba(0, 0, 0, 178),
            selection_bg: Color::rgba(10, 132, 255, 64),
            selection_fg: Color::rgb(255, 255, 255),
            link: Color::rgb(10, 132, 255),
            link_visited: Color::rgb(175, 82, 222),
        }
    }
}
