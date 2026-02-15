//! Text shaper — computes glyph positions with kerning for a text run.

use ab_glyph::{Font, GlyphId, ScaleFont};

use crate::database::{FontDatabase, FontFaceId};
use crate::metrics::RealFontMetrics;

/// A positioned glyph produced by shaping.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Character this glyph represents.
    pub codepoint: char,
    /// Glyph ID in the font.
    pub glyph_id: u32,
    /// X offset from the start of the run.
    pub x_offset: f32,
    /// Y offset from the baseline.
    pub y_offset: f32,
    /// Horizontal advance.
    pub x_advance: f32,
    /// Cluster index (byte offset in original text).
    pub cluster: u32,
}

/// Text shaper — computes glyph positions with kerning.
pub struct TextShaper<'a> {
    db: &'a FontDatabase,
}

impl<'a> TextShaper<'a> {
    /// Create a new text shaper.
    #[must_use]
    pub fn new(db: &'a FontDatabase) -> Self {
        Self { db }
    }

    /// Shape a text run, producing positioned glyphs.
    ///
    /// Returns `(glyphs, total_width)`.
    #[must_use]
    pub fn shape(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        let Some(face) = self.db.get(face_id) else {
            // Fallback: approximate shaping.
            return self.shape_fallback(text, size_px, letter_spacing);
        };

        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size_px));
        let mut glyphs = Vec::with_capacity(text.len());
        let mut pen_x = 0.0_f32;
        let mut prev_glyph: Option<GlyphId> = None;

        for (byte_idx, ch) in text.char_indices() {
            let glyph_id = face.font.glyph_id(ch);

            // Apply kerning.
            if let Some(prev) = prev_glyph {
                pen_x += scaled.kern(prev, glyph_id);
            }

            let advance = scaled.h_advance(glyph_id);

            glyphs.push(ShapedGlyph {
                codepoint: ch,
                glyph_id: glyph_id.0 as u32,
                x_offset: pen_x,
                y_offset: 0.0,
                x_advance: advance,
                cluster: byte_idx as u32,
            });

            pen_x += advance + letter_spacing;
            prev_glyph = Some(glyph_id);
        }

        (glyphs, pen_x)
    }

    /// Shape with word wrapping to a max width.
    ///
    /// Returns lines of shaped glyphs.
    #[must_use]
    pub fn shape_wrapped(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        max_width: f32,
    ) -> Vec<(Vec<ShapedGlyph>, f32)> {
        let mut lines = Vec::new();
        let mut current_line = Vec::new();
        let mut line_width = 0.0_f32;
        let _word_start = 0;
        let mut word_glyphs = Vec::new();
        let mut word_width = 0.0_f32;

        let (all_glyphs, _) = self.shape(face_id, text, size_px, letter_spacing);

        for (_i, glyph) in all_glyphs.iter().enumerate() {
            if glyph.codepoint == ' ' || glyph.codepoint == '\n' {
                // End of word — flush word to line.
                if line_width + word_width > max_width && !current_line.is_empty() {
                    lines.push((std::mem::take(&mut current_line), line_width));
                    line_width = 0.0;
                }

                // Add the word.
                for wg in word_glyphs.drain(..) {
                    let mut g: ShapedGlyph = wg;
                    g.x_offset = line_width + (g.x_offset - word_width + word_width);
                    current_line.push(g);
                }
                line_width += word_width;
                word_width = 0.0;

                if glyph.codepoint == '\n' {
                    lines.push((std::mem::take(&mut current_line), line_width));
                    line_width = 0.0;
                } else {
                    // Add the space.
                    let mut space = *glyph;
                    space.x_offset = line_width;
                    current_line.push(space);
                    line_width += glyph.x_advance + letter_spacing;
                }
            } else {
                word_glyphs.push(*glyph);
                word_width += glyph.x_advance + letter_spacing;
            }
        }

        // Flush remaining word.
        if !word_glyphs.is_empty() {
            if line_width + word_width > max_width && !current_line.is_empty() {
                lines.push((std::mem::take(&mut current_line), line_width));
                line_width = 0.0;
            }
            for wg in word_glyphs {
                current_line.push(wg);
            }
            line_width += word_width;
        }

        if !current_line.is_empty() {
            lines.push((current_line, line_width));
        }

        lines
    }

    /// Fallback shaping using approximate metrics.
    fn shape_fallback(
        &self,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        let metrics = RealFontMetrics::approximate(size_px);
        let mut glyphs = Vec::with_capacity(text.len());
        let mut pen_x = 0.0_f32;

        for (byte_idx, ch) in text.char_indices() {
            let advance = metrics.avg_char_width;
            glyphs.push(ShapedGlyph {
                codepoint: ch,
                glyph_id: ch as u32,
                x_offset: pen_x,
                y_offset: 0.0,
                x_advance: advance,
                cluster: byte_idx as u32,
            });
            pen_x += advance + letter_spacing;
        }

        (glyphs, pen_x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_fallback() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (glyphs, width) = shaper.shape(FontFaceId(999), "Hello", 14.0, 0.0);
        assert_eq!(glyphs.len(), 5);
        assert!(width > 0.0);
        // Each glyph should have increasing x_offset.
        for i in 1..glyphs.len() {
            assert!(glyphs[i].x_offset > glyphs[i - 1].x_offset);
        }
    }

    #[test]
    fn test_shape_empty() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (glyphs, width) = shaper.shape(FontFaceId(1), "", 14.0, 0.0);
        assert!(glyphs.is_empty());
        assert!((width - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_shape_letter_spacing() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (_, w0) = shaper.shape(FontFaceId(999), "AB", 14.0, 0.0);
        let (_, w2) = shaper.shape(FontFaceId(999), "AB", 14.0, 2.0);
        // With letter spacing, total width should be larger.
        assert!(w2 > w0);
    }
}
