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

use liquide_style_engine::computed::{
    FontStyle, LineHeight, TextAlign, TextOverflow, TextTransform, WhiteSpace, WordBreak,
};

/// Bundled text style properties for measurement and layout.
#[derive(Debug, Clone)]
pub struct TextProperties {
    pub font_style: FontStyle,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub line_height: LineHeight,
    pub text_align: TextAlign,
    pub text_transform: TextTransform,
    pub text_overflow: TextOverflow,
    pub white_space: WhiteSpace,
    pub word_break: WordBreak,
    pub text_indent: f32,
}

impl Default for TextProperties {
    fn default() -> Self {
        Self {
            font_style: FontStyle::Normal,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: LineHeight::Normal,
            text_align: TextAlign::Start,
            text_transform: TextTransform::None,
            text_overflow: TextOverflow::Clip,
            white_space: WhiteSpace::Normal,
            word_break: WordBreak::Normal,
            text_indent: 0.0,
        }
    }
}

impl TextProperties {
    /// Build from a ComputedStyle.
    pub fn from_style(style: &liquide_style_engine::computed::ComputedStyle) -> Self {
        Self {
            font_style: style.font_style.clone(),
            letter_spacing: style.letter_spacing,
            word_spacing: style.word_spacing,
            line_height: style.line_height.clone(),
            text_align: style.text_align,
            text_transform: style.text_transform,
            text_overflow: style.text_overflow,
            white_space: style.white_space,
            word_break: style.word_break,
            text_indent: style.text_indent,
        }
    }

    /// Resolve line height to pixels given a font size.
    pub fn line_height_px(&self, font_size: f32) -> f32 {
        match &self.line_height {
            LineHeight::Px(px) => *px,
            LineHeight::Number(n) => n * font_size,
            LineHeight::Normal => font_size * 1.2,
        }
    }

    /// Apply text-transform to a string.
    pub fn transform_text<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        match self.text_transform {
            TextTransform::Uppercase => std::borrow::Cow::Owned(text.to_uppercase()),
            TextTransform::Lowercase => std::borrow::Cow::Owned(text.to_lowercase()),
            TextTransform::Capitalize => {
                let mut result = String::with_capacity(text.len());
                let mut capitalize_next = true;
                for ch in text.chars() {
                    if ch.is_whitespace() {
                        capitalize_next = true;
                        result.push(ch);
                    } else if capitalize_next {
                        result.extend(ch.to_uppercase());
                        capitalize_next = false;
                    } else {
                        result.push(ch);
                    }
                }
                std::borrow::Cow::Owned(result)
            }
            TextTransform::None => std::borrow::Cow::Borrowed(text),
        }
    }
}

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
        props: &TextProperties,
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
        props: &TextProperties,
    ) -> TextMetrics {
        let char_width = font_size * 0.6 + props.letter_spacing;
        let space_extra = props.word_spacing;
        let transformed = props.transform_text(text);
        let total_width: f32 = transformed
            .chars()
            .map(|ch| if ch == ' ' { char_width + space_extra } else { char_width })
            .sum();
        let line_h = props.line_height_px(font_size);

        if let Some(max_w) = max_width {
            let effective_first_line = max_w - props.text_indent;
            if total_width > effective_first_line && effective_first_line > 0.0 {
                let chars_per_line = (effective_first_line / char_width).floor().max(1.0) as u32;
                let line_count =
                    ((transformed.len() as u32 + chars_per_line - 1) / chars_per_line).max(1);
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
