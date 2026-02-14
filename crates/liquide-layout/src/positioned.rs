//! Positioned layout — absolute, fixed, and sticky positioning.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::computed::Position;
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::StyleMap;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{TextMeasurer, ImageMeasurer};

/// Layout positioned (absolute/fixed) elements after normal flow.
///
/// `containing_rect` is the border box of the containing block.
pub fn layout_positioned(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    text_measurer: &dyn TextMeasurer,
    image_measurer: &dyn ImageMeasurer,
    containing_rect: Rect,
    viewport_w: f32,
    viewport_h: f32,
    base_font_size: f32,
) -> Option<LayoutBoxId> {
    let style = styles.get(node_id).cloned().unwrap_or_default();

    let box_type = match style.position {
        Position::Absolute => BoxType::Absolute,
        Position::Fixed => BoxType::Fixed,
        _ => return None,
    };

    let cb = if style.position == Position::Fixed {
        Rect::new(0.0, 0.0, viewport_w, viewport_h)
    } else {
        containing_rect
    };

    let font_size = style.font_size;

    // Resolve width/height
    let width = style
        .width
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h);
    let height = style
        .height
        .resolve_px(cb.height, base_font_size, font_size, viewport_w, viewport_h);

    // Resolve offsets
    let top = style
        .top
        .resolve_px(cb.height, base_font_size, font_size, viewport_w, viewport_h);
    let right = style
        .right
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h);
    let bottom = style
        .bottom
        .resolve_px(cb.height, base_font_size, font_size, viewport_w, viewport_h);
    let left = style
        .left
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h);

    // If layout is needed for content sizing, do a mini-layout
    let content_w = width.unwrap_or_else(|| {
        match (left, right) {
            (Some(l), Some(r)) => cb.width - l - r,
            _ => {
                // Intrinsic width from content
                let child_box = crate::block::layout_block(
                    doc, node_id, styles, tree, text_measurer, image_measurer,
                    cb.width, cb.height, 0.0, 0.0,
                    viewport_w, viewport_h, base_font_size,
                );
                tree.get(child_box).map(|b| b.content_rect.width).unwrap_or(0.0)
            }
        }
    });
    let content_h = height.unwrap_or_else(|| {
        match (top, bottom) {
            (Some(t), Some(b_val)) => cb.height - t - b_val,
            _ => 0.0,
        }
    });

    // Calculate position
    let x = if let Some(l) = left {
        cb.x + l
    } else if let Some(r) = right {
        cb.x + cb.width - r - content_w
    } else {
        cb.x
    };

    let y = if let Some(t) = top {
        cb.y + t
    } else if let Some(b_val) = bottom {
        cb.y + cb.height - b_val - content_h
    } else {
        cb.y
    };

    let box_id = tree.alloc(node_id, box_type);
    if let Some(b) = tree.get_mut(box_id) {
        b.content_rect = Rect::new(x, y, content_w, content_h);
        b.padding_rect = b.content_rect;
        b.border_rect = b.content_rect;
        b.margin_rect = b.content_rect;
    }

    Some(box_id)
}
