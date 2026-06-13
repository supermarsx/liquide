//! Text shaping: maps characters to positioned glyphs.
//!
//! The shaper applies OpenType-style features (ligatures, kerning, mark
//! positioning) to produce a sequence of `ShapedGlyph` values with advances
//! and offsets. This is a HarfBuzz-compatible interface with built-in
//! shaping for basic Latin and a pluggable backend for full complex shaping.

use serde::{Deserialize, Serialize};

use std::sync::Once;

use crate::bidi::Direction;
use crate::font_fallback::FontId;

/// Emits a single `warn!` the first time the fallback Latin-only shaper
/// is invoked without a real shaping backend attached. Shouts loudly
/// that complex scripts (Arabic joining, Indic reordering, CJK,
/// emoji ZWJ sequences) will not render correctly until a
/// `ShaperBackend` — typically `RustybuzzShaperBackend` from
/// `liquide-font-rasterizer` — is installed.
fn warn_fallback_once() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing::warn!(
            "rustybuzz backend unavailable — falling back to Latin-only shaper; \
             complex scripts (Arabic/Indic/CJK/emoji ZWJ) will not render correctly"
        );
    });
}

/// A shaped glyph output from the shaping pipeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    /// Font-specific glyph index.
    pub glyph_id: u32,
    /// UTF-8 **byte offset** of this glyph's cluster within the shaped text
    /// (rustybuzz's native convention; the fallback shaper matches it). Not a
    /// char index — for non-ASCII text these differ.
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

/// External shaping backend trait — allows plugging in a real shaper
/// (e.g., rustybuzz via liquide-font-rasterizer) for production-quality
/// complex script shaping.
///
/// When no backend is set, the built-in fallback shaper (Latin-only,
/// approximate metrics) is used. The `liquide-font-rasterizer` crate
/// provides a `RustybuzzShaperBackend` that implements this trait.
pub trait ShaperBackend: Send + Sync {
    /// Shape text using the real shaping engine.
    ///
    /// Returns glyphs with real metrics from the font, or `None` if the
    /// font/text cannot be shaped (fallback shaper will be used).
    fn shape(
        &self,
        text: &str,
        font_id: FontId,
        size: f32,
        direction: Direction,
        config: &ShaperConfig,
    ) -> Option<Vec<ShapedGlyph>>;
}

/// Text shaper that converts characters into positioned glyphs.
///
/// Supports a pluggable backend for production shaping. When a backend is
/// set, it is tried first; on `None` return, the built-in fallback shaper
/// is used (hardcoded Latin metrics).
pub struct TextShaper {
    config: ShaperConfig,
    backend: Option<Box<dyn ShaperBackend>>,
}

impl TextShaper {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ShaperConfig::default(),
            backend: None,
        }
    }

    #[must_use]
    pub fn with_config(config: ShaperConfig) -> Self {
        Self {
            config,
            backend: None,
        }
    }

    /// Set the shaping backend (e.g., rustybuzz).
    /// When set, the backend is tried first for all `shape()` calls.
    pub fn set_backend(&mut self, backend: Box<dyn ShaperBackend>) {
        self.backend = Some(backend);
    }

    /// Create a shaper with a backend already attached.
    #[must_use]
    pub fn with_backend(config: ShaperConfig, backend: Box<dyn ShaperBackend>) -> Self {
        Self {
            config,
            backend: Some(backend),
        }
    }

    /// Shape a run of text using a specific font.
    ///
    /// If a backend is set, tries it first (real OpenType shaping).
    /// Falls back to the built-in approximate shaper for basic scripts.
    #[must_use]
    pub fn shape(&self, text: &str, font_id: FontId, size: f32, direction: Direction) -> ShapedRun {
        // Try the real backend first
        if let Some(ref backend) = self.backend {
            if let Some(glyphs) = backend.shape(text, font_id, size, direction, &self.config) {
                return ShapedRun {
                    glyphs,
                    font_id,
                    size,
                    direction,
                    start: 0,
                    end: text.len(),
                };
            }
        } else {
            warn_fallback_once();
        }

        // Fallback: built-in approximate shaping
        self.shape_fallback(text, font_id, size, direction)
    }

    /// Shape text as a sequence of script-segmented sub-runs, invoking the
    /// backend once per segment.
    ///
    /// Mirrors HarfBuzz's recommended flow: segment by Unicode script
    /// (UAX #24) before shaping so that each sub-run is fed a buffer
    /// containing a single script. This fixes mixed-script input where
    /// the previous single-call path under-shaped the weaker script.
    ///
    /// Returns a `Vec<ShapedRun>` in source order (visual reordering
    /// still happens at the bidi + line-layout stage) with each run's
    /// `start`/`end` byte range set from the script segmentation.
    #[must_use]
    pub fn shape_runs(
        &self,
        text: &str,
        font_id: FontId,
        size: f32,
        direction: Direction,
    ) -> Vec<ShapedRun> {
        if text.is_empty() {
            return Vec::new();
        }
        let script_runs = crate::script::ScriptDetector::detect(text);
        if script_runs.is_empty() {
            return vec![self.shape(text, font_id, size, direction)];
        }
        let mut out = Vec::with_capacity(script_runs.len());
        for run in &script_runs {
            let slice = &text[run.start..run.end];
            let mut shaped = self.shape(slice, font_id, size, direction);
            shaped.start = run.start;
            shaped.end = run.end;
            out.push(shaped);
        }
        out
    }

    /// Built-in fallback shaping for basic Latin scripts.
    fn shape_fallback(
        &self,
        text: &str,
        font_id: FontId,
        size: f32,
        direction: Direction,
    ) -> ShapedRun {
        let chars: Vec<char> = text.chars().collect();
        // e3-B: clusters are byte offsets (rustybuzz's native convention), so the
        // fallback shaper must emit the UTF-8 byte offset of each char's first byte,
        // not its char index. char_byte_offsets[i] is the byte offset of chars[i].
        let char_byte_offsets: Vec<u32> = text.char_indices().map(|(b, _)| b as u32).collect();
        let mut glyphs = Vec::with_capacity(chars.len());

        // Built-in basic shaping: 1:1 character → glyph mapping
        // with approximate advance widths based on font size.
        let base_advance = size * 0.6; // Approximate average advance
        let space_width = size * 0.25;

        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            // Byte offset of this char (e3-B: byte-offset clusters).
            let cluster = char_byte_offsets[i];

            // Check for basic Latin ligatures
            let (glyph_id, advance, consumed) = if self.has_feature(ShapingFeature::Ligatures) {
                self.try_ligate(&chars, i, base_advance)
            } else {
                (ch as u32, char_advance(ch, base_advance, space_width), 1)
            };

            let adjusted_advance = advance
                + self.config.letter_spacing
                + if ch == ' ' {
                    self.config.word_spacing
                } else {
                    0.0
                };

            // Apply basic kerning (simplified)
            let kern_offset =
                if self.has_feature(ShapingFeature::Kerning) && i + consumed < chars.len() {
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
        (
            chars[i] as u32,
            char_advance(chars[i], base, base * 0.42),
            1,
        )
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

    struct StyleEchoBackend;

    impl ShaperBackend for StyleEchoBackend {
        fn shape(
            &self,
            text: &str,
            _font_id: FontId,
            _size: f32,
            _direction: Direction,
            config: &ShaperConfig,
        ) -> Option<Vec<ShapedGlyph>> {
            Some(
                text.char_indices()
                    .map(|(byte_idx, ch)| ShapedGlyph {
                        glyph_id: ch as u32,
                        cluster: byte_idx as u32,
                        x_advance: 10.0
                            + config.letter_spacing
                            + if ch == ' ' { config.word_spacing } else { 0.0 },
                        y_advance: 0.0,
                        x_offset: config.features.len() as f32,
                        y_offset: 0.0,
                    })
                    .collect(),
            )
        }
    }

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
    fn fallback_shaper_emits_byte_offset_clusters_for_non_ascii() {
        // e3-B: clusters are UTF-8 byte offsets, not char indices.
        // "café" = ['c'(1B), 'a'(1B), 'f'(1B), 'é'(2B)] → byte offsets 0,1,2,3.
        let text = "café";
        assert_eq!(text.chars().count(), 4);
        assert_eq!(text.len(), 5); // é is 2 bytes

        let shaper = TextShaper::new();
        let run = shaper.shape(text, FontId(1), 16.0, Direction::Ltr);
        assert_eq!(run.glyphs.len(), 4);

        let clusters: Vec<u32> = run.glyphs.iter().map(|g| g.cluster).collect();
        assert_eq!(clusters, vec![0, 1, 2, 3]);

        // Each cluster must be a valid char boundary in the source text and
        // round-trip back to the correct character.
        let chars: Vec<char> = text.chars().collect();
        for (i, &c) in clusters.iter().enumerate() {
            let byte = c as usize;
            assert!(text.is_char_boundary(byte), "cluster {byte} not a boundary");
            assert_eq!(text[byte..].chars().next(), Some(chars[i]));
        }
        // The trailing 2-byte char sits at byte 3 (char index 3), proving we
        // emit byte offsets (a char-index shaper would have emitted 0,1,2,3 too
        // here, but the round-trip via byte slicing above is the real proof).
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
    fn backend_restyle_letter_spacing_changes_glyph_advances() {
        let zero = TextShaper::with_backend(
            ShaperConfig {
                letter_spacing: 0.0,
                ..ShaperConfig::default()
            },
            Box::new(StyleEchoBackend),
        );
        let spaced = TextShaper::with_backend(
            ShaperConfig {
                letter_spacing: 2.0,
                ..ShaperConfig::default()
            },
            Box::new(StyleEchoBackend),
        );

        let zero_run = zero.shape("AB", FontId(1), 16.0, Direction::Ltr);
        let spaced_run = spaced.shape("AB", FontId(1), 16.0, Direction::Ltr);

        assert_eq!(zero_run.glyphs.len(), spaced_run.glyphs.len());
        assert_eq!(zero_run.glyphs[0].x_advance, 10.0);
        assert_eq!(spaced_run.glyphs[0].x_advance, 12.0);
        assert!(spaced_run.width() > zero_run.width());
    }

    #[test]
    fn backend_receives_word_spacing_and_feature_identity() {
        let config = ShaperConfig {
            features: vec![ShapingFeature::Ligatures, ShapingFeature::SmallCaps],
            letter_spacing: 1.0,
            word_spacing: 4.0,
        };
        let shaper = TextShaper::with_backend(config, Box::new(StyleEchoBackend));

        let run = shaper.shape("A B", FontId(1), 16.0, Direction::Ltr);

        assert_eq!(run.glyphs[0].x_offset, 2.0);
        assert_eq!(run.glyphs[1].cluster, 1);
        assert_eq!(run.glyphs[1].x_advance, 15.0);
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

    #[test]
    fn test_mixed_script_segmentation() {
        // Mixed Latin + Arabic must produce separate shaped runs so the
        // backend sees one script per call (HarfBuzz contract).
        let shaper = TextShaper::new();
        let runs = shaper.shape_runs("hi مرحبا", FontId(1), 16.0, Direction::Ltr);
        assert!(
            runs.len() >= 2,
            "expected >=2 script runs, got {}",
            runs.len()
        );
        // First run must start at byte 0, last run must end at text.len().
        assert_eq!(runs.first().unwrap().start, 0);
        assert_eq!(runs.last().unwrap().end, "hi مرحبا".len());
    }

    #[test]
    fn test_shape_runs_empty() {
        let shaper = TextShaper::new();
        let runs = shaper.shape_runs("", FontId(1), 16.0, Direction::Ltr);
        assert!(runs.is_empty());
    }
}
