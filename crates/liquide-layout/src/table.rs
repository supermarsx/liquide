//! Table layout — CSS table formatting context with colspan/rowspan support.
//!
//! Implements table layout per CSS 2.1 §17:
//! 1. Identify table-caption, table-row, and table-cell children
//! 2. Build an occupancy grid to handle colspan/rowspan spanning
//! 3. Calculate column widths (content-based with span distribution)
//! 4. Position cells in a grid pattern with proper row/column spans
//!
//! This handles `display: table` containers with `display: table-row`,
//! `display: table-cell`, and `display: table-caption` children. Also
//! works with `<table>`, `<tr>`, `<td>`, `<caption>` elements by tag
//! name as a fallback.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{Display, Position, VerticalAlign};

/// Classification of a table child for row-group ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TableGroupKind {
    Header,
    Body,
    Footer,
}

/// Classify a table child as header, body, or footer group.
fn classify_table_group(doc: &Document, node_id: NodeId, style: &liquide_style_engine::computed::ComputedStyle) -> TableGroupKind {
    match style.display {
        Display::TableHeaderGroup => return TableGroupKind::Header,
        Display::TableFooterGroup => return TableGroupKind::Footer,
        Display::TableRowGroup => return TableGroupKind::Body,
        _ => {}
    }
    if let Some(node) = doc.get(node_id) {
        match node.tag_name().as_str() {
            "thead" => return TableGroupKind::Header,
            "tfoot" => return TableGroupKind::Footer,
            "tbody" => return TableGroupKind::Body,
            _ => {}
        }
    }
    TableGroupKind::Body
}

/// Check if a child is a table row group (thead/tbody/tfoot).
fn is_table_row_group(doc: &Document, node_id: NodeId, style: &liquide_style_engine::computed::ComputedStyle) -> bool {
    matches!(
        style.display,
        Display::TableHeaderGroup | Display::TableFooterGroup | Display::TableRowGroup
    ) || doc
        .get(node_id)
        .map(|n| {
            let tag = n.tag_name();
            tag == "thead" || tag == "tbody" || tag == "tfoot"
        })
        .unwrap_or(false)
}
use liquide_style_engine::dimension::Dimension;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};

/// A resolved table row.
struct TableRow {
    _node_id: NodeId,
    cells: Vec<TableCell>,
}

/// A resolved table cell with colspan/rowspan support.
struct TableCell {
    _node_id: NodeId,
    box_id: LayoutBoxId,
    intrinsic_width: f32,
    intrinsic_height: f32,
    colspan: usize,
    rowspan: usize,
}

/// A caption block laid out above the table rows.
struct TableCaption {
    box_id: LayoutBoxId,
    height: f32,
}

/// Entry in the occupancy grid pointing back to the cell that owns the slot.
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct GridSlot {
    /// Row index of the originating cell.
    origin_row: usize,
    /// Column index of the originating cell (within the row's `cells` vec).
    origin_cell: usize,
}

/// Read a positive integer attribute (`colspan` / `rowspan`) from the DOM,
/// falling back to 1 when absent or unparseable.
fn read_span_attr(doc: &Document, node_id: NodeId, attr: &str) -> usize {
    doc.get_attribute(node_id, attr)
        .and_then(|v| v.parse::<usize>().ok())
        .map(|v| v.max(1))
        .unwrap_or(1)
}

/// Perform table layout.
///
/// Lays out a table container by:
/// 1. Laying out any `<caption>` / `display: table-caption` children as
///    blocks above the grid area
/// 2. Collecting rows and cells from child elements, reading colspan/rowspan
/// 3. Building an occupancy grid for spanning cells
/// 4. Computing column widths (distributing spanning-cell excess equally)
/// 5. Positioning cells using spanned widths/heights
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
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(container_width);

    // Resolve padding
    let pad_top = rdim(
        &style.padding.top,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_right = rdim(
        &style.padding.right,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_bottom = rdim(
        &style.padding.bottom,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_left = rdim(
        &style.padding.left,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );

    // Resolve margins
    let mar_top = rdim(
        &style.margin.top,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let mar_right = rdim(
        &style.margin.right,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let mar_bottom = rdim(
        &style.margin.bottom,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let mar_left = rdim(
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

    let content_width = (width - pad_left - pad_right - border_left - border_right).max(0.0);
    let content_x = offset_x + mar_left + border_left + pad_left;
    let content_y = offset_y + mar_top + border_top + pad_top;

    let border_spacing = style.border_spacing;

    // Table-specific CSS properties:
    // border-collapse: separate (default) uses border-spacing between cells;
    // border-collapse: collapse merges adjacent borders (not yet rendered collapsed).
    let use_collapsed = style.border_collapse == liquide_style_engine::computed::BorderCollapse::Collapse;
    let effective_spacing = if use_collapsed { 0.0 } else { border_spacing };

    // caption-side: top (default) or bottom — controls caption placement.
    let caption_bottom = style.caption_side == liquide_style_engine::computed::CaptionSide::Bottom;

    // empty-cells: show (default) or hide — hides borders/bg of empty cells.
    let _hide_empty = style.empty_cells == liquide_style_engine::computed::EmptyCells::Hide;

    // table-layout: auto (default) or fixed — fixed uses first-row widths only.
    let table_layout_fixed = style.table_layout == liquide_style_engine::computed::TableLayout::Fixed;

    // ── Step 0: Layout captions ──
    let children = doc.children(node_id).to_vec();
    let mut captions: Vec<TableCaption> = Vec::new();

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        let is_caption = child_style.display == Display::TableCaption
            || doc
                .get(child_id)
                .map(|n| n.tag_name() == "caption")
                .unwrap_or(false);

        if is_caption {
            let cap_box = crate::block::layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                content_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            );
            let cap_h = tree
                .get(cap_box)
                .map(|b| b.margin_rect.height)
                .unwrap_or(0.0);
            captions.push(TableCaption {
                box_id: cap_box,
                height: cap_h,
            });
            tree.add_child(box_id, cap_box);
        }
    }

    // Position captions above or below the grid depending on caption-side.
    // Captions use LOCAL coordinates relative to the table's content area.
    // When caption_bottom is true, we defer positioning until after grid layout.
    let mut caption_y = 0.0f32;
    if !caption_bottom {
        for cap in &captions {
            if let Some(b) = tree.get_mut(cap.box_id) {
                let dx = 0.0 - b.content_rect.x;
                let dy = caption_y - b.content_rect.y;
                b.content_rect.x += dx;
                b.content_rect.y += dy;
                b.padding_rect.x += dx;
                b.padding_rect.y += dy;
                b.border_rect.x += dx;
                b.border_rect.y += dy;
                b.margin_rect.x += dx;
                b.margin_rect.y += dy;
            }
            caption_y += cap.height;
        }
    }

    // ── Step 1: Collect rows and cells ──
    //
    // Sort children so that header groups come first, then row groups (body),
    // then footer groups — per CSS 2.1 §17.  Row group elements (thead, tbody,
    // tfoot / display: table-header-group, table-row-group, table-footer-group)
    // are treated as passthrough containers: their children are collected as rows.
    let mut rows: Vec<TableRow> = Vec::new();

    // Build an ordered list of (group_kind, child_id) so we can sort by kind
    // while preserving relative order within each kind.
    let mut grouped_children: Vec<(TableGroupKind, NodeId)> = Vec::new();
    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }
        let is_caption = child_style.display == Display::TableCaption
            || doc
                .get(child_id)
                .map(|n| n.tag_name() == "caption")
                .unwrap_or(false);
        if is_caption {
            continue;
        }
        let kind = classify_table_group(doc, child_id, &child_style);
        grouped_children.push((kind, child_id));
    }
    // Stable sort: headers first, then body, then footers
    grouped_children.sort_by_key(|(kind, _)| *kind);

    /// Helper: collect cells from a row element and push a TableRow.
    #[allow(clippy::too_many_arguments)]
    fn collect_row(
        doc: &Document,
        row_id: NodeId,
        styles: &StyleMap,
        tree: &mut LayoutTree,
        text_measurer: &dyn TextMeasurer,
        image_measurer: &dyn ImageMeasurer,
        content_width: f32,
        container_height: f32,
        viewport_w: f32,
        viewport_h: f32,
        base_font_size: f32,
        parent_box_id: LayoutBoxId,
        rows: &mut Vec<TableRow>,
    ) {
        let row_children = doc.children(row_id).to_vec();
        let mut cells = Vec::new();
        for &cell_id in &row_children {
            let cell_style = styles.get(cell_id).cloned().unwrap_or_default();
            if cell_style.display == Display::None {
                continue;
            }

            let colspan = read_span_attr(doc, cell_id, "colspan");
            let rowspan = read_span_attr(doc, cell_id, "rowspan");

            let cell_box = crate::block::layout_block(
                doc,
                cell_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                content_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
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
                colspan,
                rowspan,
            });

            tree.add_child(parent_box_id, cell_box);
        }
        rows.push(TableRow {
            _node_id: row_id,
            cells,
        });
    }

    for &(_, child_id) in &grouped_children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();

        // If this child is a row group (thead/tbody/tfoot), iterate its children as rows
        if is_table_row_group(doc, child_id, &child_style) {
            let group_children = doc.children(child_id).to_vec();
            for &gc_id in &group_children {
                let gc_style = styles.get(gc_id).cloned().unwrap_or_default();
                if gc_style.display == Display::None {
                    continue;
                }
                let gc_is_row = gc_style.display == Display::TableRow
                    || doc
                        .get(gc_id)
                        .map(|n| n.tag_name() == "tr")
                        .unwrap_or(false);
                if gc_is_row {
                    collect_row(
                        doc, gc_id, styles, tree, text_measurer, image_measurer,
                        content_width, container_height, viewport_w, viewport_h,
                        base_font_size, box_id, &mut rows,
                    );
                }
                // Non-row children inside a group are ignored (per spec)
            }
            continue;
        }

        let is_row = child_style.display == Display::TableRow
            || doc
                .get(child_id)
                .map(|n| n.tag_name() == "tr")
                .unwrap_or(false);

        if is_row {
            collect_row(
                doc, child_id, styles, tree, text_measurer, image_measurer,
                content_width, container_height, viewport_w, viewport_h,
                base_font_size, box_id, &mut rows,
            );
        } else {
            // Non-row child: treat as a single-cell row (anonymous table row)
            let cell_box = crate::block::layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                content_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
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
                    colspan: 1,
                    rowspan: 1,
                }],
            });

            tree.add_child(box_id, cell_box);
        }
    }

    // ── Step 1b: Build occupancy grid ──
    // First pass: determine grid dimensions by simulating cell placement.
    let num_rows = rows.len();
    // Upper-bound column count: sum of colspans in the widest row, grown by
    // rowspan reservations.
    let mut num_cols: usize = 0;
    {
        // Temporary occupancy just for sizing. `true` = occupied.
        let mut tmp_occ: Vec<Vec<bool>> = vec![vec![false; 0]; num_rows];
        for (ri, row) in rows.iter().enumerate() {
            let mut col = 0usize;
            for cell in &row.cells {
                // Skip occupied slots (from earlier rowspans)
                while col < tmp_occ[ri].len() && tmp_occ[ri][col] {
                    col += 1;
                }
                let end_col = col + cell.colspan;
                // Ensure all affected rows are wide enough.
                for dr in 0..cell.rowspan {
                    let r = ri + dr;
                    if r < num_rows {
                        if tmp_occ[r].len() < end_col {
                            tmp_occ[r].resize(end_col, false);
                        }
                        for c in col..end_col {
                            tmp_occ[r][c] = true;
                        }
                    }
                }
                if end_col > num_cols {
                    num_cols = end_col;
                }
                col = end_col;
            }
            if col > num_cols {
                num_cols = col;
            }
        }
    }

    if num_cols == 0 {
        // Empty table (may still have captions).
        let total_h = caption_y;
        set_table_geometry(
            tree,
            box_id,
            content_x,
            content_y,
            content_width,
            total_h,
            pad_top,
            pad_right,
            pad_bottom,
            pad_left,
            border_top,
            border_right,
            border_bottom,
            border_left,
            mar_top,
            mar_right,
            mar_bottom,
            mar_left,
        );
        return box_id;
    }

    // Now build the real occupancy grid with slot info.
    let mut grid: Vec<Vec<Option<GridSlot>>> = vec![vec![None; num_cols]; num_rows];
    // `cell_grid_col[row_index][cell_index_in_row]` = starting grid column
    let mut cell_grid_col: Vec<Vec<usize>> = Vec::with_capacity(num_rows);

    for (ri, row) in rows.iter().enumerate() {
        let mut col = 0usize;
        let mut col_indices = Vec::with_capacity(row.cells.len());
        for (ci, cell) in row.cells.iter().enumerate() {
            // Skip occupied slots
            while col < num_cols && grid[ri][col].is_some() {
                col += 1;
            }
            col_indices.push(col);
            let end_col = (col + cell.colspan).min(num_cols);
            for dr in 0..cell.rowspan {
                let r = ri + dr;
                if r < num_rows {
                    for c in col..end_col {
                        grid[r][c] = Some(GridSlot {
                            origin_row: ri,
                            origin_cell: ci,
                        });
                    }
                }
            }
            col = end_col;
        }
        cell_grid_col.push(col_indices);
    }

    // ── Step 2: Determine column widths ──
    // table-layout: fixed — use first row only, ignore intrinsic content widths.
    // This is faster and more predictable (CSS 2.1 §17.5.2.1).
    let mut col_max_widths = vec![0.0f32; num_cols];
    if table_layout_fixed && !rows.is_empty() {
        // Fixed layout: only the first row determines column widths
        let first_row = &rows[0];
        for (ci, cell) in first_row.cells.iter().enumerate() {
            if cell.colspan == 1 {
                let gc = cell_grid_col[0][ci];
                if gc < num_cols {
                    // Use explicit width from style if available, else intrinsic
                    let cell_style = styles.get(cell._node_id).cloned().unwrap_or_default();
                    let explicit_w = cell_style.width.resolve_px(
                        content_width, base_font_size, font_size, viewport_w, viewport_h,
                    );
                    col_max_widths[gc] = explicit_w.unwrap_or(cell.intrinsic_width);
                }
            }
        }
        // Spanning cells in first row
        for (ci, cell) in first_row.cells.iter().enumerate() {
            if cell.colspan > 1 {
                let gc = cell_grid_col[0][ci];
                let end_col = (gc + cell.colspan).min(num_cols);
                let span = end_col - gc;
                if span == 0 { continue; }
                let current_sum: f32 = col_max_widths[gc..end_col].iter().sum();
                let needed = cell.intrinsic_width;
                if needed > current_sum {
                    let extra_per_col = (needed - current_sum) / span as f32;
                    for c in gc..end_col {
                        col_max_widths[c] += extra_per_col;
                    }
                }
            }
        }
    } else {
        // Auto layout: use all rows' intrinsic widths
        for (ri, row) in rows.iter().enumerate() {
            for (ci, cell) in row.cells.iter().enumerate() {
                if cell.colspan == 1 {
                    let gc = cell_grid_col[ri][ci];
                    if gc < num_cols {
                        col_max_widths[gc] = col_max_widths[gc].max(cell.intrinsic_width);
                    }
                }
            }
        }

        // Distribute spanning cells' excess width equally among spanned columns.
        for (ri, row) in rows.iter().enumerate() {
            for (ci, cell) in row.cells.iter().enumerate() {
                if cell.colspan > 1 {
                    let gc = cell_grid_col[ri][ci];
                    let end_col = (gc + cell.colspan).min(num_cols);
                    let span = end_col - gc;
                    if span == 0 {
                        continue;
                    }
                    let spanned_spacing = if span > 1 {
                        (span - 1) as f32 * effective_spacing
                    } else {
                        0.0
                    };
                    let current_sum: f32 = col_max_widths[gc..end_col].iter().sum();
                    let needed = cell.intrinsic_width - spanned_spacing;
                    if needed > current_sum {
                        let extra_per_col = (needed - current_sum) / span as f32;
                        for c in gc..end_col {
                            col_max_widths[c] += extra_per_col;
                        }
                    }
                }
            }
        }
    }

    // Total intrinsic width
    let total_spacing = if num_cols > 1 {
        (num_cols - 1) as f32 * effective_spacing
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
        let scale =
            (content_width - total_spacing).max(0.0) / (total_intrinsic - total_spacing).max(1.0);
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
            cx += effective_spacing;
        }
    }

    // ── Step 2b: Determine row heights ──
    // First pass: non-spanning rows (rowspan == 1).
    let mut row_heights = vec![0.0f32; num_rows];
    for (ri, row) in rows.iter().enumerate() {
        for cell in &row.cells {
            if cell.rowspan == 1 {
                row_heights[ri] = row_heights[ri].max(cell.intrinsic_height);
            }
        }
    }

    // Second pass: distribute spanning cells' excess height equally.
    for (ri, row) in rows.iter().enumerate() {
        for cell in &row.cells {
            if cell.rowspan > 1 {
                let end_row = (ri + cell.rowspan).min(num_rows);
                let span = end_row - ri;
                if span == 0 {
                    continue;
                }
                let spanned_spacing = if span > 1 {
                    (span - 1) as f32 * effective_spacing
                } else {
                    0.0
                };
                let current_sum: f32 = row_heights[ri..end_row].iter().sum();
                let needed = cell.intrinsic_height - spanned_spacing;
                if needed > current_sum {
                    let extra_per_row = (needed - current_sum) / span as f32;
                    for r in ri..end_row {
                        row_heights[r] += extra_per_row;
                    }
                }
            }
        }
    }

    // Pre-compute row y positions
    let mut row_y_positions = Vec::with_capacity(num_rows);
    {
        let mut ry = 0.0f32;
        for (ri, &rh) in row_heights.iter().enumerate() {
            row_y_positions.push(ry);
            ry += rh;
            if ri < num_rows - 1 {
                ry += effective_spacing;
            }
        }
    }

    // ── Step 3: Position cells in grid pattern ──
    for (ri, row) in rows.iter().enumerate() {
        for (ci, cell) in row.cells.iter().enumerate() {
            let gc = cell_grid_col[ri][ci];
            let end_col = (gc + cell.colspan).min(num_cols);
            let end_row = (ri + cell.rowspan).min(num_rows);

            // Spanned width = sum of column widths + internal spacing
            let mut cell_w: f32 = col_widths[gc..end_col].iter().sum();
            if end_col > gc + 1 {
                cell_w += (end_col - gc - 1) as f32 * effective_spacing;
            }

            // Spanned height = sum of row heights + internal spacing
            let mut cell_h: f32 = row_heights[ri..end_row].iter().sum();
            if end_row > ri + 1 {
                cell_h += (end_row - ri - 1) as f32 * effective_spacing;
            }

            // Cell position is LOCAL to the table's content area
            let cell_x = col_x_positions[gc];
            let cell_y = caption_y + row_y_positions[ri];

            // Apply vertical-align within cell (CSS 2.1 §17.5.4)
            let cell_style = styles.get(cell._node_id).cloned().unwrap_or_default();
            let content_h = cell.intrinsic_height.min(cell_h);
            let v_offset = match cell_style.vertical_align {
                VerticalAlign::Middle => (cell_h - content_h) / 2.0,
                VerticalAlign::Bottom | VerticalAlign::TextBottom => cell_h - content_h,
                // Top, Baseline, and others → content at top of cell
                _ => 0.0,
            };

            // Reposition the cell box
            if let Some(b) = tree.get_mut(cell.box_id) {
                let dx = cell_x - b.content_rect.x;
                let dy = (cell_y + v_offset) - b.content_rect.y;
                let dw = cell_w - b.content_rect.width;
                let dh = cell_h - b.content_rect.height;

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
    }

    // Total grid height
    let grid_height: f32 = if num_rows > 0 {
        row_y_positions[num_rows - 1] + row_heights[num_rows - 1]
    } else {
        0.0
    };

    // Position bottom captions after the grid
    if caption_bottom {
        let mut cap_y = grid_height;
        for cap in &captions {
            if let Some(b) = tree.get_mut(cap.box_id) {
                let dx = 0.0 - b.content_rect.x;
                let dy = cap_y - b.content_rect.y;
                b.content_rect.x += dx;
                b.content_rect.y += dy;
                b.padding_rect.x += dx;
                b.padding_rect.y += dy;
                b.border_rect.x += dx;
                b.border_rect.y += dy;
                b.margin_rect.x += dx;
                b.margin_rect.y += dy;
            }
            cap_y += cap.height;
        }
        caption_y = cap_y - grid_height; // total caption height for overall sizing
    }

    let total_content_height = caption_y + grid_height;
    let content_height = style
        .height
        .resolve_px(
            container_height,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(total_content_height);

    set_table_geometry(
        tree,
        box_id,
        content_x,
        content_y,
        content_width,
        content_height,
        pad_top,
        pad_right,
        pad_bottom,
        pad_left,
        border_top,
        border_right,
        border_bottom,
        border_left,
        mar_top,
        mar_right,
        mar_bottom,
        mar_left,
    );

    box_id
}

/// Check if a node's tag name indicates a table row.
#[allow(dead_code)]
fn is_table_row_tag(doc: &Document, node_id: NodeId) -> bool {
    doc.get(node_id)
        .map(|n| n.tag_name() == "tr")
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
    use crate::{DefaultImageMeasurer, DefaultTextMeasurer};
    use liquide_dom::Document;
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};

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

        let mut engine = StyleEngine::new(
            ViewportSize {
                width: 800.0,
                height: 600.0,
            },
            16.0,
        );
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
            800.0,
            600.0,
            0.0,
            0.0,
            800.0,
            600.0,
            16.0,
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
            800.0,
            600.0,
            0.0,
            0.0,
            800.0,
            600.0,
            16.0,
        );

        assert!(tree.get(box_id).is_some());
    }
}
