//! Style primitives — borders, shadows, box model.

use crate::color::UiColor;
use serde::{Deserialize, Serialize};

/// Border style for a widget.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BorderStyle {
    pub width: f32,
    pub color: UiColor,
    pub radius: f32,
}

impl BorderStyle {
    pub const NONE: Self = Self {
        width: 0.0,
        color: UiColor::transparent(),
        radius: 0.0,
    };

    pub fn new(width: f32, color: UiColor, radius: f32) -> Self {
        Self { width, color, radius }
    }
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self::NONE
    }
}

/// Box shadow for depth / elevation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: UiColor,
}

impl BoxShadow {
    pub const NONE: Self = Self {
        offset_x: 0.0,
        offset_y: 0.0,
        blur: 0.0,
        spread: 0.0,
        color: UiColor::transparent(),
    };

    /// Standard elevation shadow.
    pub fn elevation(level: u8) -> Self {
        let blur = level as f32 * 4.0;
        let alpha = (40 + level as u16 * 10).min(180) as u8;
        Self {
            offset_x: 0.0,
            offset_y: level as f32 * 1.5,
            blur,
            spread: 0.0,
            color: UiColor::rgba(0, 0, 0, alpha),
        }
    }
}

impl Default for BoxShadow {
    fn default() -> Self {
        Self::NONE
    }
}

/// Complete style sheet for a widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleSheet {
    pub background: UiColor,
    pub foreground: UiColor,
    pub border: BorderStyle,
    pub shadow: BoxShadow,
    pub opacity: f32,
}

impl Default for StyleSheet {
    fn default() -> Self {
        Self {
            background: UiColor::transparent(),
            foreground: UiColor::black(),
            border: BorderStyle::NONE,
            shadow: BoxShadow::NONE,
            opacity: 1.0,
        }
    }
}
