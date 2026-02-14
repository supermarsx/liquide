//! Block layout — CSS block formatting context.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::computed::{Display, Position};
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::StyleMap;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{TextMeasurer, ImageMeasurer};

/// Perform block layout for a node and its children.
///
/// Block layout places children vertically, one below another, each taking
/// the full available width (unless the child has its own width constraint).
pub fn layout_block(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    text_measurer: &dyn TextMeasurer,
    image_measurer: &dyn ImageMeasurer,
    container_width: f32,
    container_height: f32,
    offset_x: f32,
    offset_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    base_font_size: f32,
) -> LayoutBoxId {
    let style = styles.get(node_id).cloned().unwrap_or_default();

    let box_id = tree.alloc(node_id, BoxType::Block);

    // Resolve own dimensions
    let font_size = style.font_size;
    let width = style
        .width
        .resolve_px(container_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(container_width);

    // Resolve padding
    let pad_top = resolve_dim(&style.padding.top, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_right = resolve_dim(&style.padding.right, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_bottom = resolve_dim(&style.padding.bottom, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_left = resolve_dim(&style.padding.left, width, base_font_size, font_size, viewport_w, viewport_h);

    // Resolve margin
    let mar_top = resolve_dim(&style.margin.top, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_right = resolve_dim(&style.margin.right, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_bottom = resolve_dim(&style.margin.bottom, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_left = resolve_dim(&style.margin.left, container_width, base_font_size, font_size, viewport_w, viewport_h);

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    let content_width = width - pad_left - pad_right - border_left - border_right;

    // Layout children
    let mut child_y = 0.0f32;
    let children = doc.children(node_id).to_vec();

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();

        // Skip display: none
        if child_style.display == Display::None {
            continue;
        }

        // Positioned children are handled separately
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        // Check if child is a text node
        if let Some(child_node) = doc.get(child_id) {
            if child_node.is_text() {
                if let Some(text) = child_node.text_content() {
                    let metrics = text_measurer.measure(
                        text,
                        child_style.font_size,
                        &child_style.font_family,
                        child_style.font_weight,
                        Some(content_width),
                    );
                    let text_box = tree.alloc(child_id, BoxType::Text { line_boxes: Vec::new() });
                    if let Some(tb) = tree.get_mut(text_box) {
                        tb.content_rect = Rect::new(0.0, child_y, metrics.width, metrics.height);
                        tb.padding_rect = tb.content_rect;
                        tb.border_rect = tb.content_rect;
                        tb.margin_rect = tb.content_rect;
                        tb.baseline = Some(metrics.baseline);
                    }
                    tree.add_child(box_id, text_box);
                    child_y += metrics.height;
                    continue;
                }
            }
        }

        // Recurse for element children
        let child_box = if child_style.is_flex_container() {
            crate::flex::layout_flex(
                doc, child_id, styles, tree, text_measurer, image_measurer,
                content_width, container_height, 0.0, child_y,
                viewport_w, viewport_h, base_font_size,
            )
        } else if child_style.is_grid_container() {
            crate::grid::layout_grid(
                doc, child_id, styles, tree, text_measurer, image_measurer,
                content_width, container_height, 0.0, child_y,
                viewport_w, viewport_h, base_font_size,
            )
        } else {
            layout_block(
                doc, child_id, styles, tree, text_measurer, image_measurer,
                content_width, container_height, 0.0, child_y,
                viewport_w, viewport_h, base_font_size,
            )
        };

        tree.add_child(box_id, child_box);

        if let Some(cb) = tree.get(child_box) {
            child_y += cb.margin_rect.height;
        }
    }

    // Content height is sum of children, or explicit height
    let content_height = style
        .height
        .resolve_px(container_height, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(child_y);

    // Set geometry
    let content_x = offset_x + mar_left + border_left + pad_left;
    let content_y = offset_y + mar_top + border_top + pad_top;

    if let Some(b) = tree.get_mut(box_id) {
        b.content_rect = Rect::new(content_x, content_y, content_width, content_height);
        b.padding_rect = Rect::new(
            content_x - pad_left,
            content_y - pad_top,
            content_width + pad_left + pad_right,
            content_height + pad_top + pad_bottom,
        );
        b.border_rect = Rect::new(
            b.padding_rect.x - border_left,
            b.padding_rect.y - border_top,
            b.padding_rect.width + border_left + border_right,
            b.padding_rect.height + border_top + border_bottom,
        );
        b.margin_rect = Rect::new(
            b.border_rect.x - mar_left,
            b.border_rect.y - mar_top,
            b.border_rect.width + mar_left + mar_right,
            b.border_rect.height + mar_top + mar_bottom,
        );
    }

    box_id
}

fn resolve_dim(
    dim: &Dimension,
    parent_px: f32,
    base_font_size: f32,
    font_size: f32,
    vw: f32,
    vh: f32,
) -> f32 {
    dim.resolve_px(parent_px, base_font_size, font_size, vw, vh)
        .unwrap_or(0.0)
}
