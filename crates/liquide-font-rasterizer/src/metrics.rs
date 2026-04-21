//! Font metrics provider — real metrics from TrueType/OpenType tables.

use ab_glyph::{Font, ScaleFont};

use crate::database::{FontDatabase, FontFaceId};

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

/// Provides real font metrics from loaded font faces.
pub struct FontMetricsProvider<'a> {
    db: &'a FontDatabase,
}

impl<'a> FontMetricsProvider<'a> {
    /// Create a new metrics provider backed by the given database.
    #[must_use]
    pub fn new(db: &'a FontDatabase) -> Self {
        Self { db }
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

    /// Measure text width using real glyph advances.
    #[must_use]
    pub fn measure_text(&self, face_id: FontFaceId, size_px: f32, text: &str) -> (f32, f32) {
        let Some(face) = self.db.get(face_id) else {
            let m = RealFontMetrics::approximate(size_px);
            return (text.chars().count() as f32 * m.avg_char_width, m.line_height);
        };

        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size_px));
        let mut width = 0.0_f32;
        let mut prev_glyph: Option<ab_glyph::GlyphId> = None;

        for ch in text.chars() {
            let glyph_id = face.font.glyph_id(ch);
            // Apply kerning.
            if let Some(prev) = prev_glyph {
                width += scaled.kern(prev, glyph_id);
            }
            width += scaled.h_advance(glyph_id);
            prev_glyph = Some(glyph_id);
        }

        let ascent = scaled.ascent();
        let descent = -scaled.descent();
        let height = ascent + descent;

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
