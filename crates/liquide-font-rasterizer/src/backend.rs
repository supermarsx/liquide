//! Bridge between `liquide-text-engine::ShaperBackend` and the real
//! rustybuzz-based `TextShaper` in this crate.
//!
//! This module provides `RustybuzzShaperBackend`, which implements the
//! `ShaperBackend` trait from `liquide-text-engine`, allowing the text
//! engine's paragraph layout to use real OpenType shaping.

use std::sync::Arc;

use liquide_text_engine::bidi::Direction;
use liquide_text_engine::font_fallback::FontId;
use liquide_text_engine::shaping::{ShaperBackend, ShaperConfig, ShapedGlyph, ShapingFeature};

use crate::database::{FontDatabase, FontFaceId};
use crate::glyph_cache::GlyphCache;

/// Maps a `FontId` (text-engine) → `FontFaceId` (font-rasterizer).
///
/// In production, the shell maintains this mapping. For now, we use
/// a direct numeric conversion.
fn font_id_to_face_id(id: FontId) -> FontFaceId {
    FontFaceId(id.0 as u32)
}

/// Shaper backend that delegates to rustybuzz via the font-rasterizer's
/// `TextShaper`.
pub struct RustybuzzShaperBackend {
    db: Arc<FontDatabase>,
    _cache: Arc<GlyphCache>,
}

impl RustybuzzShaperBackend {
    /// Create a new backend wrapping the given font database.
    #[must_use]
    pub fn new(db: Arc<FontDatabase>, cache: Arc<GlyphCache>) -> Self {
        Self { db, _cache: cache }
    }
}

impl ShaperBackend for RustybuzzShaperBackend {
    fn shape(
        &self,
        text: &str,
        font_id: FontId,
        size: f32,
        direction: Direction,
        config: &ShaperConfig,
    ) -> Option<Vec<ShapedGlyph>> {
        let face_id = font_id_to_face_id(font_id);
        let face = self.db.get(face_id)?;

        let rb_face = rustybuzz::Face::from_slice(&face.raw_data, 0)?;

        let upem = rb_face.units_per_em() as f32;
        let scale = size / upem;

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);

        if direction == Direction::Rtl {
            buffer.set_direction(rustybuzz::Direction::RightToLeft);
        }

        // Map ShaperConfig features to rustybuzz features
        let mut features = Vec::new();
        for feat in &config.features {
            let (tag, value) = match feat {
                ShapingFeature::Ligatures => (*b"liga", 1u32),
                ShapingFeature::ContextualAlternates => (*b"calt", 1),
                ShapingFeature::Kerning => (*b"kern", 1),
                ShapingFeature::SmallCaps => (*b"smcp", 1),
                ShapingFeature::OldstyleFigures => (*b"onum", 1),
                ShapingFeature::TabularFigures => (*b"tnum", 1),
                ShapingFeature::Fractions => (*b"frac", 1),
                ShapingFeature::Ordinals => (*b"ordn", 1),
                ShapingFeature::StylisticSet(n) => {
                    let n = n.clamp(&1, &20);
                    ([b's', b's', b'0' + (n / 10), b'0' + (n % 10)], 1)
                }
            };
            features.push(rustybuzz::Feature::new(
                rustybuzz::ttf_parser::Tag::from_bytes_lossy(&tag),
                value,
                ..,
            ));
        }

        let glyph_buffer = rustybuzz::shape(&rb_face, &features, buffer);
        let infos = glyph_buffer.glyph_infos();
        let positions = glyph_buffer.glyph_positions();

        let mut glyphs = Vec::with_capacity(infos.len());

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let x_advance = pos.x_advance as f32 * scale + config.letter_spacing;
            let y_advance = pos.y_advance as f32 * scale;

            // Add word spacing for space characters
            let cluster = info.cluster;
            let ch = text[cluster as usize..].chars().next().unwrap_or('\0');
            let word_extra = if ch == ' ' { config.word_spacing } else { 0.0 };

            glyphs.push(ShapedGlyph {
                glyph_id: info.glyph_id,
                cluster,
                x_advance: x_advance + word_extra,
                y_advance,
                x_offset: pos.x_offset as f32 * scale,
                y_offset: pos.y_offset as f32 * scale,
            });
        }

        Some(glyphs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_id_mapping() {
        let fid = FontId(42);
        let face_id = font_id_to_face_id(fid);
        assert_eq!(face_id.0, 42);
    }
}
