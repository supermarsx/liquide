//! # liquide-layout
//!
//! CSS box model layout engine for the LiquiDE rendering pipeline.
//!
//! Implements block, inline, flexbox, grid, and positioned layout.

pub mod block;
pub mod engine;
pub mod flex;
pub mod geometry;
pub mod grid;
pub mod inline;
pub mod intrinsic;
pub mod positioned;
pub mod tree;

pub use engine::LayoutEngine;
pub use geometry::{Point, Rect, Size};
pub use tree::{BoxType, LayoutBox, LayoutBoxId, LayoutTree, LineBox};

/// Text measurement hook — implemented by the text engine.
pub trait TextMeasurer {
    /// Measure a text run given style parameters and optional max width.
    fn measure(
        &self,
        text: &str,
        font_size: f32,
        font_family: &[String],
        font_weight: u16,
        max_width: Option<f32>,
    ) -> TextMetrics;

    /// Get the line height for a given style.
    fn line_height(&self, font_size: f32, font_family: &[String]) -> f32;

    /// Get the baseline offset for a given style.
    fn baseline(&self, font_size: f32, font_family: &[String]) -> f32;
}

/// Result of text measurement.
#[derive(Debug, Clone)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub line_count: u32,
}

/// Image measurement hook — implemented by the asset manager.
pub trait ImageMeasurer {
    /// Get intrinsic (natural) size of an image.
    fn intrinsic_size(&self, src: &str) -> Option<Size>;
}

/// Fallback text measurer that estimates sizes.
pub struct DefaultTextMeasurer;

impl TextMeasurer for DefaultTextMeasurer {
    fn measure(
        &self,
        text: &str,
        font_size: f32,
        _font_family: &[String],
        _font_weight: u16,
        max_width: Option<f32>,
    ) -> TextMetrics {
        let char_width = font_size * 0.6;
        let total_width = text.len() as f32 * char_width;
        let line_h = font_size * 1.2;

        if let Some(max_w) = max_width {
            if total_width > max_w && max_w > 0.0 {
                let chars_per_line = (max_w / char_width).floor().max(1.0) as u32;
                let line_count = ((text.len() as u32 + chars_per_line - 1) / chars_per_line).max(1);
                return TextMetrics {
                    width: max_w.min(total_width),
                    height: line_count as f32 * line_h,
                    baseline: font_size * 0.8,
                    line_count,
                };
            }
        }

        TextMetrics {
            width: total_width,
            height: line_h,
            baseline: font_size * 0.8,
            line_count: 1,
        }
    }

    fn line_height(&self, font_size: f32, _font_family: &[String]) -> f32 {
        font_size * 1.2
    }

    fn baseline(&self, font_size: f32, _font_family: &[String]) -> f32 {
        font_size * 0.8
    }
}

/// Fallback image measurer (no intrinsic sizes known).
pub struct DefaultImageMeasurer;

impl ImageMeasurer for DefaultImageMeasurer {
    fn intrinsic_size(&self, _src: &str) -> Option<Size> {
        None
    }
}
