//! Text measurement abstraction.

use unicode_segmentation::UnicodeSegmentation;

/// Font metrics for text layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
    pub avg_char_width: f32,
}

impl FontMetrics {
    /// Approximate metrics for a given font size.
    /// These are rough estimates used when no real font rasterizer is available.
    pub fn approximate(font_size: f32) -> Self {
        Self {
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
            line_height: font_size * 1.2,
            avg_char_width: font_size * 0.55,
        }
    }
}

/// Trait for measuring text dimensions.
pub trait TextMeasure {
    /// Measure the width and height of a text string at the given font size.
    fn measure_text(&self, text: &str, font_size: f32, bold: bool) -> (f32, f32);

    /// Get font metrics for a given font size.
    fn font_metrics(&self, font_size: f32) -> FontMetrics;
}

/// A simple text measurer that uses approximate character widths.
/// Used as a fallback when no real font rasterizer is available.
#[derive(Debug, Clone)]
pub struct SimpleTextMeasure;

impl TextMeasure for SimpleTextMeasure {
    fn measure_text(&self, text: &str, font_size: f32, bold: bool) -> (f32, f32) {
        let metrics = FontMetrics::approximate(font_size);
        let width_factor = if bold { 1.1 } else { 1.0 };
        let width = UnicodeSegmentation::graphemes(text, true).count() as f32
            * metrics.avg_char_width
            * width_factor;
        (width, metrics.line_height)
    }

    fn font_metrics(&self, font_size: f32) -> FontMetrics {
        FontMetrics::approximate(font_size)
    }
}

#[cfg(test)]
mod tests {
    use super::{SimpleTextMeasure, TextMeasure};

    #[test]
    fn simple_text_measure_counts_graphemes_not_bytes() {
        let measurer = SimpleTextMeasure;
        let (precomposed, _) = measurer.measure_text("é", 14.0, false);
        let (combining, _) = measurer.measure_text("e\u{0301}", 14.0, false);
        assert!((precomposed - combining).abs() < f32::EPSILON);
    }
}
