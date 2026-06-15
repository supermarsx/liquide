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

/// CSS `word-break` mode carried on a `Text` scene node.
///
/// Controls where soft line breaks may be inserted inside a run of text.
/// Mirrors `liquide_style_engine::computed::WordBreak`; the scene layer keeps
/// its own copy so the compositor crate has no dependency on the style engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordBreak {
    /// Default: break only at allowed break points (whitespace / hyphens).
    Normal,
    /// Break between any two characters (CJK-style), even mid-word.
    BreakAll,
    /// Never break within CJK runs (no effect on non-CJK here).
    KeepAll,
    /// Like `Normal`, but allow long unbreakable words to break to avoid overflow.
    BreakWord,
}

impl Default for WordBreak {
    fn default() -> Self {
        Self::Normal
    }
}

/// Position of text-emphasis marks relative to the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextEmphasisPosition {
    /// Marks drawn above the text (horizontal writing mode default).
    Over,
    /// Marks drawn below the text.
    Under,
}

impl Default for TextEmphasisPosition {
    fn default() -> Self {
        Self::Over
    }
}

/// CSS `text-emphasis` specification carried on a `Text` scene node.
///
/// Renders a small mark (dot, circle, sesame, etc.) over or under each
/// rendered character. The `mark` string is the literal glyph(s) to draw
/// (already resolved from the CSS keyword/string by the producer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEmphasis {
    /// The literal mark glyph(s) to render above/below each character
    /// (e.g. "•" for `dot`, "○" for `open circle`).
    pub mark: String,
    /// Mark color. `None` = inherit the text color.
    pub color: Option<Color>,
    /// Whether the mark is drawn over or under the text.
    pub position: TextEmphasisPosition,
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
