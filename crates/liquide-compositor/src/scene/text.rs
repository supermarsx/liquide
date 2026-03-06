//! Text decoration and text shadow types.

use crate::pixel::Color;
use serde::{Deserialize, Serialize};

/// Text decoration specification (CSS text-decoration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDecoration {
    pub line: TextDecorationLine,
    pub style: TextDecorationStyle,
    pub color: Option<Color>,
    pub thickness: f32,
    /// text-underline-offset in px (default 0.0).
    #[serde(default)]
    pub underline_offset: f32,
    /// text-underline-position: under shifts line below descenders.
    #[serde(default)]
    pub underline_position_under: bool,
    /// text-decoration-skip-ink: auto (true) skips over glyph ink.
    #[serde(default = "default_skip_ink")]
    pub skip_ink: bool,
}

fn default_skip_ink() -> bool {
    true
}

/// Which line(s) to render for text-decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorationLine {
    None,
    Underline,
    Overline,
    LineThrough,
    /// Underline + Overline
    UnderlineOverline,
}

/// Visual style of the text decoration line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorationStyle {
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Text shadow specification (CSS text-shadow — multiple allowed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub color: Color,
}

impl Default for TextDecorationLine {
    fn default() -> Self {
        Self::None
    }
}

impl Default for TextDecorationStyle {
    fn default() -> Self {
        Self::Solid
    }
}
