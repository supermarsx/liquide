//! # liquide-layout
//!
//! CSS box model layout engine for the LiquiDE rendering pipeline.
//!
//! Implements block, inline, flexbox, grid, and positioned layout.

pub mod block;
pub mod container_query;
pub mod counter;
pub mod dirty;
pub mod engine;
pub mod flex;
pub mod float;
pub mod geometry;
pub mod grid;
pub mod inline;
pub mod intrinsic;
pub mod multicol;
pub mod positioned;
pub mod replaced;
pub mod ruby;
pub mod table;
pub mod tree;
pub mod writing_mode;

pub use dirty::{LayoutDirty, LayoutDirtyCause, LayoutInvalidationSummary};
pub use engine::{LayoutEngine, LayoutInput};
pub use geometry::{ClipComplexity, Point, Rect, Size};
pub use liquide_layout_cache::{DirtyPropagation, LayoutDirtyFlags};
pub use tree::{AnchorRegistry, BoxType, LayoutBox, LayoutBoxId, LayoutTree, LineBox};

use liquide_style_engine::computed::{
    FontKerning, FontOpticalSizing, FontStretch, FontStyle, FontSynthesisSmallCaps,
    FontSynthesisStyle, FontSynthesisWeight, FontVariantAlternates, FontVariantCaps,
    FontVariantEastAsian, FontVariantEmoji, FontVariantLigatures, FontVariantNumeric,
    FontVariantPosition, Hyphens, LineBreak, LineHeight, OverflowWrap, TextAlign, TextAlignLast,
    TextBoxTrim, TextCombineUpright, TextJustify, TextOrientation, TextOverflow, TextRendering,
    TextTransform, TextWrapMode, TextWrapStyle, WhiteSpace, WhiteSpaceCollapse, WordBreak,
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
    pub overflow_wrap: OverflowWrap,
    pub hyphens: Hyphens,
    pub tab_size: f32,
    pub text_wrap_mode: TextWrapMode,
    // ── Font extras ──
    pub font_kerning: FontKerning,
    pub font_stretch: FontStretch,
    pub font_optical_sizing: FontOpticalSizing,
    pub font_variant_caps: FontVariantCaps,
    pub font_variant_numeric: FontVariantNumeric,
    pub font_variant_ligatures: FontVariantLigatures,
    pub font_variant_position: FontVariantPosition,
    pub font_variant_alternates: FontVariantAlternates,
    pub font_variant_east_asian: FontVariantEastAsian,
    pub font_variant_emoji: FontVariantEmoji,
    pub font_feature_settings: Option<String>,
    pub font_variation_settings: Option<String>,
    pub text_rendering: TextRendering,
    // ── Font synthesis (controls synthetic bold/italic/small-caps) ──
    pub font_synthesis_weight: FontSynthesisWeight,
    pub font_synthesis_style: FontSynthesisStyle,
    pub font_synthesis_small_caps: FontSynthesisSmallCaps,
    // ── Text extras ──
    pub text_align_last: TextAlignLast,
    pub text_justify: TextJustify,
    pub white_space_collapse: WhiteSpaceCollapse,
    pub line_break: LineBreak,
    pub hyphenate_character: Option<String>,
    // ── Text orientation & wrap ──
    pub text_orientation: TextOrientation,
    pub text_wrap_style: TextWrapStyle,
    // ── Font size adjust ──
    /// font-size-adjust: adjusts x-height ratio for fallback fonts.
    /// None = no adjustment, Some(ratio) = target x-height / font-size.
    pub font_size_adjust: Option<f32>,
    // ── Text extras (CJK, punctuation, spacing, initial-letter) ──
    pub text_combine_upright: TextCombineUpright,
    pub text_box_trim: TextBoxTrim,
    pub text_box_edge: Option<String>,
    pub text_spacing_trim: Option<String>,
    pub hanging_punctuation: Option<String>,
    pub initial_letter: Option<String>,
    pub text_autospace: Option<String>,
    pub hyphenate_limit_chars: Option<String>,
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
            overflow_wrap: OverflowWrap::Normal,
            hyphens: Hyphens::Manual,
            tab_size: 8.0,
            text_wrap_mode: TextWrapMode::Wrap,
            font_kerning: FontKerning::default(),
            font_stretch: FontStretch::default(),
            font_optical_sizing: FontOpticalSizing::default(),
            font_variant_caps: FontVariantCaps::default(),
            font_variant_numeric: FontVariantNumeric::default(),
            font_variant_ligatures: FontVariantLigatures::default(),
            font_variant_position: FontVariantPosition::default(),
            font_variant_alternates: FontVariantAlternates::default(),
            font_variant_east_asian: FontVariantEastAsian::default(),
            font_variant_emoji: FontVariantEmoji::default(),
            font_feature_settings: None,
            font_variation_settings: None,
            text_rendering: TextRendering::default(),
            font_synthesis_weight: FontSynthesisWeight::default(),
            font_synthesis_style: FontSynthesisStyle::default(),
            font_synthesis_small_caps: FontSynthesisSmallCaps::default(),
            text_align_last: TextAlignLast::default(),
            text_justify: TextJustify::default(),
            white_space_collapse: WhiteSpaceCollapse::default(),
            line_break: LineBreak::default(),
            hyphenate_character: None,
            text_orientation: TextOrientation::default(),
            text_wrap_style: TextWrapStyle::default(),
            font_size_adjust: None,
            text_combine_upright: TextCombineUpright::default(),
            text_box_trim: TextBoxTrim::default(),
            text_box_edge: None,
            text_spacing_trim: None,
            hanging_punctuation: None,
            initial_letter: None,
            text_autospace: None,
            hyphenate_limit_chars: None,
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
            overflow_wrap: style.overflow_wrap,
            hyphens: style.hyphens,
            tab_size: style.tab_size,
            text_wrap_mode: style.text_wrap_mode,
            font_kerning: style.font_kerning,
            font_stretch: style.font_stretch,
            font_optical_sizing: style.font_optical_sizing,
            font_variant_caps: style.font_variant_caps,
            font_variant_numeric: style.font_variant_numeric,
            font_variant_ligatures: style.font_variant_ligatures,
            font_variant_position: style.font_variant_position,
            font_variant_alternates: style.font_variant_alternates,
            font_variant_east_asian: style.font_variant_east_asian,
            font_variant_emoji: style.font_variant_emoji,
            font_feature_settings: style.font_feature_settings.clone(),
            font_variation_settings: style.font_variation_settings.clone(),
            text_rendering: style.text_rendering,
            font_synthesis_weight: style.font_synthesis_weight,
            font_synthesis_style: style.font_synthesis_style,
            font_synthesis_small_caps: style.font_synthesis_small_caps,
            text_align_last: style.text_align_last,
            text_justify: style.text_justify,
            white_space_collapse: style.white_space_collapse,
            line_break: style.line_break,
            hyphenate_character: style.hyphenate_character.clone(),
            text_orientation: style.text_orientation,
            text_wrap_style: style.text_wrap_style,
            font_size_adjust: match style.font_size_adjust {
                liquide_style_engine::computed::FontSizeAdjust::Number(n) => Some(n),
                _ => None,
            },
            text_combine_upright: style.text_combine_upright,
            text_box_trim: style.text_box_trim,
            text_box_edge: style.text_box_edge.clone(),
            text_spacing_trim: style.text_spacing_trim.clone(),
            hanging_punctuation: style.hanging_punctuation.clone(),
            initial_letter: style.initial_letter.clone(),
            text_autospace: style.text_autospace.clone(),
            hyphenate_limit_chars: style.hyphenate_limit_chars.clone(),
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

    /// Parse `font_feature_settings` CSS string into `FontFeature` values.
    #[must_use]
    pub fn parsed_font_features(&self) -> Vec<liquide_font_rasterizer::FontFeature> {
        match &self.font_feature_settings {
            Some(s) => liquide_font_rasterizer::parse_font_feature_settings(s),
            None => Vec::new(),
        }
    }

    /// Parse `font_variation_settings` CSS string into `rustybuzz::Variation` values.
    #[must_use]
    pub fn parsed_font_variations(&self) -> Vec<rustybuzz::Variation> {
        match &self.font_variation_settings {
            Some(s) => liquide_font_rasterizer::parse_font_variation_settings(s),
            None => Vec::new(),
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

/// Fallback text measurer that estimates sizes using character-class width
/// heuristics.
///
/// FALLBACK ONLY. The single measure==paint source of truth is the rustybuzz
/// shaped measurer (`liquide-font-rasterizer`'s `FontMetricsProvider::measure_text`
/// / the cached `TextShaper`), which the live pipeline supplies as the
/// `TextMeasurer` so layout's measured advance equals the painted shaped advance.
/// This `size * 0.6` per-class estimate is used only when no font-backed measurer
/// is available (e.g. headless layout tests, or a context with no font database);
/// it must NOT be relied on for a real layout/paint geometry decision, or
/// measure≠paint wrap/overlap drift returns.
pub struct DefaultTextMeasurer;

impl DefaultTextMeasurer {
    /// Approximate advance width for a character based on character class.
    /// Heuristic fallback only — see [`DefaultTextMeasurer`].
    fn approx_char_advance(ch: char, size: f32) -> f32 {
        let em = size * 0.6;
        let space = size * 0.25;
        match ch {
            ' ' => space,
            '\t' => space * 4.0,
            'W' | 'M' | 'm' | 'w' => em * 1.2,
            'i' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => em * 0.4,
            'f' | 'j' | 'r' | 't' => em * 0.6,
            'I' | '1' => em * 0.5,
            _ if ch.is_ascii_uppercase() => em * 0.95,
            _ if ch.is_ascii_lowercase() => em * 0.75,
            _ if ch.is_ascii_digit() => em * 0.75,
            _ if ch.is_ascii_punctuation() => em * 0.5,
            _ => em,
        }
    }
}

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
        let space_extra = props.word_spacing;
        let transformed = props.transform_text(text);
        let line_h = props.line_height_px(font_size);

        // Handle embedded newlines for pre/pre-wrap/pre-line white-space modes
        let preserves_newlines = matches!(
            props.white_space,
            WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::PreLine
        );

        if preserves_newlines && transformed.contains('\n') {
            let hard_lines: Vec<&str> = transformed.split('\n').collect();
            let mut max_line_width = 0.0f32;
            let mut total_lines: u32 = 0;

            for (i, line_text) in hard_lines.iter().enumerate() {
                let line_width: f32 = line_text
                    .chars()
                    .map(|ch| {
                        let base = Self::approx_char_advance(ch, font_size);
                        base + props.letter_spacing + if ch == ' ' { space_extra } else { 0.0 }
                    })
                    .sum();

                // Apply text-indent to first line only
                let indent = if i == 0 {
                    props.text_indent.max(0.0)
                } else {
                    0.0
                };

                if let Some(max_w) = max_width {
                    let allows_wrap =
                        matches!(props.white_space, WhiteSpace::PreWrap | WhiteSpace::PreLine);
                    let effective_w = if i == 0 { max_w - indent } else { max_w };
                    if allows_wrap && line_width > effective_w && effective_w > 0.0 {
                        let avg_char_w = if line_text.is_empty() {
                            font_size * 0.6 + props.letter_spacing
                        } else {
                            line_width / line_text.chars().count() as f32
                        };
                        let cpl = (effective_w / avg_char_w).floor().max(1.0) as u32;
                        let cc = line_text.chars().count() as u32;
                        let wrapped = ((cc + cpl - 1) / cpl).max(1);
                        total_lines += wrapped;
                        max_line_width = max_line_width.max(max_w.min(line_width));
                    } else {
                        total_lines += 1;
                        max_line_width = max_line_width.max(line_width + indent);
                    }
                } else {
                    total_lines += 1;
                    max_line_width = max_line_width.max(line_width + indent);
                }
            }

            return TextMetrics {
                width: if let Some(max_w) = max_width {
                    max_w.min(max_line_width)
                } else {
                    max_line_width
                },
                height: total_lines as f32 * line_h,
                baseline: font_size * 0.8,
                line_count: total_lines,
            };
        }

        // Single-line / normal wrapping path
        let total_width: f32 = transformed
            .chars()
            .map(|ch| {
                let base = Self::approx_char_advance(ch, font_size);
                base + props.letter_spacing + if ch == ' ' { space_extra } else { 0.0 }
            })
            .sum();
        let avg_char_w = if transformed.is_empty() {
            font_size * 0.6 + props.letter_spacing
        } else {
            total_width / transformed.chars().count() as f32
        };

        if let Some(max_w) = max_width {
            // white-space: nowrap / pre — do NOT wrap
            let allows_wrap = matches!(
                props.white_space,
                WhiteSpace::Normal | WhiteSpace::PreWrap | WhiteSpace::PreLine
            );
            let effective_first_line = max_w - props.text_indent.max(0.0);
            if allows_wrap && total_width > effective_first_line && effective_first_line > 0.0 {
                // Use first-line width for line 1, full width for subsequent lines
                let cpl_first = (effective_first_line / avg_char_w).floor().max(1.0) as u32;
                let cpl_rest = (max_w / avg_char_w).floor().max(1.0) as u32;
                let char_count = transformed.chars().count() as u32;
                let line_count = if char_count <= cpl_first {
                    1
                } else {
                    let remaining = char_count - cpl_first;
                    1 + ((remaining + cpl_rest - 1) / cpl_rest)
                };
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
