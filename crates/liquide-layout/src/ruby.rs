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

use crate::{Rect, Size};

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
            RubyAlign::Center | RubyAlign::SpaceAround => {
                (advance - input.annotation_width) / 2.0
            }
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
