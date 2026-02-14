//! Grid layout — CSS Grid Level 1 (simplified).

use liquide_dom::{Document, NodeId};
use liquide_style_engine::computed::{Display, GridAutoFlow, TrackSize, Position};
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::StyleMap;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{TextMeasurer, ImageMeasurer};

/// Perform grid layout.
pub fn layout_grid(
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
    let box_id = tree.alloc(node_id, BoxType::Grid);

    let font_size = style.font_size;
    let width = style
        .width
        .resolve_px(container_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(container_width);

    let pad_top = resolve_dim(&style.padding.top, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_right = resolve_dim(&style.padding.right, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_bottom = resolve_dim(&style.padding.bottom, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_left = resolve_dim(&style.padding.left, width, base_font_size, font_size, viewport_w, viewport_h);

    let mar_top = resolve_dim(&style.margin.top, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_right = resolve_dim(&style.margin.right, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_bottom = resolve_dim(&style.margin.bottom, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_left = resolve_dim(&style.margin.left, container_width, base_font_size, font_size, viewport_w, viewport_h);

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    let content_width = width - pad_left - pad_right - border_left - border_right;
    let content_x = offset_x + mar_left + border_left + pad_left;
    let content_y = offset_y + mar_top + border_top + pad_top;

    let gap_col = style.gap.width
        .resolve_px(content_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(0.0);
    let gap_row = style.gap.height
        .resolve_px(content_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(0.0);

    // Resolve column track sizes
    let col_tracks = resolve_tracks(&style.grid_template_columns, content_width, gap_col);
    let num_cols = col_tracks.len().max(1);

    // Collect children
    let children = doc.children(node_id).to_vec();
    let mut grid_items: Vec<NodeId> = Vec::new();
    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }
        grid_items.push(child_id);
    }

    // Auto-placement
    let num_rows = (grid_items.len() + num_cols - 1) / num_cols;
    let row_tracks = if style.grid_template_rows.is_empty() {
        vec![0.0f32; num_rows] // auto rows
    } else {
        let available_h = style.height
            .resolve_px(container_height, base_font_size, font_size, viewport_w, viewport_h)
            .unwrap_or(container_height);
        resolve_tracks(&style.grid_template_rows, available_h, gap_row)
    };

    // Layout each item into its cell
    let mut row_heights: Vec<f32> = vec![0.0; num_rows];

    for (idx, &child_id) in grid_items.iter().enumerate() {
        let col = idx % num_cols;
        let row = idx / num_cols;
        if row >= num_rows {
            break;
        }

        let cell_width = if col < col_tracks.len() { col_tracks[col] } else { content_width / num_cols as f32 };

        // Layout child in cell
        let child_box = crate::block::layout_block(
            doc, child_id, styles, tree, text_measurer, image_measurer,
            cell_width, container_height, 0.0, 0.0,
            viewport_w, viewport_h, base_font_size,
        );

        if let Some(cb) = tree.get(child_box) {
            row_heights[row] = row_heights[row].max(cb.margin_rect.height);
        }

        tree.add_child(box_id, child_box);
    }

    // Use explicit row heights where provided
    for (i, h) in row_tracks.iter().enumerate() {
        if i < row_heights.len() && *h > 0.0 {
            row_heights[i] = *h;
        }
    }

    // Position items
    let mut y_offsets: Vec<f32> = vec![0.0; num_rows];
    let mut cumulative_y = 0.0f32;
    for row in 0..num_rows {
        y_offsets[row] = cumulative_y;
        cumulative_y += row_heights[row] + if row < num_rows - 1 { gap_row } else { 0.0 };
    }

    let mut x_offsets: Vec<f32> = vec![0.0; num_cols];
    let mut cumulative_x = 0.0f32;
    for col in 0..num_cols {
        x_offsets[col] = cumulative_x;
        let cw = if col < col_tracks.len() { col_tracks[col] } else { content_width / num_cols as f32 };
        cumulative_x += cw + if col < num_cols - 1 { gap_col } else { 0.0 };
    }

    // Update child positions
    let child_boxes: Vec<LayoutBoxId> = tree.get(box_id)
        .map(|b| b.children.clone())
        .unwrap_or_default();

    for (idx, &child_box_id) in child_boxes.iter().enumerate() {
        let col = idx % num_cols;
        let row = idx / num_cols;
        if row >= num_rows {
            break;
        }

        let cell_x = content_x + x_offsets.get(col).copied().unwrap_or(0.0);
        let cell_y = content_y + y_offsets.get(row).copied().unwrap_or(0.0);
        let cell_w = if col < col_tracks.len() { col_tracks[col] } else { content_width / num_cols as f32 };
        let cell_h = row_heights[row];

        if let Some(b) = tree.get_mut(child_box_id) {
            b.content_rect = Rect::new(cell_x, cell_y, cell_w, cell_h);
            b.padding_rect = b.content_rect;
            b.border_rect = b.content_rect;
            b.margin_rect = b.content_rect;
        }
    }

    // Set container geometry
    let content_height = cumulative_y;
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

/// Resolve track sizes, handling fr units and fixed sizes.
fn resolve_tracks(tracks: &[TrackSize], available: f32, gap: f32) -> Vec<f32> {
    if tracks.is_empty() {
        return Vec::new();
    }

    let total_gap = if tracks.len() > 1 { (tracks.len() - 1) as f32 * gap } else { 0.0 };
    let mut fixed_total = total_gap;
    let mut fr_total = 0.0f32;
    let mut sizes = vec![0.0f32; tracks.len()];

    for (i, track) in tracks.iter().enumerate() {
        match track {
            TrackSize::Px(v) => {
                sizes[i] = *v;
                fixed_total += *v;
            }
            TrackSize::Percent(v) => {
                let px = available * v / 100.0;
                sizes[i] = px;
                fixed_total += px;
            }
            TrackSize::Fr(v) => {
                fr_total += *v;
            }
            TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent => {
                // Auto tracks get a share of remaining space
                fr_total += 1.0;
            }
            _ => {
                fr_total += 1.0;
            }
        }
    }

    // Distribute remaining space among fr tracks
    let remaining = (available - fixed_total).max(0.0);
    if fr_total > 0.0 {
        for (i, track) in tracks.iter().enumerate() {
            match track {
                TrackSize::Fr(v) => sizes[i] = remaining * (*v / fr_total),
                TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent => {
                    sizes[i] = remaining * (1.0 / fr_total);
                }
                TrackSize::Px(_) | TrackSize::Percent(_) => {} // already set
                _ => sizes[i] = remaining * (1.0 / fr_total),
            }
        }
    }

    sizes
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
