//! Positioned layout — absolute, fixed, and sticky positioning.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{Display, Position};

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};

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
        Position::Sticky => BoxType::Sticky,
        _ => return None,
    };

    let cb = if style.position == Position::Fixed {
        Rect::new(0.0, 0.0, viewport_w, viewport_h)
    } else {
        containing_rect
    };

    let is_sticky = style.position == Position::Sticky;

    let font_size = style.font_size;

    // ── Anchor positioning — CSS Anchor Positioning Level 1 ──────────
    // If this element itself has `anchor-name`, it was already registered
    // in the anchor registry during the registration pass.
    //
    // If `position-anchor` is set, look up the referenced anchor's rect
    // and use it as the reference for positioning instead of the
    // containing block.  `position-area` further constrains alignment.
    let anchor_rect = style.position_anchor.as_ref().and_then(|anchor_ref| {
        tree.anchor_registry.get(anchor_ref).copied()
    });
    let position_area = style.position_area.clone();

    // Resolve width/height
    let width = style
        .width
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h);
    let height =
        style
            .height
            .resolve_px(cb.height, base_font_size, font_size, viewport_w, viewport_h);

    // Resolve offsets
    let top = style
        .top
        .resolve_px(cb.height, base_font_size, font_size, viewport_w, viewport_h);
    let right = style
        .right
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h);
    let bottom =
        style
            .bottom
            .resolve_px(cb.height, base_font_size, font_size, viewport_w, viewport_h);
    let left = style
        .left
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h);

    // ── Resolve padding ──────────────────────────────────────────────────
    let pad_top = style
        .padding
        .top
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(0.0);
    let pad_right = style
        .padding
        .right
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(0.0);
    let pad_bottom = style
        .padding
        .bottom
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(0.0);
    let pad_left = style
        .padding
        .left
        .resolve_px(cb.width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(0.0);

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    // ── Determine content dimensions ────────────────────────────────────
    // If width/height are specified, use them after subtracting padding+border.
    // Otherwise, use intrinsic sizing via a child layout pass.
    let needs_intrinsic = width.is_none() && !(left.is_some() && right.is_some());

    // For intrinsic sizing, temporarily lay out children to measure content.
    let intrinsic_box = if needs_intrinsic {
        Some(crate::block::layout_block(
            doc,
            node_id,
            styles,
            tree,
            text_measurer,
            image_measurer,
            cb.width,
            cb.height,
            0.0,
            0.0,
            viewport_w,
            viewport_h,
            base_font_size,
        ))
    } else {
        None
    };

    let outer_w = width.unwrap_or_else(|| match (left, right) {
        (Some(l), Some(r)) => cb.width - l - r,
        _ => intrinsic_box
            .and_then(|id| tree.get(id).map(|b| b.border_rect.width))
            .unwrap_or(0.0),
    });
    let outer_h = height.unwrap_or_else(|| match (top, bottom) {
        (Some(t), Some(b_val)) => cb.height - t - b_val,
        _ => intrinsic_box
            .and_then(|id| tree.get(id).map(|b| b.border_rect.height))
            .unwrap_or(0.0),
    });

    let content_w = (outer_w - pad_left - pad_right - border_left - border_right).max(0.0);
    let content_h = (outer_h - pad_top - pad_bottom - border_top - border_bottom).max(0.0);

    // ── Calculate position ──────────────────────────────────────────────
    let (x, y) = if let Some(anchor) = anchor_rect {
        // Anchor positioning: place relative to the anchor element's rect.
        // The `position-area` value determines which edge/area to align to.
        anchor_position(anchor, outer_w, outer_h, position_area.as_deref())
    } else if is_sticky {
        // Sticky positioning: lay out in normal flow first, then clamp.
        // The normal-flow position is roughly cb.x / cb.y (the containing
        // block origin), since the element participates in normal flow.
        let normal_x = cb.x;
        let normal_y = cb.y;

        // Clamp so the element sticks within the containing block's
        // padding area, offset by the specified sticky edges.
        let min_x = if let Some(l) = left {
            cb.x + l
        } else {
            f32::NEG_INFINITY
        };
        let max_x = if let Some(r) = right {
            cb.x + cb.width - r - outer_w
        } else {
            f32::INFINITY
        };
        let min_y = if let Some(t) = top {
            cb.y + t
        } else {
            f32::NEG_INFINITY
        };
        let max_y = if let Some(b_val) = bottom {
            cb.y + cb.height - b_val - outer_h
        } else {
            f32::INFINITY
        };

        let clamped_x = normal_x.clamp(min_x, max_x);
        let clamped_y = normal_y.clamp(min_y, max_y);
        (clamped_x, clamped_y)
    } else {
        let x = if let Some(l) = left {
            cb.x + l
        } else if let Some(r) = right {
            cb.x + cb.width - r - outer_w
        } else {
            cb.x
        };

        let y = if let Some(t) = top {
            cb.y + t
        } else if let Some(b_val) = bottom {
            cb.y + cb.height - b_val - outer_h
        } else {
            cb.y
        };
        (x, y)
    };

    let content_x = x + border_left + pad_left;
    let content_y = y + border_top + pad_top;

    // ── Create the positioned box ───────────────────────────────────────
    let box_id = tree.alloc(node_id, box_type);
    if let Some(b) = tree.get_mut(box_id) {
        b.content_rect = Rect::new(content_x, content_y, content_w, content_h);
        b.padding_rect = Rect::new(
            x + border_left,
            y + border_top,
            content_w + pad_left + pad_right,
            content_h + pad_top + pad_bottom,
        );
        b.border_rect = Rect::new(x, y, outer_w, outer_h);
        b.margin_rect = b.border_rect; // positioned elements don't collapse margins
    }

    // ── Layout children within this positioned element ──────────────────
    // If we already did intrinsic layout, steal children from that box.
    // Otherwise, do a proper child layout now.
    if let Some(intrinsic_id) = intrinsic_box {
        // Intrinsic layout already laid out children in local coords.
        // Just re-parent them into the positioned box — no offset needed
        // because in the local coordinate model, children are always
        // positioned relative to their parent's content area.
        let child_ids: Vec<LayoutBoxId> = tree
            .get(intrinsic_id)
            .map(|b| b.children.clone())
            .unwrap_or_default();
        for child_id in child_ids {
            tree.add_child(box_id, child_id);
        }
    } else {
        // Lay out children in the content area of this box
        layout_children_in_positioned(
            doc,
            node_id,
            &style,
            styles,
            tree,
            text_measurer,
            image_measurer,
            content_w,
            content_h,
            0.0,  // children use local coords (0,0 = content origin)
            0.0,
            viewport_w,
            viewport_h,
            base_font_size,
            box_id,
        );
    }

    Some(box_id)
}

/// Lay out children of a positioned element inside its content area.
fn layout_children_in_positioned(
    doc: &Document,
    node_id: NodeId,
    style: &std::sync::Arc<liquide_style_engine::computed::ComputedStyle>,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    text_measurer: &dyn TextMeasurer,
    image_measurer: &dyn ImageMeasurer,
    content_w: f32,
    content_h: f32,
    content_x: f32,
    content_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    base_font_size: f32,
    parent_box: LayoutBoxId,
) {
    let children = doc.children(node_id).to_vec();

    if style.is_flex_container() {
        // Create a temporary flex container to lay out children.
        // We use layout_flex on this node but position it at content_x/content_y.
        // NOTE: layout_flex creates a box and registers it in node_index, but we
        // want the positioned parent_box to remain the canonical box for node_id.
        let flex_box = crate::flex::layout_flex(
            doc,
            node_id,
            styles,
            tree,
            text_measurer,
            image_measurer,
            content_w,
            content_h,
            content_x,
            content_y,
            viewport_w,
            viewport_h,
            base_font_size,
        );
        // Steal children from the flex box and add to the positioned parent
        let child_ids: Vec<LayoutBoxId> = tree
            .get(flex_box)
            .map(|b| b.children.clone())
            .unwrap_or_default();
        for child_id in child_ids {
            tree.add_child(parent_box, child_id);
        }
        // Restore node_index to point to the positioned box (parent_box),
        // not the temporary flex_box. This ensures hit-testing and lookups
        // find the correctly positioned element.
        tree.set_node_box(node_id, parent_box);
    } else if style.is_grid_container() {
        // Same issue: layout_grid creates a temporary box. We must restore
        // node_index to point to the positioned parent.
        let grid_box = crate::grid::layout_grid(
            doc,
            node_id,
            styles,
            tree,
            text_measurer,
            image_measurer,
            content_w,
            content_h,
            content_x,
            content_y,
            viewport_w,
            viewport_h,
            base_font_size,
        );
        let child_ids: Vec<LayoutBoxId> = tree
            .get(grid_box)
            .map(|b| b.children.clone())
            .unwrap_or_default();
        for child_id in child_ids {
            tree.add_child(parent_box, child_id);
        }
        // Restore node_index to point to the positioned parent_box
        tree.set_node_box(node_id, parent_box);
    } else {
        // Block-level children
        let mut child_y = 0.0f32;
        for &child_id in &children {
            let child_style = styles.get(child_id).cloned().unwrap_or_default();
            if child_style.display == Display::None {
                continue;
            }
            if matches!(child_style.position, Position::Absolute | Position::Fixed) {
                continue;
            }

            // Check if child is a text node
            if let Some(child_node) = doc.get(child_id) {
                if child_node.is_text() {
                    if let Some(text) = child_node.text_content() {
                        let text_props = crate::TextProperties::from_style(&child_style);
                        let metrics = text_measurer.measure(
                            text,
                            child_style.font_size,
                            &child_style.font_family,
                            child_style.font_weight,
                            Some(content_w),
                            &text_props,
                        );
                        let text_box = tree.alloc(
                            child_id,
                            BoxType::Text {
                                line_boxes: Vec::new(),
                            },
                        );
                        if let Some(tb) = tree.get_mut(text_box) {
                            tb.content_rect = Rect::new(
                                content_x,
                                content_y + child_y,
                                metrics.width,
                                metrics.height,
                            );
                            tb.padding_rect = tb.content_rect;
                            tb.border_rect = tb.content_rect;
                            tb.margin_rect = tb.content_rect;
                            tb.baseline = Some(metrics.baseline);
                        }
                        tree.add_child(parent_box, text_box);
                        child_y += metrics.height;
                        continue;
                    }
                }
            }

            let child_box = crate::block::layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                content_w,
                content_h,
                content_x,
                content_y + child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            );
            tree.add_child(parent_box, child_box);
            if let Some(cb) = tree.get(child_box) {
                child_y += cb.margin_rect.height;
            }
        }
    }
}

/// Compute position for an anchor-positioned element.
///
/// Given the anchor element's absolute rect, the positioned element's
/// outer size, and an optional `position-area` keyword, returns the
/// (x, y) placement.
///
/// Supported `position-area` keywords (CSS Anchor Positioning Level 1):
///   - `"top"` — centered above the anchor
///   - `"bottom"` — centered below the anchor
///   - `"left"` — to the left, vertically centered
///   - `"right"` — to the right, vertically centered
///   - `"center"` — centered over the anchor
///   - `"top left"` / `"top right"` / `"bottom left"` / `"bottom right"`
///
/// If `position_area` is None or unrecognized, defaults to placing the
/// element directly below the anchor, horizontally aligned to its start.
fn anchor_position(
    anchor: Rect,
    elem_w: f32,
    elem_h: f32,
    position_area: Option<&str>,
) -> (f32, f32) {
    let area = position_area.unwrap_or("");
    // Normalize: lowercase, trim
    let area = area.trim();

    match area {
        "top" => {
            // Center above the anchor.
            let x = anchor.x + (anchor.width - elem_w) / 2.0;
            let y = anchor.y - elem_h;
            (x, y)
        }
        "bottom" => {
            // Center below the anchor.
            let x = anchor.x + (anchor.width - elem_w) / 2.0;
            let y = anchor.y + anchor.height;
            (x, y)
        }
        "left" => {
            // To the left, vertically centered.
            let x = anchor.x - elem_w;
            let y = anchor.y + (anchor.height - elem_h) / 2.0;
            (x, y)
        }
        "right" => {
            // To the right, vertically centered.
            let x = anchor.x + anchor.width;
            let y = anchor.y + (anchor.height - elem_h) / 2.0;
            (x, y)
        }
        "center" => {
            // Centered over the anchor.
            let x = anchor.x + (anchor.width - elem_w) / 2.0;
            let y = anchor.y + (anchor.height - elem_h) / 2.0;
            (x, y)
        }
        "top left" => {
            let x = anchor.x - elem_w;
            let y = anchor.y - elem_h;
            (x, y)
        }
        "top right" => {
            let x = anchor.x + anchor.width;
            let y = anchor.y - elem_h;
            (x, y)
        }
        "bottom left" => {
            let x = anchor.x - elem_w;
            let y = anchor.y + anchor.height;
            (x, y)
        }
        "bottom right" => {
            let x = anchor.x + anchor.width;
            let y = anchor.y + anchor.height;
            (x, y)
        }
        _ => {
            // Default: place below the anchor, aligned to its left edge.
            let x = anchor.x;
            let y = anchor.y + anchor.height;
            (x, y)
        }
    }
}
