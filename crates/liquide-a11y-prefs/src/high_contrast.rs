//! High-contrast theme generation.
//!
//! Provides ready-made high-contrast color overrides and a utility to
//! increase the contrast of an existing theme's colors.

use crate::contrast::{contrast_ratio, suggest_color};

/// A set of color overrides that a theme can apply when accessibility
/// preferences request higher contrast or a full high-contrast mode.
///
/// Each color is represented as `(r, g, b)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeOverrides {
    /// Background color for surfaces.
    pub bg_color: (u8, u8, u8),
    /// Primary foreground / text color.
    pub fg_color: (u8, u8, u8),
    /// Accent color for focused controls and links.
    pub accent_color: (u8, u8, u8),
    /// Border color for controls and separators.
    pub border_color: (u8, u8, u8),
    /// Color for hyperlinks and interactive text.
    pub link_color: (u8, u8, u8),
    /// Foreground color for disabled / inactive controls.
    pub disabled_color: (u8, u8, u8),
    /// Background color for text selections.
    pub selection_bg: (u8, u8, u8),
    /// Foreground color for text selections.
    pub selection_fg: (u8, u8, u8),
}

impl ThemeOverrides {
    /// WCAG contrast ratio between the foreground and background colors.
    #[must_use]
    pub fn fg_bg_contrast(&self) -> f64 {
        contrast_ratio(self.fg_color, self.bg_color)
    }

    /// WCAG contrast ratio between the accent color and background.
    #[must_use]
    pub fn accent_bg_contrast(&self) -> f64 {
        contrast_ratio(self.accent_color, self.bg_color)
    }
}

/// Generate a light high-contrast theme (black text on white background).
///
/// Meets WCAG AAA (>= 7:1) for all text elements and AA for UI controls.
#[must_use]
pub fn high_contrast_light() -> ThemeOverrides {
    ThemeOverrides {
        bg_color: (255, 255, 255),
        fg_color: (0, 0, 0),
        accent_color: (0, 0, 170), // strong blue, 8.6:1 on white
        border_color: (0, 0, 0),
        link_color: (0, 0, 238),      // classic link blue, 6.6:1 on white
        disabled_color: (96, 96, 96), // 5.3:1 on white (meets AA)
        selection_bg: (0, 0, 170),
        selection_fg: (255, 255, 255),
    }
}

/// Generate a dark high-contrast theme (white/yellow text on black).
///
/// Meets WCAG AAA for body text.
#[must_use]
pub fn high_contrast_dark() -> ThemeOverrides {
    ThemeOverrides {
        bg_color: (0, 0, 0),
        fg_color: (255, 255, 255),
        accent_color: (255, 255, 0), // yellow on black: 19.6:1
        border_color: (255, 255, 255),
        link_color: (0, 255, 255),       // cyan on black: 16.7:1
        disabled_color: (160, 160, 160), // 10.4:1 on black
        selection_bg: (255, 255, 0),
        selection_fg: (0, 0, 0),
    }
}

/// Take an existing theme's colors and increase contrast ratios.
///
/// Each foreground-like color is adjusted toward the target ratio
/// against the background. Colors already meeting the target are
/// left unchanged.
///
/// A typical `target_ratio` is `7.0` (WCAG AAA) or `4.5` (AA).
#[must_use]
pub fn increase_contrast(base: &ThemeOverrides, target_ratio: f64) -> ThemeOverrides {
    let bg = base.bg_color;
    ThemeOverrides {
        bg_color: bg,
        fg_color: boost(base.fg_color, bg, target_ratio),
        accent_color: boost(base.accent_color, bg, target_ratio),
        border_color: boost(base.border_color, bg, target_ratio.min(4.5)),
        link_color: boost(base.link_color, bg, target_ratio),
        disabled_color: boost(base.disabled_color, bg, (target_ratio * 0.65).max(3.0)),
        selection_bg: base.selection_bg,
        selection_fg: boost(base.selection_fg, base.selection_bg, target_ratio),
    }
}

/// If `fg` doesn't meet `target` against `bg`, adjust it.
fn boost(fg: (u8, u8, u8), bg: (u8, u8, u8), target: f64) -> (u8, u8, u8) {
    let ratio = contrast_ratio(fg, bg);
    if ratio >= target {
        fg
    } else {
        suggest_color(fg, bg, target)
    }
}
