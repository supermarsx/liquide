//! Ruby layout — `display: ruby` and `display: ruby-text` (CSS Ruby L1).
//!
//! Ruby annotations are small helper text placed above, below, or beside
//! base text to aid pronunciation (common in CJK typography).
//!
//! ## Layout model
//!
//! A ruby container (`display: ruby`) contains:
//! - **Ruby base** (`display: ruby-base`): the annotated text.
//! - **Ruby text** (`display: ruby-text`): the annotation.
//! - **Ruby base container** / **Ruby text container**: grouping wrappers.
//!
//! The annotation is centered over its base.  If the annotation is wider than
//! the base, the base is padded symmetrically (and vice versa).
//!
//! ## This implementation
//!
//! We pair each ruby base box with its corresponding ruby text box and
//! compute offsets.  The inline layout engine calls [`layout_ruby_container`]
//! when it encounters a `display: ruby` box.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::Display;

use crate::{Rect, Size, TextMeasurer, TextProperties};

/// Position of ruby annotation relative to the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RubyPosition {
    /// Above the base (default for horizontal text).
    Over,
    /// Below the base.
    Under,
    /// Same as over but with inter-character spacing.
    InterCharacter,
}

impl Default for RubyPosition {
    fn default() -> Self {
        Self::Over
    }
}

/// Alignment of ruby text within its annotation box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RubyAlign {
    /// Distribute space at start & end.
    SpaceAround,
    /// Center the annotation.
    Center,
    /// Align start.
    Start,
    /// Distribute evenly.
    SpaceBetween,
}

impl Default for RubyAlign {
    fn default() -> Self {
        Self::SpaceAround
    }
}

/// A ruby base + annotation pair after layout.
#[derive(Debug, Clone)]
pub struct RubyPair {
    /// Rect of the base text.
    pub base_rect: Rect,
    /// Rect of the annotation text.
    pub annotation_rect: Rect,
    /// The combined advance (inline direction).
    pub advance: f32,
}

/// Input for a single ruby base–annotation pair.
#[derive(Debug, Clone, Copy)]
pub struct RubyInput {
    /// Width of the base text.
    pub base_width: f32,
    /// Height of the base text.
    pub base_height: f32,
    /// Width of the annotation text.
    pub annotation_width: f32,
    /// Height of the annotation text (font size of ruby text).
    pub annotation_height: f32,
}

/// Configuration for ruby layout.
#[derive(Debug, Clone)]
pub struct RubyConfig {
    pub position: RubyPosition,
    pub align: RubyAlign,
    /// Gap between base and annotation (default 2px).
    pub gap: f32,
    /// Starting x offset in the line.
    pub start_x: f32,
    /// Baseline y of the inline line.
    pub baseline_y: f32,
}

impl Default for RubyConfig {
    fn default() -> Self {
        Self {
            position: RubyPosition::Over,
            align: RubyAlign::SpaceAround,
            gap: 2.0,
            start_x: 0.0,
            baseline_y: 0.0,
        }
    }
}

/// Lay out a ruby container: pair bases with annotations and compute rects.
///
/// Returns paired layout results and the total inline advance.
pub fn layout_ruby_container(config: &RubyConfig, pairs: &[RubyInput]) -> (Vec<RubyPair>, f32) {
    let mut results = Vec::with_capacity(pairs.len());
    let mut x = config.start_x;

    for input in pairs {
        // The advance width is the max of base and annotation.
        let advance = input.base_width.max(input.annotation_width);

        // Center the narrower one within the advance.
        let base_offset_x = (advance - input.base_width) / 2.0;
        let ann_offset_x = match config.align {
            RubyAlign::Center | RubyAlign::SpaceAround => (advance - input.annotation_width) / 2.0,
            RubyAlign::Start => 0.0,
            RubyAlign::SpaceBetween => {
                // If annotation has multiple glyphs this would distribute,
                // but at the box level we just center.
                (advance - input.annotation_width) / 2.0
            }
        };

        // Compute y positions.
        let base_rect;
        let annotation_rect;

        match config.position {
            RubyPosition::Over => {
                // Annotation sits above the base.
                let base_y = config.baseline_y - input.base_height;
                let ann_y = base_y - config.gap - input.annotation_height;

                base_rect = Rect::new(
                    x + base_offset_x,
                    base_y,
                    input.base_width,
                    input.base_height,
                );
                annotation_rect = Rect::new(
                    x + ann_offset_x,
                    ann_y,
                    input.annotation_width,
                    input.annotation_height,
                );
            }
            RubyPosition::Under => {
                // Annotation sits below the base.
                let base_y = config.baseline_y - input.base_height;
                let ann_y = config.baseline_y + config.gap;

                base_rect = Rect::new(
                    x + base_offset_x,
                    base_y,
                    input.base_width,
                    input.base_height,
                );
                annotation_rect = Rect::new(
                    x + ann_offset_x,
                    ann_y,
                    input.annotation_width,
                    input.annotation_height,
                );
            }
            RubyPosition::InterCharacter => {
                // For vertical text: annotation to the right of the base.
                // In horizontal mode, fallback to Over.
                let base_y = config.baseline_y - input.base_height;
                let ann_y = base_y - config.gap - input.annotation_height;
                base_rect = Rect::new(
                    x + base_offset_x,
                    base_y,
                    input.base_width,
                    input.base_height,
                );
                annotation_rect = Rect::new(
                    x + ann_offset_x,
                    ann_y,
                    input.annotation_width,
                    input.annotation_height,
                );
            }
        }

        results.push(RubyPair {
            base_rect,
            annotation_rect,
            advance,
        });

        x += advance;
    }

    let total_advance = x - config.start_x;
    (results, total_advance)
}

/// Compute the total block size needed for a ruby line (base + annotation + gap).
pub fn ruby_block_size(base_height: f32, annotation_height: f32, gap: f32) -> f32 {
    base_height + annotation_height + gap
}

/// Compute the content size of a ruby container.
pub fn ruby_container_size(config: &RubyConfig, pairs: &[RubyInput]) -> Size {
    let (_, total_inline) = layout_ruby_container(config, pairs);

    let max_base_h = pairs.iter().map(|p| p.base_height).fold(0.0f32, f32::max);
    let max_ann_h = pairs
        .iter()
        .map(|p| p.annotation_height)
        .fold(0.0f32, f32::max);
    let block = ruby_block_size(max_base_h, max_ann_h, config.gap);

    Size::new(total_inline, block)
}

/// Result of laying out a ruby container for integration with inline layout.
#[derive(Debug, Clone)]
pub struct RubyContainerResult {
    /// Total inline advance (width) of the ruby container.
    pub inline_advance: f32,
    /// Height of the base text area.
    pub base_height: f32,
    /// Height of the annotation text area.
    pub annotation_height: f32,
    /// Gap between base and annotation.
    pub gap: f32,
    /// Whether the annotation is above (Over) or below (Under) the base.
    pub position: RubyPosition,
    /// The laid-out pairs.
    pub pairs: Vec<RubyPair>,
}

impl RubyContainerResult {
    /// Total block size (base + annotation + gap).
    pub fn total_block_size(&self) -> f32 {
        ruby_block_size(self.base_height, self.annotation_height, self.gap)
    }

    /// How much extra space the annotation needs above the base line.
    /// Returns 0 if the annotation is below.
    pub fn annotation_overhead(&self) -> f32 {
        match self.position {
            RubyPosition::Over | RubyPosition::InterCharacter => self.annotation_height + self.gap,
            RubyPosition::Under => 0.0,
        }
    }

    /// How much extra space the annotation needs below the base line.
    /// Returns 0 if the annotation is above.
    pub fn annotation_underhang(&self) -> f32 {
        match self.position {
            RubyPosition::Under => self.annotation_height + self.gap,
            RubyPosition::Over | RubyPosition::InterCharacter => 0.0,
        }
    }
}

/// Convert the style-engine `RubyPosition` to the layout-internal one.
fn convert_ruby_position(pos: liquide_style_engine::computed::RubyPosition) -> RubyPosition {
    match pos {
        liquide_style_engine::computed::RubyPosition::Over
        | liquide_style_engine::computed::RubyPosition::AlternateOver => RubyPosition::Over,
        liquide_style_engine::computed::RubyPosition::Under
        | liquide_style_engine::computed::RubyPosition::AlternateUnder => RubyPosition::Under,
    }
}

/// Convert the style-engine `RubyAlign` to the layout-internal one.
fn convert_ruby_align(align: liquide_style_engine::computed::RubyAlign) -> RubyAlign {
    match align {
        liquide_style_engine::computed::RubyAlign::SpaceAround => RubyAlign::SpaceAround,
        liquide_style_engine::computed::RubyAlign::Center => RubyAlign::Center,
        liquide_style_engine::computed::RubyAlign::Start => RubyAlign::Start,
        liquide_style_engine::computed::RubyAlign::SpaceBetween => RubyAlign::SpaceBetween,
    }
}

/// Lay out a `display: ruby` container by inspecting the DOM for ruby-base
/// and ruby-text children, measuring their text, and pairing them.
///
/// Children that are not `display: ruby-text` are treated as ruby bases.
/// Each base is paired with the next ruby-text sibling.  Unpaired bases
/// get an empty annotation; unpaired ruby-text nodes are ignored.
pub fn layout_ruby_from_dom(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    text_measurer: &(impl TextMeasurer + ?Sized),
    start_x: f32,
    baseline_y: f32,
) -> RubyContainerResult {
    let style = styles.get(node_id).cloned().unwrap_or_default();
    let position = convert_ruby_position(style.ruby_position);
    let align = convert_ruby_align(style.ruby_align);

    let children = doc.children(node_id).to_vec();

    // Collect base/text children.  Non-ruby-text children are bases.
    let mut bases: Vec<NodeId> = Vec::new();
    let mut texts: Vec<NodeId> = Vec::new();

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::RubyText {
            texts.push(child_id);
        } else if child_style.display != Display::None {
            bases.push(child_id);
        }
    }

    // Build RubyInput pairs by zipping bases with texts.
    let mut pairs_input: Vec<RubyInput> = Vec::new();
    let pair_count = bases.len();

    for i in 0..pair_count {
        let base_id = bases[i];
        let base_style = styles.get(base_id).cloned().unwrap_or_default();
        let base_text = collect_text_content(doc, base_id);
        let base_props = TextProperties::from_style(&base_style);
        let base_metrics = text_measurer.measure(
            &base_text,
            base_style.font_size,
            &base_style.font_family,
            base_style.font_weight,
            None,
            &base_props,
        );

        let (ann_w, ann_h) = if i < texts.len() {
            let text_id = texts[i];
            let text_style = styles.get(text_id).cloned().unwrap_or_default();
            let ann_text = collect_text_content(doc, text_id);
            let ann_props = TextProperties::from_style(&text_style);
            let ann_metrics = text_measurer.measure(
                &ann_text,
                text_style.font_size,
                &text_style.font_family,
                text_style.font_weight,
                None,
                &ann_props,
            );
            (ann_metrics.width, ann_metrics.height)
        } else {
            (0.0, 0.0)
        };

        pairs_input.push(RubyInput {
            base_width: base_metrics.width,
            base_height: base_metrics.height,
            annotation_width: ann_w,
            annotation_height: ann_h,
        });
    }

    let config = RubyConfig {
        position,
        align,
        gap: 2.0,
        start_x,
        baseline_y,
    };

    let (pairs, total_advance) = layout_ruby_container(&config, &pairs_input);

    let max_base_h = pairs_input
        .iter()
        .map(|p| p.base_height)
        .fold(0.0f32, f32::max);
    let max_ann_h = pairs_input
        .iter()
        .map(|p| p.annotation_height)
        .fold(0.0f32, f32::max);

    RubyContainerResult {
        inline_advance: total_advance,
        base_height: max_base_h,
        annotation_height: max_ann_h,
        gap: config.gap,
        position,
        pairs,
    }
}

/// Recursively collect text content from a node and its descendants.
fn collect_text_content(doc: &Document, node_id: NodeId) -> String {
    let mut result = String::new();
    if let Some(node) = doc.get(node_id) {
        if let Some(text) = node.text_content() {
            result.push_str(text);
        }
    }
    for &child_id in doc.children(node_id) {
        result.push_str(&collect_text_content(doc, child_id));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pair(base_w: f32, ann_w: f32) -> RubyInput {
        RubyInput {
            base_width: base_w,
            base_height: 16.0,
            annotation_width: ann_w,
            annotation_height: 8.0,
        }
    }

    #[test]
    fn single_pair_centered() {
        let config = RubyConfig {
            baseline_y: 20.0,
            ..Default::default()
        };
        let (pairs, advance) = layout_ruby_container(&config, &[make_pair(20.0, 30.0)]);
        assert_eq!(pairs.len(), 1);
        // Advance should be max(20, 30) = 30
        assert!((advance - 30.0).abs() < 0.01);
        // Base should be centered: offset = (30-20)/2 = 5
        assert!((pairs[0].base_rect.x - 5.0).abs() < 0.01);
        // Annotation should be centered: offset = 0
        assert!((pairs[0].annotation_rect.x - 0.0).abs() < 0.01);
    }

    #[test]
    fn multiple_pairs_advance() {
        let config = RubyConfig::default();
        let inputs = vec![make_pair(20.0, 20.0), make_pair(30.0, 10.0)];
        let (pairs, advance) = layout_ruby_container(&config, &inputs);
        assert_eq!(pairs.len(), 2);
        // Total advance = 20 + 30 = 50
        assert!((advance - 50.0).abs() < 0.01);
    }

    #[test]
    fn under_position() {
        let config = RubyConfig {
            position: RubyPosition::Under,
            baseline_y: 20.0,
            ..Default::default()
        };
        let (pairs, _) = layout_ruby_container(&config, &[make_pair(20.0, 20.0)]);
        // Annotation should be below baseline
        assert!(pairs[0].annotation_rect.y > config.baseline_y);
    }

    #[test]
    fn ruby_block_size_calculation() {
        let size = ruby_block_size(16.0, 8.0, 2.0);
        assert!((size - 26.0).abs() < 0.01);
    }

    #[test]
    fn container_size() {
        let config = RubyConfig::default();
        let inputs = vec![make_pair(20.0, 20.0), make_pair(30.0, 15.0)];
        let size = ruby_container_size(&config, &inputs);
        assert!((size.width - 50.0).abs() < 0.01);
        assert!((size.height - 26.0).abs() < 0.01); // 16 + 8 + 2
    }
}
