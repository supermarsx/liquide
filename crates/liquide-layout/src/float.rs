//! Float layout — CSS float positioning.
//!
//! Implements basic float behaviour:
//! 1. Float elements are taken out of normal flow
//! 2. Left floats stack from the left, right floats from the right
//! 3. Subsequent (non-float) content flows around the floats
//!
//! This module provides a `FloatContext` that tracks placed floats and can
//! be queried for available width at a given vertical position.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{Display, Float, Position};

use crate::geometry::Rect;
use crate::tree::{LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};
/// A placed float with its occupied rectangle.
#[derive(Debug, Clone)]
struct PlacedFloat {
    rect: Rect,
    _side: FloatSide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatSide {
    Left,
    Right,
}

/// Float context: tracks placed floats and provides exclusion queries.
///
/// Used during block layout to determine how content should flow around
/// floating elements.
pub struct FloatContext {
    left_floats: Vec<PlacedFloat>,
    right_floats: Vec<PlacedFloat>,
    container_width: f32,
}

impl FloatContext {
    /// Create a new float context for a given container width.
    pub fn new(container_width: f32) -> Self {
        Self {
            left_floats: Vec::new(),
            right_floats: Vec::new(),
            container_width,
        }
    }

    /// Place a left float at the current position.
    pub fn place_left(&mut self, width: f32, height: f32, y: f32) -> Rect {
        // Find the leftmost x that doesn't overlap existing floats at this y
        let x = self.left_edge_at(y, height);
        let rect = Rect::new(x, y, width, height);
        self.left_floats.push(PlacedFloat {
            rect,
            _side: FloatSide::Left,
        });
        rect
    }

    /// Place a right float at the current position.
    pub fn place_right(&mut self, width: f32, height: f32, y: f32) -> Rect {
        // Find the rightmost x that doesn't overlap existing floats at this y
        let right_edge = self.right_edge_at(y, height);
        let x = (right_edge - width).max(0.0);
        let rect = Rect::new(x, y, width, height);
        self.right_floats.push(PlacedFloat {
            rect,
            _side: FloatSide::Right,
        });
        rect
    }

    /// Get the available width for content at a given y position.
    pub fn available_width_at(&self, y: f32, height: f32) -> (f32, f32, f32) {
        let left = self.left_edge_at(y, height);
        let right = self.right_edge_at(y, height);
        (left, right, (right - left).max(0.0))
    }

    /// Clear floats: return the y position below all floats of the given type.
    pub fn clear_y(&self, clear: ClearSide) -> f32 {
        let mut y = 0.0f32;
        match clear {
            ClearSide::Left | ClearSide::Both => {
                for f in &self.left_floats {
                    y = y.max(f.rect.y + f.rect.height);
                }
            }
            _ => {}
        }
        match clear {
            ClearSide::Right | ClearSide::Both => {
                for f in &self.right_floats {
                    y = y.max(f.rect.y + f.rect.height);
                }
            }
            _ => {}
        }
        y
    }

    /// Get the left edge (furthest right occupied by left floats) at a given y range.
    fn left_edge_at(&self, y: f32, height: f32) -> f32 {
        let mut edge = 0.0f32;
        for f in &self.left_floats {
            if Self::ranges_overlap(y, y + height, f.rect.y, f.rect.y + f.rect.height) {
                edge = edge.max(f.rect.x + f.rect.width);
            }
        }
        edge
    }

    /// Get the right edge (furthest left occupied by right floats) at a given y range.
    fn right_edge_at(&self, y: f32, height: f32) -> f32 {
        let mut edge = self.container_width;
        for f in &self.right_floats {
            if Self::ranges_overlap(y, y + height, f.rect.y, f.rect.y + f.rect.height) {
                edge = edge.min(f.rect.x);
            }
        }
        edge
    }

    /// Check if two vertical ranges overlap.
    fn ranges_overlap(a_top: f32, a_bottom: f32, b_top: f32, b_bottom: f32) -> bool {
        a_top < b_bottom && a_bottom > b_top
    }
}

/// Which side to clear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClearSide {
    Left,
    Right,
    Both,
    None,
}

impl From<liquide_style_engine::computed::Clear> for ClearSide {
    fn from(c: liquide_style_engine::computed::Clear) -> Self {
        match c {
            liquide_style_engine::computed::Clear::Left => ClearSide::Left,
            liquide_style_engine::computed::Clear::Right => ClearSide::Right,
            liquide_style_engine::computed::Clear::Both => ClearSide::Both,
            liquide_style_engine::computed::Clear::InlineStart => ClearSide::Left,
            liquide_style_engine::computed::Clear::InlineEnd => ClearSide::Right,
            liquide_style_engine::computed::Clear::None => ClearSide::None,
        }
    }
}

/// Layout children of a block container with float support.
///
/// This is called from block layout when children may have `float` set.
/// It processes children in order: floated children are positioned via
/// the float context, and non-floated children have their available
/// width adjusted based on active floats.
pub fn layout_block_with_floats(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    text_measurer: &dyn TextMeasurer,
    image_measurer: &dyn ImageMeasurer,
    container_width: f32,
    container_height: f32,
    content_x: f32,
    content_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    base_font_size: f32,
    parent_box_id: LayoutBoxId,
) -> f32 {
    let mut float_ctx = FloatContext::new(container_width);
    let mut block_y = 0.0f32;

    let children = doc.children(node_id).to_vec();

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        // Handle clear
        let clear: ClearSide = child_style.clear.into();
        if clear != ClearSide::None {
            let clear_y = float_ctx.clear_y(clear);
            if clear_y > block_y {
                block_y = clear_y;
            }
        }

        let float_side = match child_style.float {
            Float::Left | Float::InlineStart => Some(FloatSide::Left),
            Float::Right | Float::InlineEnd => Some(FloatSide::Right),
            Float::None => None,
        };

        if let Some(side) = float_side {
            // Consume shape exclusion properties for CSS Shapes Level 1.
            // shape-outside defines the float exclusion area (circle, polygon, etc.),
            // shape-margin expands it, and shape-image-threshold sets the alpha cutoff
            // for image-based shapes. Full shape geometry computation is TODO.
            let _shape_outside = &child_style.shape_outside;
            let _shape_margin = child_style.shape_margin;
            let _shape_image_threshold = child_style.shape_image_threshold;

            // Layout the float to determine its intrinsic size
            let float_box = crate::block::layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                container_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            );

            let (fw, fh) = tree
                .get(float_box)
                .map(|b| (b.margin_rect.width, b.margin_rect.height))
                .unwrap_or((0.0, 0.0));

            // Place the float
            let placed = match side {
                FloatSide::Left => float_ctx.place_left(fw, fh, block_y),
                FloatSide::Right => float_ctx.place_right(fw, fh, block_y),
            };

            // Reposition float box to LOCAL coordinates (relative to parent content area)
            if let Some(b) = tree.get_mut(float_box) {
                let dx = placed.x - b.margin_rect.x;
                let dy = placed.y - b.margin_rect.y;
                b.content_rect.x += dx;
                b.content_rect.y += dy;
                b.padding_rect.x += dx;
                b.padding_rect.y += dy;
                b.border_rect.x += dx;
                b.border_rect.y += dy;
                b.margin_rect.x += dx;
                b.margin_rect.y += dy;
            }

            tree.add_child(parent_box_id, float_box);
        } else {
            // Normal flow child — determine available width around floats
            let line_height = child_style.font_size * 1.2; // estimate
            let (left_edge, _right_edge, avail_w) =
                float_ctx.available_width_at(block_y, line_height);

            let child_box = crate::block::layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                avail_w,
                container_height,
                content_x + left_edge,
                content_y + block_y,
                viewport_w,
                viewport_h,
                base_font_size,
            );

            let child_h = tree
                .get(child_box)
                .map(|b| b.margin_rect.height)
                .unwrap_or(0.0);

            tree.add_child(parent_box_id, child_box);
            block_y += child_h;
        }
    }

    // Final height must clear all floats
    let clear_all = float_ctx.clear_y(ClearSide::Both);
    block_y.max(clear_all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_context_basic() {
        let mut ctx = FloatContext::new(400.0);

        // Place a left float
        let rect = ctx.place_left(100.0, 50.0, 0.0);
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.width, 100.0);

        // Available width at y=0 should be reduced
        let (left, _right, avail) = ctx.available_width_at(0.0, 10.0);
        assert_eq!(left, 100.0);
        assert_eq!(avail, 300.0);

        // Available width below the float should be full
        let (left, _right, avail) = ctx.available_width_at(60.0, 10.0);
        assert_eq!(left, 0.0);
        assert_eq!(avail, 400.0);
    }

    #[test]
    fn float_context_left_and_right() {
        let mut ctx = FloatContext::new(400.0);
        ctx.place_left(100.0, 50.0, 0.0);
        ctx.place_right(80.0, 50.0, 0.0);

        let (left, right, avail) = ctx.available_width_at(0.0, 10.0);
        assert_eq!(left, 100.0);
        assert_eq!(right, 320.0);
        assert_eq!(avail, 220.0);
    }

    #[test]
    fn float_clear() {
        let mut ctx = FloatContext::new(400.0);
        ctx.place_left(100.0, 50.0, 0.0);
        ctx.place_right(80.0, 30.0, 10.0);

        assert_eq!(ctx.clear_y(ClearSide::Left), 50.0);
        assert_eq!(ctx.clear_y(ClearSide::Right), 40.0);
        assert_eq!(ctx.clear_y(ClearSide::Both), 50.0);
    }

    #[test]
    fn stacking_left_floats() {
        let mut ctx = FloatContext::new(400.0);
        let r1 = ctx.place_left(100.0, 50.0, 0.0);
        assert_eq!(r1.x, 0.0);

        // Second left float at same y should stack next to first
        let r2 = ctx.place_left(80.0, 50.0, 0.0);
        assert_eq!(r2.x, 100.0);
    }
}
