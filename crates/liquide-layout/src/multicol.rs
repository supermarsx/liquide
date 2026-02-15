//! Multi-column layout — CSS Multi-column Layout Level 1.
//!
//! Splits content into multiple columns when `column-count` or `column-width` is set.
//!
//! Algorithm:
//! 1. Calculate number of columns from `column-count` and/or `column-width`
//! 2. Calculate column width from available width, number of columns, and `column-gap`
//! 3. Layout children as block flow within a single virtual column
//! 4. When content exceeds column height, start a new column
//! 5. Position columns side by side

use liquide_dom::{Document, NodeId};
use liquide_style_engine::computed::{Display, Position};
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::StyleMap;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};

/// Perform multi-column layout.
pub fn layout_multicol(
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

    let font_size = style.font_size;
    let width = style
        .width
        .resolve_px(container_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(container_width);

    // Resolve padding
    let pad_top = rdim(&style.padding.top, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_right = rdim(&style.padding.right, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_bottom = rdim(&style.padding.bottom, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_left = rdim(&style.padding.left, width, base_font_size, font_size, viewport_w, viewport_h);

    // Resolve margins
    let mar_top = rdim(&style.margin.top, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_right = rdim(&style.margin.right, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_bottom = rdim(&style.margin.bottom, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_left = rdim(&style.margin.left, container_width, base_font_size, font_size, viewport_w, viewport_h);

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    let content_width = (width - pad_left - pad_right - border_left - border_right).max(0.0);
    let content_x = offset_x + mar_left + border_left + pad_left;
    let content_y = offset_y + mar_top + border_top + pad_top;

    // ── Determine column parameters ──
    let column_gap = style
        .column_gap
        .resolve_px(content_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(font_size); // CSS default column-gap is `normal` ≈ 1em

    let column_width_hint = style
        .column_width
        .resolve_px(content_width, base_font_size, font_size, viewport_w, viewport_h);

    // Resolve column count per CSS Multi-column spec §3:
    //   If column-count is set, use it.
    //   If only column-width is set, compute count from available width.
    //   If both are set, column-count is an upper limit.
    let column_count = match (style.column_count, column_width_hint) {
        (Some(count), Some(cw)) if cw > 0.0 => {
            let derived = ((content_width + column_gap) / (cw + column_gap)).floor().max(1.0) as u32;
            derived.min(count).max(1)
        }
        (Some(count), _) => count.max(1),
        (None, Some(cw)) if cw > 0.0 => {
            ((content_width + column_gap) / (cw + column_gap)).floor().max(1.0) as u32
        }
        _ => 1,
    };

    let col_width = if column_count > 1 {
        (content_width - (column_count - 1) as f32 * column_gap) / column_count as f32
    } else {
        content_width
    };

    // ── Layout all children as block flow ──
    let children = doc.children(node_id).to_vec();
    let mut child_boxes: Vec<(LayoutBoxId, f32)> = Vec::new();
    let mut total_height = 0.0f32;

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        // Layout each child into a single column-width block
        let child_box = crate::block::layout_block(
            doc,
            child_id,
            styles,
            tree,
            text_measurer,
            image_measurer,
            col_width,
            container_height,
            0.0,
            0.0,
            viewport_w,
            viewport_h,
            base_font_size,
        );

        let h = tree
            .get(child_box)
            .map(|b| b.margin_rect.height)
            .unwrap_or(0.0);
        child_boxes.push((child_box, h));
        total_height += h;
        tree.add_child(box_id, child_box);
    }

    // ── Determine column height ──
    // If explicit height is set, use it as the column height.
    // Otherwise, balance by dividing total content height among columns.
    let explicit_height = style
        .height
        .resolve_px(container_height, base_font_size, font_size, viewport_w, viewport_h);

    let col_height = explicit_height.unwrap_or_else(|| {
        if column_count > 1 {
            // Balanced columns: aim for equal height
            (total_height / column_count as f32).max(1.0)
        } else {
            total_height
        }
    });

    // ── Distribute children into columns ──
    let mut current_col = 0u32;
    let mut col_y = 0.0f32;
    let mut max_col_height = 0.0f32;

    for &(child_box_id, child_h) in &child_boxes {
        // Break to next column if needed
        if col_y > 0.0 && col_y + child_h > col_height && current_col + 1 < column_count {
            if col_y > max_col_height {
                max_col_height = col_y;
            }
            current_col += 1;
            col_y = 0.0;
        }

        let col_x = content_x + current_col as f32 * (col_width + column_gap);
        let target_y = content_y + col_y;

        // Reposition the child box by applying a delta
        if let Some(b) = tree.get_mut(child_box_id) {
            let dx = col_x - b.content_rect.x;
            let dy = target_y - b.content_rect.y;
            b.content_rect.x += dx;
            b.content_rect.y += dy;
            b.padding_rect.x += dx;
            b.padding_rect.y += dy;
            b.border_rect.x += dx;
            b.border_rect.y += dy;
            b.margin_rect.x += dx;
            b.margin_rect.y += dy;
        }

        col_y += child_h;
    }

    // Track the last column's height
    if col_y > max_col_height {
        max_col_height = col_y;
    }

    let content_height = explicit_height.unwrap_or(max_col_height);

    // ── Set container geometry ──
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

fn rdim(
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

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};
    use crate::{DefaultTextMeasurer, DefaultImageMeasurer};

    #[test]
    fn basic_multicol_layout() {
        let mut doc = Document::new();
        let root = doc.root();
        let container = doc.create_element("div");
        doc.append_child(root, container);

        for _ in 0..6 {
            let child = doc.create_element("p");
            doc.append_child(container, child);
        }

        let mut engine = StyleEngine::new(ViewportSize { width: 800.0, height: 600.0 }, 16.0);
        engine.add_stylesheet(
            "div { column-count: 3; width: 600px; } p { height: 50px; }",
        );

        let styles = engine.restyle_all(&doc);
        let mut tree = LayoutTree::new();

        let box_id = layout_multicol(
            &doc,
            container,
            &styles,
            &mut tree,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
            800.0, 600.0, 0.0, 0.0, 800.0, 600.0, 16.0,
        );

        assert!(tree.get(box_id).is_some());
        // Should have 6 children distributed across 3 columns
        let container_box = tree.get(box_id).unwrap();
        assert_eq!(container_box.children.len(), 6);
    }

    #[test]
    fn single_column_fallback() {
        let mut doc = Document::new();
        let root = doc.root();
        let container = doc.create_element("div");
        let child = doc.create_element("p");
        doc.append_child(root, container);
        doc.append_child(container, child);

        let mut engine = StyleEngine::default();
        engine.add_stylesheet("div { width: 200px; } p { height: 30px; }");

        let styles = engine.restyle_all(&doc);
        let mut tree = LayoutTree::new();

        let box_id = layout_multicol(
            &doc,
            container,
            &styles,
            &mut tree,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
            200.0, 600.0, 0.0, 0.0, 200.0, 600.0, 16.0,
        );

        // No column-count set → single column, like block layout
        let b = tree.get(box_id).unwrap();
        assert!(b.content_rect.width > 0.0);
    }
}
