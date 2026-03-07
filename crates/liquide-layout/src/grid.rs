//! Grid layout — CSS Grid Level 1 (simplified).

use std::collections::HashMap;

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{AlignContent, AlignItems, AlignSelf, BoxSizing, Display, GridAutoFlow, GridLine, JustifyContent, JustifyItems, JustifySelf, Position, TrackSize};
use liquide_style_engine::dimension::Dimension;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};

/// A resolved grid item with its placement coordinates.
struct GridItem {
    node_id: NodeId,
    col_start: usize,
    col_end: usize, // exclusive
    row_start: usize,
    row_end: usize, // exclusive
    box_id: Option<LayoutBoxId>,
}

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
    let explicit_width = style
        .width
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        );
    let width = explicit_width.unwrap_or(container_width);

    let pad_top = resolve_dim(
        &style.padding.top,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_right = resolve_dim(
        &style.padding.right,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_bottom = resolve_dim(
        &style.padding.bottom,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_left = resolve_dim(
        &style.padding.left,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );

    let mar_top = resolve_dim(
        &style.margin.top,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let mar_right = resolve_dim(
        &style.margin.right,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let mar_bottom = resolve_dim(
        &style.margin.bottom,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let mar_left = resolve_dim(
        &style.margin.left,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    // box-sizing: content-box (default) — `width` is the content width
    // box-sizing: border-box — `width` includes padding + border
    // width: auto — fills container, subtract padding + border regardless
    let content_width = match (explicit_width, style.box_sizing) {
        (Some(w), BoxSizing::ContentBox) => w,
        (Some(w), BoxSizing::BorderBox) => {
            (w - pad_left - pad_right - border_left - border_right).max(0.0)
        }
        (None, _) => (width - pad_left - pad_right - border_left - border_right).max(0.0),
    };
    let content_x = offset_x + mar_left + border_left + pad_left;
    let content_y = offset_y + mar_top + border_top + pad_top;

    let gap_col = style
        .gap
        .width
        .resolve_px(
            content_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let gap_row = style
        .gap
        .height
        .resolve_px(
            content_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);

    // ── Resolve grid_template_areas into named line mappings ──
    let area_line_names = resolve_area_line_names(&style.grid_template_areas);

    // ── Dense auto-flow flag ──
    let dense = matches!(
        style.grid_auto_flow,
        GridAutoFlow::RowDense | GridAutoFlow::ColumnDense
    );

    // ── Implicit track sizes from style ──
    let implicit_col_size = resolve_track_px(&style.grid_auto_columns, content_width);
    let implicit_row_size =
        |available: f32| -> f32 { resolve_track_px(&style.grid_auto_rows, available) };

    // Resolve column track sizes (check for subgrid)
    let has_subgrid_cols = style
        .grid_template_columns
        .iter()
        .any(|t| matches!(t, TrackSize::Subgrid));
    let has_subgrid_rows = style
        .grid_template_rows
        .iter()
        .any(|t| matches!(t, TrackSize::Subgrid));

    // For subgrid, try to inherit tracks from the parent grid container
    let parent_col_tracks = if has_subgrid_cols {
        // Walk up the tree to find parent grid box to inherit column tracks
        tree.get(box_id)
            .and_then(|b| b.parent)
            .and_then(|pid| tree.get(pid))
            .map(|p| p.grid_col_tracks.clone())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let parent_row_tracks = if has_subgrid_rows {
        tree.get(box_id)
            .and_then(|b| b.parent)
            .and_then(|pid| tree.get(pid))
            .map(|p| p.grid_row_tracks.clone())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let col_tracks = if has_subgrid_cols && !parent_col_tracks.is_empty() {
        parent_col_tracks
    } else {
        // First expand any repeat() definitions
        let expanded_cols = expand_repeat_tracks(&style.grid_template_columns, content_width, gap_col);
        resolve_tracks(&expanded_cols, content_width, gap_col)
    };
    let num_cols = col_tracks.len().max(1);
    // Number of column lines = num_cols + 1 (lines are edges of tracks)
    let num_col_lines = num_cols + 1;
    
    // Calculate explicit row count for resolving negative row lines
    // The explicit grid is defined by grid-template-rows, grid-template-areas, etc.
    let explicit_row_count = if !style.grid_template_rows.is_empty() {
        style.grid_template_rows.len()
    } else if !style.grid_template_areas.is_empty() {
        style.grid_template_areas.len()
    } else {
        1 // Minimum 1 row
    };
    let num_row_lines = explicit_row_count + 1;

    // Collect children, resolve explicit placement
    let children = doc.children(node_id).to_vec();
    let mut placed_items: Vec<GridItem> = Vec::new();
    let mut auto_items: Vec<NodeId> = Vec::new();

    // Collect effective children, flattening display:contents nodes
    let mut effective_children: Vec<NodeId> = Vec::new();
    fn collect_grid_children(doc: &Document, children: &[NodeId], styles: &StyleMap, out: &mut Vec<NodeId>) {
        for &child_id in children {
            let child_style = styles.get(child_id).cloned().unwrap_or_default();
            if child_style.display == Display::None {
                continue;
            }
            if matches!(child_style.display, Display::Contents) {
                let grandchildren = doc.children(child_id).to_vec();
                collect_grid_children(doc, &grandchildren, styles, out);
                continue;
            }
            out.push(child_id);
        }
    }
    collect_grid_children(doc, &children, styles, &mut effective_children);

    for &child_id in &effective_children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        // Resolve explicit grid placement using the helper that handles negative lines
        let col_start = resolve_grid_line(&child_style.grid_column.start, num_col_lines, &area_line_names, "col", true);
        let col_end = match &child_style.grid_column.end {
            GridLine::Span(n) => col_start.map(|s| s + *n as usize),
            other => resolve_grid_line(other, num_col_lines, &area_line_names, "col", false),
        };
        let row_start = resolve_grid_line(&child_style.grid_row.start, num_row_lines, &area_line_names, "row", true);
        let row_end = match &child_style.grid_row.end {
            GridLine::Span(n) => row_start.map(|s| s + *n as usize),
            other => resolve_grid_line(other, num_row_lines, &area_line_names, "row", false),
        };

        if col_start.is_some() || row_start.is_some() {
            // Explicitly placed item
            let cs = col_start.unwrap_or(0);
            let ce = col_end.unwrap_or(cs + 1);
            let rs = row_start.unwrap_or(0);
            let re = row_end.unwrap_or(rs + 1);
            placed_items.push(GridItem {
                node_id: child_id,
                col_start: cs,
                col_end: ce,
                row_start: rs,
                row_end: re,
                box_id: None,
            });
        } else {
            auto_items.push(child_id);
        }
    }

    // Build an occupancy grid for auto-placement
    let _total_items = placed_items.len() + auto_items.len();
    let max_explicit_row = placed_items.iter().map(|it| it.row_end).max().unwrap_or(0);
    let min_auto_rows = (auto_items.len() + num_cols - 1) / num_cols;
    let mut num_rows = max_explicit_row.max(min_auto_rows).max(1);

    // Occupied cells tracker
    let mut occupied = vec![vec![false; num_cols]; num_rows];
    for item in &placed_items {
        for r in item.row_start..item.row_end.min(num_rows) {
            for c in item.col_start..item.col_end.min(num_cols) {
                if r < occupied.len() && c < num_cols {
                    occupied[r][c] = true;
                }
            }
        }
    }

    // Auto-place remaining items into unoccupied cells.
    // grid-auto-flow: column/column-dense iterates columns-first (down then across).
    let column_flow = matches!(
        style.grid_auto_flow,
        GridAutoFlow::Column | GridAutoFlow::ColumnDense
    );
    let mut auto_cursor_row: usize = 0;
    let mut auto_cursor_col: usize = 0;
    for child_id in auto_items {
        // Dense packing: reset cursor to (0,0) to backtrack and fill gaps
        if dense {
            auto_cursor_row = 0;
            auto_cursor_col = 0;
        }

        // Find next unoccupied cell
        loop {
            if column_flow {
                if auto_cursor_row >= num_rows {
                    // Extend to next column
                    auto_cursor_row = 0;
                    auto_cursor_col += 1;
                }
            } else if auto_cursor_row >= num_rows {
                // Extend the grid
                num_rows += 1;
                occupied.push(vec![false; num_cols]);
            }
            // Ensure occupied grid is large enough
            while auto_cursor_row >= occupied.len() {
                num_rows += 1;
                occupied.push(vec![false; num_cols]);
            }
            if auto_cursor_col < num_cols && !occupied[auto_cursor_row][auto_cursor_col] {
                break;
            }
            if column_flow {
                auto_cursor_row += 1;
            } else {
                auto_cursor_col += 1;
                if auto_cursor_col >= num_cols {
                    auto_cursor_col = 0;
                    auto_cursor_row += 1;
                }
            }
        }
        occupied[auto_cursor_row][auto_cursor_col] = true;
        placed_items.push(GridItem {
            node_id: child_id,
            col_start: auto_cursor_col,
            col_end: auto_cursor_col + 1,
            row_start: auto_cursor_row,
            row_end: auto_cursor_row + 1,
            box_id: None,
        });
        if column_flow {
            auto_cursor_row += 1;
        } else {
            auto_cursor_col += 1;
            if auto_cursor_col >= num_cols {
                auto_cursor_col = 0;
                auto_cursor_row += 1;
            }
        }
    }

    // Recalculate num_rows from all items
    num_rows = placed_items
        .iter()
        .map(|it| it.row_end)
        .max()
        .unwrap_or(1)
        .max(1);

    let available_h_for_rows = style
        .height
        .resolve_px(
            container_height,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(container_height);
    let implicit_row_px = implicit_row_size(available_h_for_rows);

    let row_tracks = if has_subgrid_rows && !parent_row_tracks.is_empty() {
        let mut rt = parent_row_tracks;
        while rt.len() < num_rows {
            rt.push(implicit_row_px);
        }
        rt
    } else if style.grid_template_rows.is_empty() {
        // Use grid-auto-rows for all implicit rows (instead of 0)
        vec![implicit_row_px; num_rows]
    } else {
        // First expand any repeat() definitions
        let expanded_rows = expand_repeat_tracks(&style.grid_template_rows, available_h_for_rows, gap_row);
        let mut rt = resolve_tracks(&expanded_rows, available_h_for_rows, gap_row);
        // Extend with implicit row size (grid-auto-rows) for tracks beyond the template
        while rt.len() < num_rows {
            rt.push(implicit_row_px);
        }
        rt
    };

    // Layout each item into its cell (spanning support)
    let mut row_heights: Vec<f32> = vec![0.0; num_rows];

    for item in &mut placed_items {
        // Calculate spanned cell width: sum of columns [col_start..col_end] + gaps
        let span_cols = item.col_end.saturating_sub(item.col_start).max(1);
        let mut cell_width = 0.0f32;
        let fallback_col = if implicit_col_size > 0.0 {
            implicit_col_size
        } else {
            content_width / num_cols as f32
        };
        for c in item.col_start..item.col_end.min(num_cols) {
            cell_width += if c < col_tracks.len() {
                col_tracks[c]
            } else {
                fallback_col
            };
        }
        // Add inter-column gaps within the span
        if span_cols > 1 {
            cell_width += (span_cols - 1) as f32 * gap_col;
        }

        // Layout child in cell (dispatch to correct layout mode)
        let child_style = styles.get(item.node_id).cloned().unwrap_or_default();

        // Handle text node children — measure them directly instead of
        // delegating to layout_block (which would create a 0×0 box).
        let child_box = if let Some(child_node) = doc.get(item.node_id) {
            if child_node.is_text() {
                if let Some(text) = child_node.text_content() {
                    let text_props = crate::TextProperties::from_style(&child_style);
                    let metrics = text_measurer.measure(
                        text,
                        child_style.font_size,
                        &child_style.font_family,
                        child_style.font_weight,
                        Some(cell_width),
                        &text_props,
                    );
                    let text_box = tree.alloc(
                        item.node_id,
                        BoxType::Text {
                            line_boxes: Vec::new(),
                        },
                    );
                    if let Some(tb) = tree.get_mut(text_box) {
                        tb.content_rect = Rect::new(0.0, 0.0, metrics.width, metrics.height);
                        tb.padding_rect = tb.content_rect;
                        tb.border_rect = tb.content_rect;
                        tb.margin_rect = tb.content_rect;
                        tb.baseline = Some(metrics.baseline);
                    }
                    tree.add_child(box_id, text_box);
                    text_box
                } else {
                    // Empty text node — still needs a box
                    let text_box = tree.alloc(item.node_id, BoxType::Text { line_boxes: Vec::new() });
                    tree.add_child(box_id, text_box);
                    text_box
                }
            } else if child_style.is_flex_container() {
            crate::flex::layout_flex(
                doc,
                item.node_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                cell_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        } else if child_style.is_grid_container() {
            crate::grid::layout_grid(
                doc,
                item.node_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                cell_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        } else {
            crate::block::layout_block(
                doc,
                item.node_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                cell_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        }
        } else {
            // Node not found — create a placeholder
            crate::block::layout_block(
                doc,
                item.node_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                cell_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        };

        if let Some(cb) = tree.get(child_box) {
            // Distribute height across spanned rows (use max for first row)
            let span_rows = item.row_end.saturating_sub(item.row_start).max(1);
            let child_h = cb.margin_rect.height;
            if span_rows == 1 {
                if item.row_start < num_rows {
                    row_heights[item.row_start] = row_heights[item.row_start].max(child_h);
                }
            } else {
                // Spread height evenly across spanned rows
                let per_row = child_h / span_rows as f32;
                for r in item.row_start..item.row_end.min(num_rows) {
                    row_heights[r] = row_heights[r].max(per_row);
                }
            }
        }

        item.box_id = Some(child_box);
        tree.add_child(box_id, child_box);
    }

    // Use explicit row heights where provided
    for (i, h) in row_tracks.iter().enumerate() {
        if i < row_heights.len() && *h > 0.0 {
            row_heights[i] = *h;
        }
    }

    // Position items
    // First compute raw track offsets, then apply justify-content / align-content.
    let fallback_col_pos = if implicit_col_size > 0.0 {
        implicit_col_size
    } else {
        content_width / num_cols.max(1) as f32
    };

    // ── justify-content: distribute horizontal free space between column tracks ──
    let total_col_width: f32 = (0..num_cols)
        .map(|c| if c < col_tracks.len() { col_tracks[c] } else { fallback_col_pos })
        .sum::<f32>()
        + if num_cols > 1 { (num_cols - 1) as f32 * gap_col } else { 0.0 };
    let free_x = (content_width - total_col_width).max(0.0);
    let (jc_start, jc_extra) = grid_content_distribute(style.justify_content, free_x, num_cols);

    let mut x_offsets: Vec<f32> = vec![0.0; num_cols];
    let mut cumulative_x = jc_start;
    for col in 0..num_cols {
        x_offsets[col] = cumulative_x;
        let cw = if col < col_tracks.len() {
            col_tracks[col]
        } else {
            fallback_col_pos
        };
        cumulative_x += cw + gap_col + if col < num_cols - 1 { jc_extra } else { 0.0 };
    }

    // ── align-content: distribute vertical free space between row tracks ──
    let explicit_container_h = style.height.resolve_px(
        container_height, base_font_size, font_size, viewport_w, viewport_h,
    );
    let total_row_height: f32 = row_heights.iter().sum::<f32>()
        + if num_rows > 1 { (num_rows - 1) as f32 * gap_row } else { 0.0 };
    let container_h_for_align = explicit_container_h.unwrap_or(total_row_height);
    let free_y = (container_h_for_align - total_row_height).max(0.0);
    let (ac_start, ac_extra) = grid_content_distribute_align(style.align_content, free_y, num_rows);

    let mut y_offsets: Vec<f32> = vec![0.0; num_rows];
    let mut cumulative_y = ac_start;
    for row in 0..num_rows {
        y_offsets[row] = cumulative_y;
        cumulative_y += row_heights[row] + gap_row + if row < num_rows - 1 { ac_extra } else { 0.0 };
    }

    // Update child positions using placed_items (supports spanning)
    for item in &placed_items {
        let child_box_id = match item.box_id {
            Some(id) => id,
            None => continue,
        };

        // Cell position is LOCAL to the grid container's content area
        let cell_x = x_offsets.get(item.col_start).copied().unwrap_or(0.0);
        let cell_y = y_offsets.get(item.row_start).copied().unwrap_or(0.0);

        // Width spans multiple columns + inter-column gaps
        let span_cols = item.col_end.saturating_sub(item.col_start).max(1);
        let mut cell_w = 0.0f32;
        for c in item.col_start..item.col_end.min(num_cols) {
            cell_w += if c < col_tracks.len() {
                col_tracks[c]
            } else {
                fallback_col_pos
            };
        }
        if span_cols > 1 {
            cell_w += (span_cols - 1) as f32 * gap_col;
        }

        // Height spans multiple rows + inter-row gaps
        let span_rows = item.row_end.saturating_sub(item.row_start).max(1);
        let mut cell_h = 0.0f32;
        for r in item.row_start..item.row_end.min(num_rows) {
            cell_h += row_heights[r];
        }
        if span_rows > 1 {
            cell_h += (span_rows - 1) as f32 * gap_row;
        }

        if let Some(b) = tree.get_mut(child_box_id) {
            // Apply justify-items / justify-self alignment within the grid cell (horizontal)
            let child_style = styles.get(item.node_id).cloned().unwrap_or_default();
            let child_w = b.margin_rect.width.min(cell_w);
            let alignment = match child_style.justify_self {
                JustifySelf::Auto | JustifySelf::Normal | JustifySelf::Stretch => style.justify_items,
                JustifySelf::Center => JustifyItems::Center,
                JustifySelf::Start | JustifySelf::SelfStart | JustifySelf::FlexStart => JustifyItems::Start,
                JustifySelf::End | JustifySelf::SelfEnd | JustifySelf::FlexEnd => JustifyItems::End,
            };
            let x_offset = match alignment {
                JustifyItems::Center => (cell_w - child_w) / 2.0,
                JustifyItems::End | JustifyItems::FlexEnd | JustifyItems::SelfEnd | JustifyItems::Right => cell_w - child_w,
                _ => 0.0, // Start, Stretch, Normal
            };

            // Apply align-items / align-self alignment within the grid cell (vertical)
            let child_h = b.margin_rect.height.min(cell_h);
            let v_alignment = match child_style.align_self {
                AlignSelf::Auto => style.align_items,
                AlignSelf::Stretch => AlignItems::Stretch,
                AlignSelf::Center => AlignItems::Center,
                AlignSelf::FlexStart => AlignItems::FlexStart,
                AlignSelf::FlexEnd => AlignItems::FlexEnd,
                AlignSelf::Baseline => AlignItems::Baseline,
            };
            let y_offset = match v_alignment {
                AlignItems::Center => (cell_h - child_h) / 2.0,
                AlignItems::FlexEnd => cell_h - child_h,
                _ => 0.0, // FlexStart, Stretch, Baseline
            };

            // Reposition the child box to its cell position.
            // Use delta-based shifting to preserve the child's own
            // padding/border/margin box geometry from its layout pass.
            let dx = cell_x + x_offset - b.margin_rect.x;
            let dy = cell_y + y_offset - b.margin_rect.y;
            b.content_rect.x += dx;
            b.content_rect.y += dy;
            b.padding_rect.x += dx;
            b.padding_rect.y += dy;
            b.border_rect.x += dx;
            b.border_rect.y += dy;
            b.margin_rect.x += dx;
            b.margin_rect.y += dy;
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
        // Store resolved grid tracks so subgrid children can inherit them
        b.grid_col_tracks = col_tracks.clone();
        b.grid_row_tracks = row_heights.clone();
    }

    box_id
}

/// Resolve track sizes, handling fr units and fixed sizes.
fn resolve_tracks(tracks: &[TrackSize], available: f32, gap: f32) -> Vec<f32> {
    if tracks.is_empty() {
        return Vec::new();
    }

    let total_gap = if tracks.len() > 1 {
        (tracks.len() - 1) as f32 * gap
    } else {
        0.0
    };
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
            TrackSize::MinMax(min, max) => {
                // Use min size initially; will expand toward max during distribution
                let min_px = resolve_track_px(min, available);
                let _max_px = resolve_track_px(max, available);
                sizes[i] = min_px;
                fixed_total += min_px;
                // We'll handle expansion below if there's remaining space
            }
            TrackSize::FitContent(max_percent) => {
                // Acts like minmax(auto, max_percent%)
                let max_px = available * max_percent / 100.0;
                let track_count = tracks.len().max(1) as f32;
                sizes[i] = max_px.min(available / track_count);
                fixed_total += sizes[i];
            }
            TrackSize::MinContent => {
                // MinContent gets a smaller share of remaining space
                fr_total += 0.5;
            }
            TrackSize::MaxContent => {
                // MaxContent gets a larger share of remaining space
                fr_total += 1.5;
            }
            TrackSize::Auto => {
                // Auto behaves like minmax(min-content, max-content) — default weight
                fr_total += 1.0;
            }
            TrackSize::Subgrid => {
                // Subgrid inherits parent grid tracks — treat as auto for now
                fr_total += 1.0;
            }
            TrackSize::Repeat { .. } => {
                // Repeat tracks should have been expanded before reaching here
                // Treat as auto fallback
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
                TrackSize::MinContent => {
                    sizes[i] = remaining * (0.5 / fr_total);
                }
                TrackSize::MaxContent => {
                    sizes[i] = remaining * (1.5 / fr_total);
                }
                TrackSize::Auto => {
                    sizes[i] = remaining * (1.0 / fr_total);
                }
                TrackSize::MinMax(_min, max) => {
                    // Expand toward max if there's remaining space
                    let max_px = resolve_track_px(max, available);
                    let grow = (max_px - sizes[i])
                        .max(0.0)
                        .min(remaining * (1.0 / fr_total));
                    sizes[i] += grow;
                }
                TrackSize::Px(_) | TrackSize::Percent(_) | TrackSize::FitContent(_) => {} // already set
                TrackSize::Subgrid | TrackSize::Repeat { .. } => {
                    // Subgrid/Repeat: treat like auto for distribution
                    sizes[i] = remaining * (1.0 / fr_total);
                }
            }
        }
    }

    sizes
}

/// Resolve a single track size value to pixels.
fn resolve_track_px(track: &TrackSize, available: f32) -> f32 {
    match track {
        TrackSize::Px(v) => *v,
        TrackSize::Percent(v) => available * v / 100.0,
        TrackSize::Fr(_) => 0.0,
        TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent | TrackSize::Subgrid => 0.0,
        TrackSize::MinMax(min, _) => resolve_track_px(min, available),
        TrackSize::FitContent(pct) => available * pct / 100.0,
        TrackSize::Repeat { .. } => 0.0, // Should be expanded before here
    }
}

/// Expand repeat() track definitions into concrete tracks.
///
/// - `repeat(N, track)` → N copies of track
/// - `repeat(auto-fill, track)` → as many tracks as fit in available space
/// - `repeat(auto-fit, track)` → same as auto-fill, but empty tracks collapse
///
/// Note: auto-fit collapsing is handled during layout, not here.
fn expand_repeat_tracks(tracks: &[TrackSize], available: f32, gap: f32) -> Vec<TrackSize> {
    use liquide_style_engine::computed::RepeatMode;
    
    let mut expanded = Vec::new();
    
    for track in tracks {
        match track {
            TrackSize::Repeat { mode, tracks: inner } => {
                match mode {
                    RepeatMode::Count(n) => {
                        // Fixed count: repeat N times
                        for _ in 0..*n {
                            expanded.extend(inner.clone());
                        }
                    }
                    RepeatMode::AutoFill | RepeatMode::AutoFit => {
                        // Calculate how many repetitions fit
                        // First, calculate the minimum size of one repetition
                        let rep_size: f32 = inner.iter()
                            .map(|t| min_track_size(t, available))
                            .sum();
                        
                        if rep_size > 0.0 {
                            // Calculate how many can fit
                            // Available = (count * rep_size) + ((count - 1) * gap)
                            // Solve for count: count = (available + gap) / (rep_size + gap)
                            let count = ((available + gap) / (rep_size + gap)).floor() as u32;
                            let count = count.max(1); // At least one
                            
                            for _ in 0..count {
                                expanded.extend(inner.clone());
                            }
                        } else {
                            // Fallback: at least one instance
                            expanded.extend(inner.clone());
                        }
                    }
                }
            }
            other => {
                expanded.push(other.clone());
            }
        }
    }
    
    expanded
}

/// Calculate the minimum intrinsic size of a track.
fn min_track_size(track: &TrackSize, available: f32) -> f32 {
    match track {
        TrackSize::Px(v) => *v,
        TrackSize::Percent(v) => available * v / 100.0,
        TrackSize::Fr(_) => 0.0, // fr tracks have no minimum
        TrackSize::MinMax(min, _) => min_track_size(min, available),
        TrackSize::FitContent(pct) => available * pct / 100.0,
        TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent | TrackSize::Subgrid => 0.0,
        TrackSize::Repeat { .. } => 0.0, // Nested repeat not supported
    }
}

/// Resolve `grid_template_areas` rows into named line mappings.
///
/// Each row in `grid_template_areas` is a space-separated string of area names.
/// This produces a mapping from "{name}-start" / "{name}-end" to line indices
/// for both row and column axes, which can be used to resolve named grid lines.
fn resolve_area_line_names(areas: &[String]) -> HashMap<String, usize> {
    let mut names: HashMap<String, usize> = HashMap::new();
    if areas.is_empty() {
        return names;
    }

    for (row_idx, row_str) in areas.iter().enumerate() {
        let tokens: Vec<&str> = row_str.split_whitespace().collect();
        for (col_idx, token) in tokens.iter().enumerate() {
            if *token == "." {
                continue; // null cell token
            }
            let start_row_key = format!("{}-start-row", token);
            let end_row_key = format!("{}-end-row", token);
            let start_col_key = format!("{}-start-col", token);
            let end_col_key = format!("{}-end-col", token);

            // Row lines: first occurrence sets start, every occurrence updates end
            names.entry(start_row_key).or_insert(row_idx);
            names.insert(end_row_key, row_idx + 1);

            // Column lines
            names.entry(start_col_key).or_insert(col_idx);
            names.insert(end_col_key, col_idx + 1);
        }
    }
    names
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

/// Resolve a GridLine to a zero-based line index.
///
/// CSS grid lines are 1-indexed:
/// - Positive lines (1, 2, 3...) count from the start
/// - Negative lines (-1, -2, -3...) count from the end
///
/// `num_lines` is the total number of lines (= num_tracks + 1).
/// `area_names` maps names like "header-start-row" → 0 for named grid area lookup.
fn resolve_grid_line(
    line: &GridLine,
    num_lines: usize,
    area_names: &std::collections::HashMap<String, usize>,
    axis: &str,
    is_start: bool,
) -> Option<usize> {
    match line {
        GridLine::Line(n) => {
            if *n > 0 {
                // Positive: 1 => index 0, 2 => index 1, etc.
                Some((*n - 1) as usize)
            } else if *n < 0 {
                // Negative: -1 => last line, -2 => second-to-last, etc.
                // -1 => num_lines - 1, -2 => num_lines - 2
                let abs_n = (-*n) as usize;
                if abs_n <= num_lines {
                    Some(num_lines - abs_n)
                } else {
                    Some(0) // Clamped to first line
                }
            } else {
                // Line 0 is invalid in CSS Grid, treat as auto
                None
            }
        }
        GridLine::Named(name) => {
            // Look up "name-start-row", "name-end-col", etc. from grid-template-areas
            let suffix = if is_start { "start" } else { "end" };
            let key = format!("{}-{}-{}", name, suffix, axis);
            area_names.get(&key).copied()
        }
        GridLine::Span(_) | GridLine::Auto => None,
    }
}

/// Distribute free space between grid tracks for `justify-content`.
///
/// CSS Grid §10.5: The justify-content property aligns the grid within the
/// grid container along the inline axis when the total grid size is less
/// than the container size.
fn grid_content_distribute(jc: JustifyContent, free: f32, count: usize) -> (f32, f32) {
    if count == 0 || free <= 0.0 {
        return (0.0, 0.0);
    }
    match jc {
        JustifyContent::FlexStart => (0.0, 0.0),
        JustifyContent::FlexEnd => (free, 0.0),
        JustifyContent::Center => (free / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if count > 1 {
                (0.0, free / (count - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        JustifyContent::SpaceAround => {
            let s = free / count as f32;
            (s / 2.0, s)
        }
        JustifyContent::SpaceEvenly => {
            let s = free / (count + 1) as f32;
            (s, s)
        }
    }
}

/// Distribute free space between grid tracks for `align-content`.
fn grid_content_distribute_align(ac: AlignContent, free: f32, count: usize) -> (f32, f32) {
    if count == 0 || free <= 0.0 {
        return (0.0, 0.0);
    }
    match ac {
        AlignContent::FlexStart => (0.0, 0.0),
        AlignContent::FlexEnd => (free, 0.0),
        AlignContent::Center => (free / 2.0, 0.0),
        AlignContent::SpaceBetween => {
            if count > 1 {
                (0.0, free / (count - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        AlignContent::SpaceAround => {
            let s = free / count as f32;
            (s / 2.0, s)
        }
        AlignContent::Stretch => (0.0, free / count as f32),
    }
}
