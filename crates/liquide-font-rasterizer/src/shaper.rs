//! Text shaper — computes glyph positions using OpenType shaping (GSUB/GPOS).
//!
//! Uses rustybuzz (a pure-Rust port of HarfBuzz) for production-quality
//! text shaping, including ligatures, kerning, and complex script support.

use crate::database::{FontDatabase, FontFaceId};

/// A positioned glyph produced by shaping.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Character this glyph represents (first char in cluster).
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

/// Text shaper — computes glyph positions with full OpenType shaping.
pub struct TextShaper<'a> {
    db: &'a FontDatabase,
}

impl<'a> TextShaper<'a> {
    #[must_use]
    pub fn new(db: &'a FontDatabase) -> Self {
        Self { db }
    }

    /// Shape a text run using OpenType shaping, producing positioned glyphs.
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
            return self.shape_fallback(text, size_px, letter_spacing);
        };

        // Try OpenType shaping via rustybuzz
        if let Some(rb_face) = rustybuzz::Face::from_slice(&face.raw_data, 0) {
            return self.shape_with_rustybuzz(&rb_face, text, size_px, letter_spacing);
        }

        // Fallback to ab_glyph kerning-only shaping
        self.shape_with_ab_glyph(face, text, size_px, letter_spacing)
    }

    /// Shape using rustybuzz (full OpenType GSUB/GPOS).
    fn shape_with_rustybuzz(
        &self,
        face: &rustybuzz::Face<'_>,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        let upem = face.units_per_em() as f32;
        let scale = size_px / upem;

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);

        let glyph_buffer = rustybuzz::shape(face, &[], buffer);
        let infos = glyph_buffer.glyph_infos();
        let positions = glyph_buffer.glyph_positions();

        let mut glyphs = Vec::with_capacity(infos.len());
        let mut pen_x = 0.0_f32;

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let cluster = info.cluster;
            // Find the character at this cluster position
            let ch = text[cluster as usize..].chars().next().unwrap_or('\0');

            let x_offset = pen_x + pos.x_offset as f32 * scale;
            let y_offset = pos.y_offset as f32 * scale;
            let x_advance = pos.x_advance as f32 * scale;

            glyphs.push(ShapedGlyph {
                codepoint: ch,
                glyph_id: info.glyph_id,
                x_offset,
                y_offset,
                x_advance,
                cluster,
            });

            pen_x += x_advance + letter_spacing;
        }

        (glyphs, pen_x)
    }

    /// Fallback shaping using ab_glyph (kerning only, no GSUB/GPOS).
    fn shape_with_ab_glyph(
        &self,
        face: &crate::database::LoadedFace,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        use ab_glyph::{Font, ScaleFont};

        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size_px));
        let mut glyphs = Vec::with_capacity(text.len());
        let mut pen_x = 0.0_f32;
        let mut prev_glyph: Option<ab_glyph::GlyphId> = None;

        for (byte_idx, ch) in text.char_indices() {
            let glyph_id = face.font.glyph_id(ch);

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
    #[must_use]
    pub fn shape_wrapped(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        max_width: f32,
    ) -> Vec<(Vec<ShapedGlyph>, f32)> {
        // Shape the full text first
        let (all_glyphs, _) = self.shape(face_id, text, size_px, letter_spacing);

        let mut lines: Vec<(Vec<ShapedGlyph>, f32)> = Vec::new();
        let mut current_line: Vec<ShapedGlyph> = Vec::new();
        let mut line_start_x = 0.0_f32;
        let mut last_break_idx: Option<usize> = None;
        let mut _last_break_x = 0.0_f32;

        for (i, glyph) in all_glyphs.iter().enumerate() {
            let glyph_end = glyph.x_offset + glyph.x_advance - line_start_x;

            if glyph.codepoint == '\n' {
                // Hard line break
                let line_width = if current_line.is_empty() {
                    0.0
                } else {
                    current_line
                        .last()
                        .map(|g| g.x_offset + g.x_advance - line_start_x)
                        .unwrap_or(0.0)
                };
                lines.push((std::mem::take(&mut current_line), line_width));
                line_start_x = glyph.x_offset + glyph.x_advance;
                last_break_idx = None;
                continue;
            }

            if glyph.codepoint == ' ' {
                last_break_idx = Some(i);
                _last_break_x = glyph.x_offset;
            }

            if glyph_end > max_width && !current_line.is_empty() {
                // Need to wrap
                if let Some(break_idx) = last_break_idx {
                    // Wrap at last space
                    let keep = break_idx - (i - current_line.len());
                    let wrapped: Vec<ShapedGlyph> = current_line.drain(keep..).collect();
                    let line_width = if current_line.is_empty() {
                        0.0
                    } else {
                        current_line
                            .last()
                            .map(|g| g.x_offset + g.x_advance - line_start_x)
                            .unwrap_or(0.0)
                    };
                    lines.push((std::mem::take(&mut current_line), line_width));

                    // Skip the space at the break point and rebase positions
                    if !wrapped.is_empty() {
                        line_start_x = wrapped[0].x_offset;
                        current_line = wrapped.into_iter().skip(1).collect();
                    }
                    last_break_idx = None;
                } else {
                    // No break point — force break at current position
                    let line_width = if current_line.is_empty() {
                        0.0
                    } else {
                        current_line
                            .last()
                            .map(|g| g.x_offset + g.x_advance - line_start_x)
                            .unwrap_or(0.0)
                    };
                    lines.push((std::mem::take(&mut current_line), line_width));
                    line_start_x = glyph.x_offset;
                }
            }

            current_line.push(*glyph);
        }

        // Flush remaining
        if !current_line.is_empty() {
            let line_width = current_line
                .last()
                .map(|g| g.x_offset + g.x_advance - line_start_x)
                .unwrap_or(0.0);
            lines.push((current_line, line_width));
        }

        if lines.is_empty() {
            lines.push((Vec::new(), 0.0));
        }

        lines
    }

    /// Fallback shaping when no font is available — approximate metrics.
    fn shape_fallback(
        &self,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        let avg_width = size_px * 0.55;
        let mut glyphs = Vec::with_capacity(text.len());
        let mut pen_x = 0.0_f32;

        for (byte_idx, ch) in text.char_indices() {
            let advance = avg_width;
            glyphs.push(ShapedGlyph {
                codepoint: ch,
                glyph_id: 0,
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
    use crate::database::FontDatabase;

    #[test]
    fn test_shape_fallback() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (glyphs, width) = shaper.shape(FontFaceId(999), "Hello", 16.0, 0.0);
        assert_eq!(glyphs.len(), 5);
        assert!(width > 0.0);
    }

    #[test]
    fn test_shape_with_letter_spacing() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (_, width_no_spacing) = shaper.shape(FontFaceId(999), "AB", 16.0, 0.0);
        let (_, width_with_spacing) = shaper.shape(FontFaceId(999), "AB", 16.0, 5.0);
        assert!(width_with_spacing > width_no_spacing);
    }

    #[test]
    fn test_shape_wrapped_basic() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let lines = shaper.shape_wrapped(FontFaceId(999), "Short", 16.0, 0.0, 1000.0);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_empty_text() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (glyphs, width) = shaper.shape(FontFaceId(999), "", 16.0, 0.0);
        assert!(glyphs.is_empty());
        assert!((width - 0.0).abs() < f32::EPSILON);
    }
}
