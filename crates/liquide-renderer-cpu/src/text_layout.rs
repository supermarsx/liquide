//! Text layout bridge — connects `liquide-text-engine` to the CPU renderer.
//!
//! Provides a [`TextLayoutEngine`] that wraps the text-engine's paragraph
//! layouter and font-rasterizer's database to produce fully-positioned
//! glyph runs ready for atlas insertion and rendering.
//!
//! # Flow
//!
//! ```text
//! &str + FontDatabase
//!   │
//!   ▼
//! TextShaper (font-rasterizer)  ──or──  TextShaper (text-engine)
//!   │
//!   ▼
//! ParagraphLayouter (text-engine)
//!   │
//!   ▼
//! PositionedGlyph[] + LayoutLine[]
//!   │
//!   ▼
//! GlyphRasterizer (font-rasterizer)  →  GlyphAtlas (renderer)
//! ```

use std::sync::{Arc, Mutex, MutexGuard};

use liquide_font_rasterizer::database::{FontDatabase, FontFaceId};
use liquide_font_rasterizer::metrics::FontMetricsProvider;
use liquide_text_engine::paragraph::TextAlignment;

/// Bridges font-rasterizer and text-engine for complete text layout.
///
/// Provides measurement and layout APIs that widgets and the scene renderer
/// can use to position text with real font metrics.
pub struct TextLayoutEngine {
    font_db: Arc<Mutex<FontDatabase>>,
}

impl TextLayoutEngine {
    /// Create a new text layout engine backed by the given font database.
    pub fn new(font_db: Arc<Mutex<FontDatabase>>) -> Self {
        Self { font_db }
    }

    fn lock_font_db(&self) -> MutexGuard<'_, FontDatabase> {
        self.font_db.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Measure the width and height of a single-line text run.
    ///
    /// Returns `(width, height)` in pixels.
    pub fn measure_text(
        &self,
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
    ) -> (f32, f32) {
        let db = self.lock_font_db();
        if let Some(face_id) = db.resolve(font_family, font_weight, false) {
            let provider = FontMetricsProvider::new(&db);
            let (width, height) = provider.measure_text(face_id, font_size, text);
            (width, height)
        } else {
            // Fallback: approximate metrics.
            let m = liquide_font_rasterizer::metrics::RealFontMetrics::approximate(font_size);
            (text.len() as f32 * m.avg_char_width, m.line_height)
        }
    }

    /// Layout a paragraph of text with word wrapping and alignment.
    ///
    /// Returns a [`TextLayoutResult`] with positioned glyphs and line info.
    pub fn layout_paragraph(
        &self,
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        max_width: f32,
        alignment: TextAlignment,
        line_height: f32,
    ) -> TextLayoutResult {
        if text.is_empty() {
            return TextLayoutResult {
                lines: Vec::new(),
                width: 0.0,
                height: 0.0,
                font_size,
                line_height: if line_height > 0.0 {
                    font_size * line_height
                } else {
                    font_size * 1.2
                },
            };
        }

        let db = self.lock_font_db();

        let face_id = db.resolve(font_family, font_weight, false);
        let provider = FontMetricsProvider::new(&db);

        // Get real font metrics if we have a face.
        let metrics = if let Some(fid) = face_id {
            provider.metrics(fid, font_size)
        } else {
            liquide_font_rasterizer::metrics::RealFontMetrics::approximate(font_size)
        };

        // Use font-rasterizer's TextShaper for glyph positioning.
        let shaper = liquide_font_rasterizer::shaper::TextShaper::new(&db);
        let shaper_face = face_id.unwrap_or(FontFaceId::FALLBACK);

        // Compute letter spacing from the difference between line_height factor
        // and default (1.2). In practice, letter_spacing comes from theme.
        let letter_spacing = 0.0_f32;

        // Layout lines with word wrapping.
        let wrapped_lines =
            shaper.shape_wrapped(shaper_face, text, font_size, letter_spacing, max_width);

        let effective_line_height = if line_height > 0.0 {
            font_size * line_height
        } else {
            metrics.line_height
        };

        let mut result_lines = Vec::with_capacity(wrapped_lines.len());
        let mut total_height = 0.0_f32;
        let mut max_width_actual = 0.0_f32;

        let is_last_line = wrapped_lines.len();
        for (i, (glyphs, line_width)) in wrapped_lines.iter().enumerate() {
            let baseline_y = (i as f32 + 1.0) * effective_line_height;
            let is_last = i == is_last_line - 1;

            // Apply horizontal alignment offset.
            let positioned: Vec<PositionedGlyph> = match alignment {
                TextAlignment::Start => glyphs
                    .iter()
                    .map(|g| PositionedGlyph {
                        codepoint: g.codepoint,
                        glyph_id: g.glyph_id,
                        x: g.x_offset,
                        y: baseline_y,
                        advance: g.x_advance,
                    })
                    .collect(),
                TextAlignment::End => {
                    let x_offset = (max_width - line_width).max(0.0);
                    glyphs
                        .iter()
                        .map(|g| PositionedGlyph {
                            codepoint: g.codepoint,
                            glyph_id: g.glyph_id,
                            x: g.x_offset + x_offset,
                            y: baseline_y,
                            advance: g.x_advance,
                        })
                        .collect()
                }
                TextAlignment::Center => {
                    let x_offset = ((max_width - line_width) / 2.0).max(0.0);
                    glyphs
                        .iter()
                        .map(|g| PositionedGlyph {
                            codepoint: g.codepoint,
                            glyph_id: g.glyph_id,
                            x: g.x_offset + x_offset,
                            y: baseline_y,
                            advance: g.x_advance,
                        })
                        .collect()
                }
                TextAlignment::Justify => {
                    // Don't justify the last line — treat it as Start-aligned
                    if is_last || glyphs.is_empty() {
                        glyphs
                            .iter()
                            .map(|g| PositionedGlyph {
                                codepoint: g.codepoint,
                                glyph_id: g.glyph_id,
                                x: g.x_offset,
                                y: baseline_y,
                                advance: g.x_advance,
                            })
                            .collect()
                    } else {
                        // Count spaces (word break opportunities)
                        let space_count = glyphs
                            .iter()
                            .filter(|g| g.codepoint == ' ')
                            .count();
                        
                        if space_count == 0 {
                            // No spaces to expand, just use Start alignment
                            glyphs
                                .iter()
                                .map(|g| PositionedGlyph {
                                    codepoint: g.codepoint,
                                    glyph_id: g.glyph_id,
                                    x: g.x_offset,
                                    y: baseline_y,
                                    advance: g.x_advance,
                                })
                                .collect()
                        } else {
                            // Distribute extra space across word gaps
                            let extra_space = (max_width - line_width).max(0.0);
                            let space_expansion = extra_space / space_count as f32;
                            let mut accumulated_expansion = 0.0_f32;
                            
                            glyphs
                                .iter()
                                .map(|g| {
                                    let x = g.x_offset + accumulated_expansion;
                                    if g.codepoint == ' ' {
                                        accumulated_expansion += space_expansion;
                                    }
                                    PositionedGlyph {
                                        codepoint: g.codepoint,
                                        glyph_id: g.glyph_id,
                                        x,
                                        y: baseline_y,
                                        advance: if g.codepoint == ' ' {
                                            g.x_advance + space_expansion
                                        } else {
                                            g.x_advance
                                        },
                                    }
                                })
                                .collect()
                        }
                    }
                }
            };

            max_width_actual = max_width_actual.max(*line_width);
            total_height = baseline_y + metrics.descent;

            result_lines.push(TextLine {
                glyphs: positioned,
                width: *line_width,
                baseline_y,
                ascent: metrics.ascent,
                descent: metrics.descent,
            });
        }

        TextLayoutResult {
            lines: result_lines,
            width: max_width_actual,
            height: total_height,
            font_size,
            line_height: effective_line_height,
        }
    }

    /// Get the font database reference for external use.
    pub fn font_db(&self) -> &Arc<Mutex<FontDatabase>> {
        &self.font_db
    }
}

/// A positioned glyph in a layout result.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    pub codepoint: char,
    pub glyph_id: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

/// A single line in a text layout result.
#[derive(Debug, Clone)]
pub struct TextLine {
    pub glyphs: Vec<PositionedGlyph>,
    pub width: f32,
    pub baseline_y: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl TextLine {
    /// Total height of this line.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

/// Result of laying out a paragraph of text.
#[derive(Debug, Clone)]
pub struct TextLayoutResult {
    pub lines: Vec<TextLine>,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub line_height: f32,
}

impl TextLayoutResult {
    /// Total number of positioned glyphs across all lines.
    pub fn glyph_count(&self) -> usize {
        self.lines.iter().map(|l| l.glyphs.len()).sum()
    }

    /// Whether the layout is empty (no glyphs).
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_text_fallback() {
        let db = FontDatabase::new();
        let engine = TextLayoutEngine::new(Arc::new(Mutex::new(db)));
        let (w, h) = engine.measure_text("Hello", "Manrope", 14.0, 400);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_layout_paragraph_empty() {
        let db = FontDatabase::new();
        let engine = TextLayoutEngine::new(Arc::new(Mutex::new(db)));
        let result =
            engine.layout_paragraph("", "Manrope", 14.0, 400, 200.0, TextAlignment::Start, 1.4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_layout_paragraph_single_line() {
        let db = FontDatabase::new();
        let engine = TextLayoutEngine::new(Arc::new(Mutex::new(db)));
        let result = engine.layout_paragraph(
            "Hello World",
            "Manrope",
            14.0,
            400,
            500.0, // Wide enough for single line
            TextAlignment::Start,
            1.4,
        );
        assert_eq!(result.lines.len(), 1);
        assert!(result.width > 0.0);
        assert!(result.height > 0.0);
    }
}
