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
}
