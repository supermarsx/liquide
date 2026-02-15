//! Table layout — basic CSS table formatting context.
//!
//! Implements a simplified table layout:
//! 1. Identify table-row and table-cell children
//! 2. Calculate column widths (equal distribution or content-based)
//! 3. Position cells in a grid pattern
//!
//! This handles `display: table` containers with `display: table-row`
//! and `display: table-cell` children. Also works with `<table>`, `<tr>`,
//! `<td>` elements by tag name as a fallback.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::computed::{Display, Position};
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::StyleMap;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};

/// A resolved table row.
struct TableRow {
    _node_id: NodeId,
    cells: Vec<TableCell>,
}

/// A resolved table cell.
struct TableCell {
    _node_id: NodeId,
    box_id: LayoutBoxId,
    intrinsic_width: f32,
    intrinsic_height: f32,
}

/// Perform table layout.
///
/// Lays out a table container by:
/// 1. Collecting rows and cells from child elements
/// 2. Computing column widths based on content or equal distribution
/// 3. Positioning cells in a grid pattern with proper row heights
pub fn layout_table(
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

    let border_spacing = style.border_spacing;

    // ── Step 1: Collect rows and cells ──
    let children = doc.children(node_id).to_vec();
    let mut rows: Vec<TableRow> = Vec::new();

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        let is_row = child_style.display == Display::TableRow || is_table_row_tag(doc, child_id);

        if is_row {
            // This child is a row — collect its cells
            let row_children = doc.children(child_id).to_vec();
            let mut cells = Vec::new();
            for &cell_id in &row_children {
                let cell_style = styles.get(cell_id).cloned().unwrap_or_default();
                if cell_style.display == Display::None {
                    continue;
                }

                // Layout cell to get intrinsic size
                let cell_box = crate::block::layout_block(
                    doc, cell_id, styles, tree, text_measurer, image_measurer,
                    content_width, container_height, 0.0, 0.0,
                    viewport_w, viewport_h, base_font_size,
                );

                let (iw, ih) = tree
                    .get(cell_box)
                    .map(|b| (b.margin_rect.width, b.margin_rect.height))
                    .unwrap_or((0.0, 0.0));

                cells.push(TableCell {
                    _node_id: cell_id,
                    box_id: cell_box,
                    intrinsic_width: iw,
                    intrinsic_height: ih,
                });

                tree.add_child(box_id, cell_box);
            }
            rows.push(TableRow {
                _node_id: child_id,
                cells,
            });
        } else {
            // Non-row child: treat as a single-cell row (anonymous table row)
            let cell_box = crate::block::layout_block(
                doc, child_id, styles, tree, text_measurer, image_measurer,
                content_width, container_height, 0.0, 0.0,
                viewport_w, viewport_h, base_font_size,
            );

            let (iw, ih) = tree
                .get(cell_box)
                .map(|b| (b.margin_rect.width, b.margin_rect.height))
                .unwrap_or((0.0, 0.0));

            rows.push(TableRow {
                _node_id: child_id,
                cells: vec![TableCell {
                    _node_id: child_id,
                    box_id: cell_box,
                    intrinsic_width: iw,
                    intrinsic_height: ih,
                }],
            });

            tree.add_child(box_id, cell_box);
        }
    }

    // ── Step 2: Determine column count and widths ──
    let num_cols = rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if num_cols == 0 {
        // Empty table
        set_table_geometry(
            tree, box_id, content_x, content_y, content_width, 0.0,
            pad_top, pad_right, pad_bottom, pad_left,
            border_top, border_right, border_bottom, border_left,
            mar_top, mar_right, mar_bottom, mar_left,
        );
        return box_id;
    }

    // Compute max intrinsic width per column
    let mut col_max_widths = vec![0.0f32; num_cols];
    for row in &rows {
        for (ci, cell) in row.cells.iter().enumerate() {
            if ci < num_cols {
                col_max_widths[ci] = col_max_widths[ci].max(cell.intrinsic_width);
            }
        }
    }

    // Total intrinsic width
    let total_spacing = if num_cols > 1 {
        (num_cols - 1) as f32 * border_spacing
    } else {
        0.0
    };
    let total_intrinsic: f32 = col_max_widths.iter().sum::<f32>() + total_spacing;

    // Distribute widths: if intrinsic fits, use it; otherwise scale proportionally
    let col_widths: Vec<f32> = if total_intrinsic <= content_width && total_intrinsic > 0.0 {
        // Distribute extra space equally
        let extra = (content_width - total_intrinsic) / num_cols as f32;
        col_max_widths.iter().map(|w| w + extra).collect()
    } else if total_intrinsic > 0.0 {
        // Scale down proportionally
        let scale = (content_width - total_spacing).max(0.0) / (total_intrinsic - total_spacing).max(1.0);
        col_max_widths.iter().map(|w| w * scale).collect()
    } else {
        // Equal distribution
        let cw = (content_width - total_spacing).max(0.0) / num_cols as f32;
        vec![cw; num_cols]
    };

    // Pre-compute column x positions
    let mut col_x_positions = Vec::with_capacity(num_cols);
    let mut cx = 0.0f32;
    for (ci, &cw) in col_widths.iter().enumerate() {
        col_x_positions.push(cx);
        cx += cw;
        if ci < num_cols - 1 {
            cx += border_spacing;
        }
    }

    // ── Step 3: Position cells in grid pattern ──
    let mut row_y = 0.0f32;

    for row in &rows {
        // Determine row height: max cell height in this row
        let row_height = row
            .cells
            .iter()
            .map(|c| c.intrinsic_height)
            .fold(0.0f32, |a, b| a.max(b));

        for (ci, cell) in row.cells.iter().enumerate() {
            if ci >= num_cols {
                break;
            }

            let cell_x = content_x + col_x_positions[ci];
            let cell_y = content_y + row_y;
            let cell_w = col_widths[ci];

            // Reposition the cell box
            if let Some(b) = tree.get_mut(cell.box_id) {
                let dx = cell_x - b.content_rect.x;
                let dy = cell_y - b.content_rect.y;
                let dw = cell_w - b.content_rect.width;
                let dh = row_height - b.content_rect.height;

                b.content_rect.x += dx;
                b.content_rect.y += dy;
                b.content_rect.width += dw;
                b.content_rect.height += dh;
                b.padding_rect.x += dx;
                b.padding_rect.y += dy;
                b.padding_rect.width += dw;
                b.padding_rect.height += dh;
                b.border_rect.x += dx;
                b.border_rect.y += dy;
                b.border_rect.width += dw;
                b.border_rect.height += dh;
                b.margin_rect.x += dx;
                b.margin_rect.y += dy;
                b.margin_rect.width += dw;
                b.margin_rect.height += dh;
            }
        }

        row_y += row_height + border_spacing;
    }

    // Remove trailing spacing
    if !rows.is_empty() {
        row_y -= border_spacing;
    }

    let content_height = style
        .height
        .resolve_px(container_height, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(row_y);

    set_table_geometry(
        tree, box_id, content_x, content_y, content_width, content_height,
        pad_top, pad_right, pad_bottom, pad_left,
        border_top, border_right, border_bottom, border_left,
        mar_top, mar_right, mar_bottom, mar_left,
    );

    box_id
}

/// Check if a node's tag name indicates a table row.
fn is_table_row_tag(doc: &Document, node_id: NodeId) -> bool {
    doc.get(node_id)
        .map(|n| {
            let binding = n.tag_name();
            let tag = binding.as_str();
            tag == "tr" || tag == "thead" || tag == "tbody" || tag == "tfoot"
        })
        .unwrap_or(false)
}

/// Set geometry for the table container box.
#[allow(clippy::too_many_arguments)]
fn set_table_geometry(
    tree: &mut LayoutTree,
    box_id: LayoutBoxId,
    content_x: f32,
    content_y: f32,
    content_width: f32,
    content_height: f32,
    pad_top: f32,
    pad_right: f32,
    pad_bottom: f32,
    pad_left: f32,
    border_top: f32,
    border_right: f32,
    border_bottom: f32,
    border_left: f32,
    mar_top: f32,
    mar_right: f32,
    mar_bottom: f32,
    mar_left: f32,
) {
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
    fn basic_table_layout() {
        let mut doc = Document::new();
        let root = doc.root();
        let table = doc.create_element("table");
        doc.append_child(root, table);

        // Create two rows with two cells each
        for _ in 0..2 {
            let tr = doc.create_element("tr");
            doc.append_child(table, tr);
            for _ in 0..2 {
                let td = doc.create_element("td");
                doc.append_child(tr, td);
            }
        }

        let mut engine = StyleEngine::new(ViewportSize { width: 800.0, height: 600.0 }, 16.0);
        engine.add_stylesheet(
            "table { display: table; width: 400px; } tr { display: table-row; } td { display: table-cell; height: 30px; }",
        );

        let styles = engine.restyle_all(&doc);
        let mut tree = LayoutTree::new();

        let box_id = layout_table(
            &doc,
            table,
            &styles,
            &mut tree,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
            800.0, 600.0, 0.0, 0.0, 800.0, 600.0, 16.0,
        );

        let table_box = tree.get(box_id).unwrap();
        assert_eq!(table_box.children.len(), 4); // 2 rows × 2 cells
        assert!(table_box.content_rect.width > 0.0);
    }

    #[test]
    fn empty_table() {
        let mut doc = Document::new();
        let root = doc.root();
        let table = doc.create_element("table");
        doc.append_child(root, table);

        let engine = StyleEngine::default();
        let styles = engine.restyle_all(&doc);
        let mut tree = LayoutTree::new();

        let box_id = layout_table(
            &doc,
            table,
            &styles,
            &mut tree,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
            800.0, 600.0, 0.0, 0.0, 800.0, 600.0, 16.0,
        );

        assert!(tree.get(box_id).is_some());
    }
}
