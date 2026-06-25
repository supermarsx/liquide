//! Font metrics provider — real metrics from TrueType/OpenType tables.

use std::cell::RefCell;
use std::collections::HashMap;

use ab_glyph::{Font, ScaleFont};

use crate::database::{FontDatabase, FontFaceId};
use crate::shaper::{FontFeature, TextShaper};

/// Real font metrics extracted from font tables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealFontMetrics {
    /// Font size in pixels.
    pub size: f32,
    /// Ascent from baseline (positive, upward).
    pub ascent: f32,
    /// Descent from baseline (positive, downward).
    pub descent: f32,
    /// Line gap (extra spacing between lines).
    pub line_gap: f32,
    /// Total line height (ascent + descent + line_gap).
    pub line_height: f32,
    /// Average character width.
    pub avg_char_width: f32,
    /// Approximate x-height (height of lowercase 'x').
    pub x_height: f32,
    /// Approximate cap height (height of uppercase 'H').
    pub cap_height: f32,
    /// Underline position below baseline.
    pub underline_offset: f32,
    /// Underline thickness.
    pub underline_thickness: f32,
    /// Strikethrough position above baseline.
    pub strikethrough_offset: f32,
    /// Strikethrough thickness.
    pub strikethrough_thickness: f32,
    /// Units per em.
    pub units_per_em: f32,
}

impl RealFontMetrics {
    /// Create approximate metrics for a given size (fallback when no real font).
    #[must_use]
    pub fn approximate(size: f32) -> Self {
        Self {
            size,
            ascent: size * 0.8,
            descent: size * 0.2,
            line_gap: size * 0.0,
            line_height: size * 1.2,
            avg_char_width: size * 0.55,
            x_height: size * 0.5,
            cap_height: size * 0.7,
            underline_offset: size * 0.15,
            underline_thickness: size * 0.07,
            strikethrough_offset: size * 0.3,
            strikethrough_thickness: size * 0.07,
            units_per_em: 1000.0,
        }
    }

    /// Resolve a CSS `ex` unit value to pixels using real x-height.
    ///
    /// The `ex` unit is defined as the x-height of the font. If the real
    /// x-height is zero (missing glyph data), falls back to `font_size * 0.5`.
    #[must_use]
    pub fn ex_size(&self, value: f32) -> f32 {
        let x_h = if self.x_height > 0.0 {
            self.x_height
        } else {
            self.size * 0.5
        };
        x_h * value
    }

    /// Resolve a CSS `ch` unit value to pixels using real average character width.
    ///
    /// The `ch` unit is defined as the advance width of the '0' glyph. We use
    /// `avg_char_width` as the best available approximation from font tables.
    /// Falls back to `font_size * 0.5` when the measured width is zero.
    #[must_use]
    pub fn ch_size(&self, value: f32) -> f32 {
        let ch_w = if self.avg_char_width > 0.0 {
            self.avg_char_width
        } else {
            self.size * 0.5
        };
        ch_w * value
    }
}

/// The OpenType features applied when measuring text width.
///
/// These MUST match the features the live paint path enables
/// (`renderer-cpu/renderer/text_shaping.rs::default_features`): standard
/// kerning + standard ligatures + contextual alternates — the CSS defaults for
/// `normal` text. Measuring with the same features is what makes the measured
/// width equal the shaped painted width, so layout and paint agree and text no
/// longer wraps/overlaps where it fits when painted.
fn measure_features() -> [FontFeature; 3] {
    [
        FontFeature::kerning(true),
        FontFeature::ligatures(true),
        FontFeature::contextual_alternates(true),
    ]
}

/// Cache key for a measured advance width: (face, quantized size, text).
///
/// The size is quantized to 1/64 px so float jitter in size_px does not blow up
/// the cache while still distinguishing genuinely different sizes.
type WidthKey = (FontFaceId, u32, String);

/// Provides real font metrics from loaded font faces.
pub struct FontMetricsProvider<'a> {
    db: &'a FontDatabase,
    /// Per-`(face, size, text)` shaped advance-width cache. Shaping a run with
    /// rustybuzz is more expensive than the old kerning-only loop, and layout
    /// re-measures the same strings repeatedly, so we memoize the shaped width.
    /// `RefCell` keeps `measure_text` on `&self` (the layout trait measures
    /// through a shared reference).
    width_cache: RefCell<HashMap<WidthKey, f32>>,
}

impl<'a> FontMetricsProvider<'a> {
    /// Create a new metrics provider backed by the given database.
    #[must_use]
    pub fn new(db: &'a FontDatabase) -> Self {
        Self {
            db,
            width_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Get metrics for a font face at the given pixel size.
    #[must_use]
    pub fn metrics(&self, face_id: FontFaceId, size_px: f32) -> RealFontMetrics {
        let Some(face) = self.db.get(face_id) else {
            return RealFontMetrics::approximate(size_px);
        };

        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size_px));
        let ascent = scaled.ascent();
        let descent = -scaled.descent(); // ab_glyph returns negative descent
        let line_gap = scaled.line_gap();
        let line_height = ascent + descent + line_gap;

        // Measure average character width from a sample of common chars.
        let sample = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut total_w = 0.0_f32;
        let mut count = 0;
        for ch in sample.chars() {
            if let Some(glyph_id) = face.font.glyph_id(ch).into() {
                let advance = scaled.h_advance(glyph_id);
                total_w += advance;
                count += 1;
            }
        }
        let avg_char_width = if count > 0 {
            total_w / count as f32
        } else {
            size_px * 0.55
        };

        // Approximate x-height and cap-height from specific glyphs.
        let x_height = self
            .glyph_height(&scaled, &face.font, 'x')
            .unwrap_or(size_px * 0.5);
        let cap_height = self
            .glyph_height(&scaled, &face.font, 'H')
            .unwrap_or(size_px * 0.7);

        RealFontMetrics {
            size: size_px,
            ascent,
            descent,
            line_gap,
            line_height,
            avg_char_width,
            x_height,
            cap_height,
            underline_offset: descent * 0.5,
            underline_thickness: (size_px * 0.07).max(1.0),
            strikethrough_offset: x_height * 0.5,
            strikethrough_thickness: (size_px * 0.07).max(1.0),
            units_per_em: face.font.units_per_em().unwrap_or(1000.0) as f32,
        }
    }

    /// Measure text width using the SAME OpenType shaping as paint.
    ///
    /// The width is produced by [`TextShaper`] (rustybuzz GSUB/GPOS — kerning,
    /// ligatures, contextual alternates) with the exact feature set the live
    /// paint path enables, so a string's measured advance width equals its
    /// shaped painted width. This is the core of measure==paint: previously this
    /// did only ab_glyph pair-kerning (no ligatures/contextual shaping), so the
    /// measured width underestimated the painted width and text wrapped /
    /// overlapped in fixed-height boxes.
    ///
    /// The returned width is the run advance with letter-/word-spacing = 0;
    /// callers fold spacing in on top (matching paint, which applies
    /// letter-spacing per glyph and word-spacing per space). Returns
    /// `(width, height)` where height is the font's ascent + descent at `size_px`.
    #[must_use]
    pub fn measure_text(&self, face_id: FontFaceId, size_px: f32, text: &str) -> (f32, f32) {
        let Some(face) = self.db.get(face_id) else {
            let m = RealFontMetrics::approximate(size_px);
            return (
                text.chars().count() as f32 * m.avg_char_width,
                m.line_height,
            );
        };

        // Height is a cheap pure-metrics read (no shaping needed).
        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size_px));
        let height = scaled.ascent() + (-scaled.descent());

        if text.is_empty() {
            return (0.0, height);
        }

        // Quantize size to 1/64 px for a stable cache key.
        let size_key = (size_px * 64.0).round() as u32;
        let key: WidthKey = (face_id, size_key, text.to_string());
        if let Some(&w) = self.width_cache.borrow().get(&key) {
            return (w, height);
        }

        // Shape with the same features paint uses; letter-spacing 0 (callers add
        // spacing). `shape_with_features` returns the total advance width.
        let shaper = TextShaper::new(self.db);
        let (_glyphs, width) =
            shaper.shape_with_features(face_id, text, size_px, 0.0, &measure_features());

        self.width_cache.borrow_mut().insert(key, width);
        (width, height)
    }

    /// Measure the height of a specific glyph (for x-height, cap-height).
    fn glyph_height(
        &self,
        scaled: &ab_glyph::PxScaleFont<&ab_glyph::FontArc>,
        font: &ab_glyph::FontArc,
        ch: char,
    ) -> Option<f32> {
        let glyph_id = font.glyph_id(ch);
        let glyph = glyph_id.with_scale(scaled.scale());
        let outlined = font.outline_glyph(glyph)?;
        let bounds = outlined.px_bounds();
        Some(bounds.height())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaper::TextShaper;

    /// A database with the embedded fallback face registered, plus its resolved
    /// `FontFaceId`. The embedded Roboto carries real GPOS kerning + GSUB
    /// ligatures, so it exercises full shaping (not just pair-kerning).
    fn db_with_face() -> (FontDatabase, FontFaceId) {
        let mut db = FontDatabase::new();
        let registered = db.register_embedded_fallback();
        assert!(registered >= 1, "embedded fallback must register a face");
        let fid = db
            .resolve("sans-serif", 400, false)
            .or_else(|| db.resolve(crate::database::EMBEDDED_FALLBACK_FAMILY, 400, false))
            .expect("embedded fallback must resolve");
        (db, fid)
    }

    /// CORE measure==paint guarantee: the width returned by `measure_text` MUST
    /// equal the width the shaper (rustybuzz, same kern/liga/calt features paint
    /// uses) produces for the same string — so layout and paint agree and text
    /// no longer wraps/overlaps where it fits when painted.
    ///
    /// Teeth: if `measure_text` regresses to ab_glyph kerning-only (no GSUB
    /// ligatures / contextual shaping), its width diverges from the shaped width
    /// for the kerning/ligature strings below and this fails.
    #[test]
    fn measured_width_equals_shaped_paint_width() {
        let (db, fid) = db_with_face();
        let shaper = TextShaper::new(&db);
        let features = measure_features();

        // Strings chosen to trigger kerning pairs (AV, To, Wa, Ye) and standard
        // ligatures (fi, fl, ffi) — exactly where kerning-only measurement
        // underestimated the shaped width.
        let cases = [
            "AVAWaToYe",
            "office fluff, waffle, final",
            "Confirm action",
            "Are you sure you want to proceed?",
        ];
        for size in [12.0_f32, 14.0, 18.0, 24.0] {
            for text in cases {
                let (measured, _h) = FontMetricsProvider::new(&db).measure_text(fid, size, text);
                let (_g, shaped) =
                    shaper.shape_with_features(fid, text, size, 0.0, &features);
                assert!(
                    (measured - shaped).abs() <= 0.01,
                    "measure_text width must equal shaped paint width \
                     (text={text:?}, size={size}): measured={measured}, shaped={shaped}"
                );
                assert!(measured > 0.0, "non-empty text must measure > 0");
            }
        }
    }

    /// The shaped measurement must differ from naive ab_glyph pair-kerning for a
    /// ligature-bearing string — proving full shaping is actually in effect (and
    /// guarding against a silent revert to kerning-only).
    #[test]
    fn shaped_measure_differs_from_kerning_only() {
        use ab_glyph::{Font, ScaleFont};
        let (db, fid) = db_with_face();
        let face = db.get(fid).expect("face");
        let size = 16.0_f32;
        let text = "office final affluent waffle"; // many fi/fl ligatures

        // Old kerning-only path (what measure_text used to do).
        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size));
        let mut kern_only = 0.0_f32;
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for ch in text.chars() {
            let gid = face.font.glyph_id(ch);
            if let Some(p) = prev {
                kern_only += scaled.kern(p, gid);
            }
            kern_only += scaled.h_advance(gid);
            prev = Some(gid);
        }

        let (shaped, _h) = FontMetricsProvider::new(&db).measure_text(fid, size, text);
        // Ligatures collapse glyph pairs into single (often narrower) glyphs, so
        // the shaped width is not identical to the kerning-only sum. If they were
        // bit-identical, shaping is not actually engaged.
        assert!(
            (shaped - kern_only).abs() > 0.01,
            "shaped width ({shaped}) must differ from kerning-only ({kern_only}) — \
             full GSUB/GPOS shaping is not engaged"
        );
    }

    /// The width cache must not change the result (a cached re-measure equals the
    /// first measure exactly).
    #[test]
    fn width_cache_is_transparent() {
        let (db, fid) = db_with_face();
        let provider = FontMetricsProvider::new(&db);
        let (first, _) = provider.measure_text(fid, 14.0, "Confirm action");
        let (again, _) = provider.measure_text(fid, 14.0, "Confirm action");
        assert_eq!(first, again, "cached width must equal first measurement");
    }

    #[test]
    fn test_approximate_metrics() {
        let m = RealFontMetrics::approximate(16.0);
        assert!((m.ascent - 12.8).abs() < 0.1);
        assert!((m.descent - 3.2).abs() < 0.1);
        assert!((m.line_height - 19.2).abs() < 0.1);
        assert!(m.avg_char_width > 0.0);
    }

    #[test]
    fn test_provider_fallback() {
        let db = FontDatabase::new();
        let provider = FontMetricsProvider::new(&db);
        // With no fonts loaded, should return approximate metrics.
        let m = provider.metrics(FontFaceId(999), 14.0);
        assert!((m.size - 14.0).abs() < 0.001);
        assert!(m.line_height > 0.0);
    }

    #[test]
    fn test_approximate_metrics_proportions() {
        let m = RealFontMetrics::approximate(32.0);
        assert!(m.ascent > m.descent);
        assert!(m.line_height > m.ascent);
        assert!(m.cap_height > m.x_height);
    }

    #[test]
    fn test_measure_text_no_font_fallback() {
        let db = FontDatabase::new();
        let provider = FontMetricsProvider::new(&db);
        let (width, height) = provider.measure_text(FontFaceId(999), 16.0, "Hello");
        assert!(width > 0.0);
        assert!(height > 0.0);
    }

    #[test]
    fn test_measure_text_empty_string() {
        let db = FontDatabase::new();
        let provider = FontMetricsProvider::new(&db);
        let (width, _height) = provider.measure_text(FontFaceId(999), 16.0, "");
        assert!((width - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ex_size_uses_real_x_height() {
        let mut m = RealFontMetrics::approximate(16.0);
        // Override x_height with a "real" value different from the approximation.
        m.x_height = 9.0;
        // 2ex should be 2 * 9.0 = 18.0
        assert!((m.ex_size(2.0) - 18.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ex_size_fallback_when_zero() {
        let mut m = RealFontMetrics::approximate(16.0);
        m.x_height = 0.0;
        // Should fall back to font_size * 0.5 = 8.0
        assert!((m.ex_size(1.0) - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ch_size_uses_real_avg_char_width() {
        let mut m = RealFontMetrics::approximate(16.0);
        // Override avg_char_width with a "real" value.
        m.avg_char_width = 10.0;
        // 3ch should be 3 * 10.0 = 30.0
        assert!((m.ch_size(3.0) - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ch_size_fallback_when_zero() {
        let mut m = RealFontMetrics::approximate(16.0);
        m.avg_char_width = 0.0;
        // Should fall back to font_size * 0.5 = 8.0
        assert!((m.ch_size(1.0) - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_approximate_ex_and_ch_consistency() {
        let m = RealFontMetrics::approximate(20.0);
        // Approximate x_height = 20 * 0.5 = 10, so ex_size(1) = 10
        assert!((m.ex_size(1.0) - 10.0).abs() < f32::EPSILON);
        // Approximate avg_char_width = 20 * 0.55 = 11, so ch_size(1) = 11
        assert!((m.ch_size(1.0) - 11.0).abs() < f32::EPSILON);
    }
}
