//! Font-backed text measurer for the layout engine.
//!
//! Uses real glyph metrics from `liquide-font-rasterizer` instead of the
//! `DefaultTextMeasurer` fallback that estimates `char_width = font_size * 0.6`.

use std::sync::{Arc, Mutex};

use liquide_font_rasterizer::database::FontDatabase;
use liquide_font_rasterizer::metrics::FontMetricsProvider;
use liquide_layout::{TextMeasurer, TextMetrics, TextProperties};
use liquide_style_engine::computed::WhiteSpace;

/// Text measurer backed by a real `FontDatabase` with loaded TTF/OTF font faces.
///
/// Falls back to approximate metrics when a requested font family is not loaded.
pub struct FontTextMeasurer {
    font_db: Arc<Mutex<FontDatabase>>,
}

impl FontTextMeasurer {
    /// Create a new measurer backed by the given font database.
    pub fn new(font_db: Arc<Mutex<FontDatabase>>) -> Self {
        Self { font_db }
    }
}

impl TextMeasurer for FontTextMeasurer {
    fn measure(
        &self,
        text: &str,
        font_size: f32,
        font_family: &[String],
        font_weight: u16,
        max_width: Option<f32>,
        props: &TextProperties,
    ) -> TextMetrics {
        let db = self.font_db.lock().unwrap();
        let provider = FontMetricsProvider::new(&db);

        // Try to resolve a font face from the requested family list.
        let face_id = font_family
            .iter()
            .find_map(|family| db.resolve(family, font_weight, false))
            .or_else(|| db.resolve("sans-serif", font_weight, false));

        // Measure text width using real glyph advances when possible.
        let transformed = props.transform_text(text);
        let (text_width, _text_height) = if let Some(fid) = face_id {
            let (w, _h) = provider.measure_text(fid, font_size, &transformed);
            // Apply letter-spacing and word-spacing adjustments.
            let extra_spacing = transformed.chars().count() as f32 * props.letter_spacing
                + transformed.chars().filter(|c| *c == ' ').count() as f32 * props.word_spacing;
            (w + extra_spacing, _h)
        } else {
            // Fallback: approximate with avg char width.
            let metrics = liquide_font_rasterizer::metrics::RealFontMetrics::approximate(font_size);
            let char_width = metrics.avg_char_width + props.letter_spacing;
            let space_extra = props.word_spacing;
            let w: f32 = transformed
                .chars()
                .map(|ch| if ch == ' ' { char_width + space_extra } else { char_width })
                .sum();
            (w, metrics.line_height)
        };

        // Get real font metrics for line height and baseline.
        let real_metrics = if let Some(fid) = face_id {
            provider.metrics(fid, font_size)
        } else {
            liquide_font_rasterizer::metrics::RealFontMetrics::approximate(font_size)
        };

        let line_h = props.line_height_px(font_size);
        let baseline = real_metrics.ascent;

        if let Some(max_w) = max_width {
            let allows_wrap = matches!(
                props.white_space,
                WhiteSpace::Normal | WhiteSpace::PreWrap | WhiteSpace::PreLine
            );
            let effective_first_line = max_w - props.text_indent;
            if allows_wrap && text_width > effective_first_line && effective_first_line > 0.0 {
                // Estimate wrapped line count from measured width.
                let char_count = transformed.chars().count().max(1) as f32;
                let chars_per_line_approx =
                    (effective_first_line / (text_width / char_count))
                        .floor()
                        .max(1.0) as u32;
                let char_count_u32 = char_count as u32;
                let line_count = ((char_count_u32 + chars_per_line_approx - 1)
                    / chars_per_line_approx)
                    .max(1);
                return TextMetrics {
                    width: max_w.min(text_width),
                    height: line_count as f32 * line_h,
                    baseline,
                    line_count,
                };
            }
        }

        TextMetrics {
            width: text_width,
            height: line_h,
            baseline,
            line_count: 1,
        }
    }

    fn line_height(&self, font_size: f32, font_family: &[String]) -> f32 {
        let db = self.font_db.lock().unwrap();
        let provider = FontMetricsProvider::new(&db);

        let face_id = font_family
            .iter()
            .find_map(|family| db.resolve(family, 400, false));

        if let Some(fid) = face_id {
            let m = provider.metrics(fid, font_size);
            m.line_height
        } else {
            font_size * 1.2
        }
    }

    fn baseline(&self, font_size: f32, font_family: &[String]) -> f32 {
        let db = self.font_db.lock().unwrap();
        let provider = FontMetricsProvider::new(&db);

        let face_id = font_family
            .iter()
            .find_map(|family| db.resolve(family, 400, false));

        if let Some(fid) = face_id {
            let m = provider.metrics(fid, font_size);
            m.ascent
        } else {
            font_size * 0.8
        }
    }
}
