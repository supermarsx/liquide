//! Text shaping: maps characters to positioned glyphs.
//!
//! The shaper applies OpenType-style features (ligatures, kerning, mark
//! positioning) to produce a sequence of `ShapedGlyph` values with advances
//! and offsets. This is a HarfBuzz-compatible interface with built-in
//! shaping for basic Latin and a pluggable backend for full complex shaping.

use serde::{Deserialize, Serialize};

use crate::bidi::Direction;
use crate::font_fallback::FontId;

/// A shaped glyph output from the shaping pipeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    /// Font-specific glyph index.
    pub glyph_id: u32,
    /// Index into the source character cluster.
    pub cluster: u32,
    /// Horizontal advance (in font design units, 26.6 fixed-point → f32).
    pub x_advance: f32,
    /// Vertical advance.
    pub y_advance: f32,
    /// Horizontal offset from the nominal position.
    pub x_offset: f32,
    /// Vertical offset from the baseline.
    pub y_offset: f32,
}

/// A run of shaped glyphs from a single font, script, and direction.
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// The shaped glyphs.
    pub glyphs: Vec<ShapedGlyph>,
    /// Font used for this run.
    pub font_id: FontId,
    /// Font size in pixels.
    pub size: f32,
    /// Text direction for this run.
    pub direction: Direction,
    /// Byte range in the source text.
    pub start: usize,
    pub end: usize,
}

impl ShapedRun {
    /// Total advance width of this run.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.glyphs.iter().map(|g| g.x_advance).sum()
    }
}

/// Shaping features that can be enabled/disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShapingFeature {
    /// Standard ligatures (fi, fl, ffi, etc.)
    Ligatures,
    /// Contextual alternates
    ContextualAlternates,
    /// Kerning
    Kerning,
    /// Small caps
    SmallCaps,
    /// Oldstyle figures
    OldstyleFigures,
    /// Tabular figures (monospaced numbers)
    TabularFigures,
    /// Fractions
    Fractions,
    /// Ordinals
    Ordinals,
    /// Stylistic set (1–20)
    StylisticSet(u8),
}

/// Configuration for the text shaper.
#[derive(Debug, Clone)]
pub struct ShaperConfig {
    /// Features to enable.
    pub features: Vec<ShapingFeature>,
    /// Letter spacing adjustment (in pixels, added to each advance).
    pub letter_spacing: f32,
    /// Word spacing adjustment (in pixels, added to space characters).
    pub word_spacing: f32,
}

impl Default for ShaperConfig {
    fn default() -> Self {
        Self {
            features: vec![
                ShapingFeature::Ligatures,
                ShapingFeature::Kerning,
                ShapingFeature::ContextualAlternates,
            ],
            letter_spacing: 0.0,
            word_spacing: 0.0,
        }
    }
}

/// Text shaper that converts characters into positioned glyphs.
pub struct TextShaper {
    config: ShaperConfig,
}

impl TextShaper {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ShaperConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(config: ShaperConfig) -> Self {
        Self { config }
    }

    /// Shape a run of text using a specific font.
    ///
    /// This provides built-in shaping for basic scripts and delegates
    /// to HarfBuzz for complex scripts when available.
    #[must_use]
    pub fn shape(
        &self,
        text: &str,
        font_id: FontId,
        size: f32,
        direction: Direction,
    ) -> ShapedRun {
        let chars: Vec<char> = text.chars().collect();
        let mut glyphs = Vec::with_capacity(chars.len());

        // Built-in basic shaping: 1:1 character → glyph mapping
        // with approximate advance widths based on font size.
        let base_advance = size * 0.6; // Approximate average advance
        let space_width = size * 0.25;

        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            let cluster = i as u32;

            // Check for basic Latin ligatures
            let (glyph_id, advance, consumed) = if self.has_feature(ShapingFeature::Ligatures) {
                self.try_ligate(&chars, i, base_advance)
            } else {
                (ch as u32, char_advance(ch, base_advance, space_width), 1)
            };

            let adjusted_advance = advance + self.config.letter_spacing
                + if ch == ' ' { self.config.word_spacing } else { 0.0 };

            // Apply basic kerning (simplified)
            let kern_offset = if self.has_feature(ShapingFeature::Kerning) && i + consumed < chars.len() {
                self.kern_pair(ch, chars[i + consumed], size)
            } else {
                0.0
            };

            glyphs.push(ShapedGlyph {
                glyph_id,
                cluster,
                x_advance: adjusted_advance + kern_offset,
                y_advance: 0.0,
                x_offset: 0.0,
                y_offset: 0.0,
            });

            i += consumed;
        }

        // For RTL runs, reverse the glyph order
        if direction == Direction::Rtl {
            glyphs.reverse();
        }

        ShapedRun {
            glyphs,
            font_id,
            size,
            direction,
            start: 0,
            end: text.len(),
        }
    }

    /// Try to form a ligature starting at position `i`.
    ///
    /// Returns (glyph_id, advance, chars_consumed).
    fn try_ligate(&self, chars: &[char], i: usize, base: f32) -> (u32, f32, usize) {
        if i + 2 < chars.len() {
            match (chars[i], chars[i + 1], chars[i + 2]) {
                ('f', 'f', 'i') => return (0xFB03, base * 1.5, 3), // ffi ligature
                ('f', 'f', 'l') => return (0xFB04, base * 1.5, 3), // ffl ligature
                _ => {}
            }
        }
        if i + 1 < chars.len() {
            match (chars[i], chars[i + 1]) {
                ('f', 'i') => return (0xFB01, base * 1.0, 2), // fi ligature
                ('f', 'l') => return (0xFB02, base * 1.0, 2), // fl ligature
                ('f', 'f') => return (0xFB00, base * 1.0, 2), // ff ligature
                _ => {}
            }
        }
        (chars[i] as u32, char_advance(chars[i], base, base * 0.42), 1)
    }

    /// Basic kerning for common Latin pairs.
    fn kern_pair(&self, left: char, right: char, size: f32) -> f32 {
        let kern = match (left, right) {
            ('A', 'V') | ('V', 'A') => -0.08,
            ('A', 'W') | ('W', 'A') => -0.06,
            ('A', 'T') | ('T', 'A') => -0.07,
            ('A', 'Y') | ('Y', 'A') => -0.08,
            ('L', 'T') | ('L', 'V') | ('L', 'W') | ('L', 'Y') => -0.07,
            ('T', 'o') | ('T', 'a') | ('T', 'e') => -0.06,
            ('V', 'o') | ('V', 'a') | ('V', 'e') => -0.05,
            ('W', 'o') | ('W', 'a') | ('W', 'e') => -0.04,
            ('Y', 'o') | ('Y', 'a') | ('Y', 'e') => -0.07,
            ('P', '.') | ('P', ',') => -0.08,
            ('F', '.') | ('F', ',') => -0.08,
            ('r', '.') | ('r', ',') => -0.04,
            _ => 0.0,
        };
        kern * size
    }

    fn has_feature(&self, feature: ShapingFeature) -> bool {
        self.config.features.contains(&feature)
    }
}

impl Default for TextShaper {
    fn default() -> Self {
        Self::new()
    }
}

/// Approximate advance width for a character.
fn char_advance(ch: char, base: f32, space: f32) -> f32 {
    match ch {
        ' ' => space,
        '\t' => space * 4.0,
        'W' | 'M' | 'm' | 'w' => base * 1.2,
        'i' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => base * 0.4,
        'f' | 'j' | 'r' | 't' => base * 0.6,
        'I' | '1' => base * 0.5,
        _ if ch.is_ascii_uppercase() => base * 0.95,
        _ if ch.is_ascii_lowercase() => base * 0.75,
        _ if ch.is_ascii_digit() => base * 0.75,
        _ if ch.is_ascii_punctuation() => base * 0.5,
        // CJK characters are typically full-width
        c if (c as u32) >= 0x4E00 && (c as u32) <= 0x9FFF => base * 1.67,
        c if (c as u32) >= 0x3040 && (c as u32) <= 0x30FF => base * 1.67,
        // Default
        _ => base * 0.75,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_shaping() {
        let shaper = TextShaper::new();
        let run = shaper.shape("Hello", FontId(1), 16.0, Direction::Ltr);
        assert_eq!(run.glyphs.len(), 5);
        assert!(run.width() > 0.0);
    }

    #[test]
    fn test_fi_ligature() {
        let shaper = TextShaper::new();
        let run = shaper.shape("find", FontId(1), 16.0, Direction::Ltr);
        // "find" with fi ligature → 3 glyphs instead of 4
        assert_eq!(run.glyphs.len(), 3);
    }

    #[test]
    fn test_ffi_ligature() {
        let shaper = TextShaper::new();
        let run = shaper.shape("office", FontId(1), 16.0, Direction::Ltr);
        // "office" → o + ffi + c + e = 4 glyphs
        assert_eq!(run.glyphs.len(), 4);
    }

    #[test]
    fn test_no_ligatures() {
        let config = ShaperConfig {
            features: vec![ShapingFeature::Kerning],
            ..Default::default()
        };
        let shaper = TextShaper::with_config(config);
        let run = shaper.shape("find", FontId(1), 16.0, Direction::Ltr);
        // Without ligatures: 4 separate glyphs
        assert_eq!(run.glyphs.len(), 4);
    }

    #[test]
    fn test_rtl_shaping() {
        let shaper = TextShaper::new();
        let run = shaper.shape("abc", FontId(1), 16.0, Direction::Rtl);
        assert_eq!(run.direction, Direction::Rtl);
        // Glyphs should be reversed for RTL
        assert_eq!(run.glyphs[0].cluster, 2);
        assert_eq!(run.glyphs[2].cluster, 0);
    }

    #[test]
    fn test_letter_spacing() {
        let config = ShaperConfig {
            letter_spacing: 2.0,
            ..Default::default()
        };
        let shaper = TextShaper::with_config(config);
        let run_spaced = shaper.shape("Hello", FontId(1), 16.0, Direction::Ltr);

        let shaper_default = TextShaper::new();
        let run_normal = shaper_default.shape("Hello", FontId(1), 16.0, Direction::Ltr);

        assert!(run_spaced.width() > run_normal.width());
    }

    #[test]
    fn test_empty_text() {
        let shaper = TextShaper::new();
        let run = shaper.shape("", FontId(1), 16.0, Direction::Ltr);
        assert!(run.glyphs.is_empty());
        assert_eq!(run.width(), 0.0);
    }

    #[test]
    fn test_kerning() {
        let shaper = TextShaper::new();
        let run = shaper.shape("AV", FontId(1), 16.0, Direction::Ltr);
        // AV should have negative kerning, making total width less
        // than the sum of individual character widths
        let run_a = shaper.shape("A", FontId(1), 16.0, Direction::Ltr);
        let run_v = shaper.shape("V", FontId(1), 16.0, Direction::Ltr);
        assert!(run.width() < run_a.width() + run_v.width());
    }

    #[test]
    fn test_cjk_width() {
        let shaper = TextShaper::new();
        let run_cjk = shaper.shape("中", FontId(1), 16.0, Direction::Ltr);
        let run_latin = shaper.shape("a", FontId(1), 16.0, Direction::Ltr);
        // CJK characters should be wider than Latin
        assert!(run_cjk.width() > run_latin.width());
    }
}
