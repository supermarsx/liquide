//! Built-in 8x16 bitmap font covering printable ASCII (32..=126).
//!
//! Each glyph is 8 pixels wide and 16 pixels tall, stored as 16 bytes
//! (one byte per row, MSB = leftmost pixel).  The font data is
//! embedded as a compile-time constant — no runtime allocation or
//! file I/O required.
//!
//! Glyph patterns are in the style of the classic VGA/CP437 ROM font.
//!
//! Antialiasing is achieved via a greyscale glyph cache: each 1-bit
//! glyph is upscaled 4x, Gaussian-blurred, and downsampled back to
//! 8x16 alpha values.  The cache is computed once on first use.

mod data;
mod render;

#[cfg(test)]
mod tests;

pub use render::draw_text;

use data::{FALLBACK_GLYPH, FONT_DATA};

/// A built-in 8x16 bitmap font.
///
/// This is a zero-sized type — all glyph data lives in a static table.
/// Obtain an instance via [`BitmapFont::new`] or the provided
/// [`DEFAULT`](BitmapFont::DEFAULT) constant.
#[derive(Debug, Clone, Copy)]
pub struct BitmapFont;

impl BitmapFont {
    /// Glyph cell width in pixels.
    pub const GLYPH_WIDTH: u32 = 8;

    /// Glyph cell height in pixels.
    pub const GLYPH_HEIGHT: u32 = 16;

    /// A ready-to-use constant instance.
    pub const DEFAULT: BitmapFont = BitmapFont;

    /// Create a new `BitmapFont` (all instances are identical).
    #[must_use]
    pub const fn new() -> Self {
        BitmapFont
    }

    /// Return the 16 row-bytes for `ch`.
    ///
    /// Each byte encodes one row of 8 pixels (MSB = leftmost).
    /// Characters outside printable ASCII (32..=126) return a
    /// solid filled-block glyph (all `0xFF` bytes).
    #[must_use]
    pub fn glyph(&self, ch: char) -> &[u8; 16] {
        let code = ch as u32;
        if code >= 32 && code <= 126 {
            &FONT_DATA[(code - 32) as usize]
        } else {
            &FALLBACK_GLYPH
        }
    }

    /// Measure the pixel dimensions of a string.
    ///
    /// Returns `(width, height)` where width is the longest line
    /// (in pixels) and height accounts for newlines.  An empty
    /// string returns `(0, 0)`.
    #[must_use]
    pub fn measure_text(&self, text: &str) -> (u32, u32) {
        if text.is_empty() {
            return (0, 0);
        }
        let mut max_width: u32 = 0;
        let mut current_width: u32 = 0;
        let mut lines: u32 = 1;
        for ch in text.chars() {
            if ch == '\n' {
                max_width = max_width.max(current_width);
                current_width = 0;
                lines += 1;
            } else {
                current_width += Self::GLYPH_WIDTH;
            }
        }
        max_width = max_width.max(current_width);
        (max_width, lines * Self::GLYPH_HEIGHT)
    }
}
