//! Flex layout — CSS Flexbox Level 1 with multi-line wrap support.
//!
//! Implements the core flexbox algorithm:
//! 1. Collect flex items (skip display:none and absolutely positioned)
//! 2. Sort by order
//! 3. Determine main/cross axis
//! 4. Break into flex lines (wrap support)
//! 5. Per-line: grow/shrink, justify, min/max clamp
//! 6. Cross axis: align-items, align-content for multi-line
//! 7. Position items absolutely

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{
    AlignContent, AlignItems, AspectRatio, Display, FlexDirection, FlexWrap, JustifyContent, Position,
};
use liquide_style_engine::dimension::Dimension;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};

/// Perform flexbox layout with full multi-line wrapping.
pub fn layout_flex(
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
    let box_id = tree.alloc(node_id, BoxType::Flex);

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

    let direction = style.flex_direction;
    let is_row = matches!(direction, FlexDirection::Row | FlexDirection::RowReverse);
    let is_reverse = matches!(
        direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
    );
    let wrap = style.flex_wrap;
    let should_wrap = wrap != FlexWrap::NoWrap;

    let gap_dim = if is_row {
        &style.gap.width
    } else {
        &style.gap.height
    };
    let gap = gap_dim
        .resolve_px(
            content_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let cross_gap_dim = if is_row {
        &style.gap.height
    } else {
        &style.gap.width
    };
    let cross_gap = cross_gap_dim
        .resolve_px(
            content_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);

    // ── Step 1: Collect flex items ──
    let children = doc.children(node_id).to_vec();
    let mut items: Vec<FlexItem> = Vec::new();

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        let child_box = if child_style.is_flex_container() {
            crate::flex::layout_flex(
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
            )
        } else if child_style.is_grid_container() {
            crate::grid::layout_grid(
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
            )
        } else {
            crate::block::layout_block(
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
            )
        };

        let intrinsic = tree
            .get(child_box)
            .map(|b| b.margin_rect)
            .unwrap_or(Rect::zero());

        let flex_basis = child_style.flex_basis.resolve_px(
            content_width,
            base_font_size,
            child_style.font_size,
            viewport_w,
            viewport_h,
        );

        let main_size = if is_row {
            flex_basis.unwrap_or(intrinsic.width)
        } else {
            flex_basis.unwrap_or(intrinsic.height)
        };

        let min_main = if is_row {
            child_style
                .min_width
                .resolve_px(
                    content_width,
                    base_font_size,
                    child_style.font_size,
                    viewport_w,
                    viewport_h,
                )
                .unwrap_or(0.0)
        } else {
            child_style
                .min_height
                .resolve_px(
                    container_height,
                    base_font_size,
                    child_style.font_size,
                    viewport_w,
                    viewport_h,
                )
                .unwrap_or(0.0)
        };

        let max_main = if is_row {
            child_style
                .max_width
                .resolve_px(
                    content_width,
                    base_font_size,
                    child_style.font_size,
                    viewport_w,
                    viewport_h,
                )
                .unwrap_or(f32::INFINITY)
        } else {
            child_style
                .max_height
                .resolve_px(
                    container_height,
                    base_font_size,
                    child_style.font_size,
                    viewport_w,
                    viewport_h,
                )
                .unwrap_or(f32::INFINITY)
        };

        // Calculate baseline for this item
        // For text items, use the baseline stored in the layout box
        // For other items, default to the bottom of the content box
        let item_baseline = tree.get(child_box)
            .and_then(|b| b.baseline)
            .unwrap_or_else(|| {
                // Default baseline: use content height for row layout, width for column
                if is_row { intrinsic.height } else { intrinsic.width }
            });

        items.push(FlexItem {
            box_id: child_box,
            node_id: child_id,
            flex_grow: child_style.flex_grow,
            flex_shrink: child_style.flex_shrink,
            base_main_size: main_size,
            main_size,
            cross_size: {
                let intrinsic_cross = if is_row {
                    intrinsic.height
                } else {
                    intrinsic.width
                };
                // Apply aspect-ratio when cross size is 0 (no explicit dimension)
                match child_style.aspect_ratio {
                    AspectRatio::Ratio(w, h) if intrinsic_cross <= 0.0 && w > 0.0 => {
                        if is_row { main_size * (h / w) } else { main_size * (w / h) }
                    }
                    _ => intrinsic_cross,
                }
            },
            min_main,
            max_main,
            order: child_style.order,
            baseline: item_baseline,
        });

        tree.add_child(box_id, child_box);
    }

    // ── Step 2: Sort by order ──
    items.sort_by_key(|i| i.order);
    if is_reverse {
        items.reverse();
    }

    // ── Step 3: Break into flex lines ──
    let available_main = if is_row {
        content_width
    } else {
        style
            .height
            .resolve_px(
                container_height,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(container_height)
    };

    let lines = if should_wrap && !items.is_empty() {
        let mut lines: Vec<FlexLine> = Vec::new();
        let mut line_start = 0;
        let mut line_main = 0.0f32;

        for i in 0..items.len() {
            let item_main = items[i].base_main_size;
            let gap_before = if i > line_start { gap } else { 0.0 };

            if i > line_start && line_main + gap_before + item_main > available_main {
                lines.push(FlexLine {
                    start: line_start,
                    end: i,
                });
                line_start = i;
                line_main = item_main;
            } else {
                line_main += gap_before + item_main;
            }
        }
        lines.push(FlexLine {
            start: line_start,
            end: items.len(),
        });

        if wrap == FlexWrap::WrapReverse {
            lines.reverse();
        }
        lines
    } else {
        vec![FlexLine {
            start: 0,
            end: items.len(),
        }]
    };

    // ── Step 4: Per-line grow/shrink ──
    for line in &lines {
        let line_items = &mut items[line.start..line.end];
        let count = line_items.len();
        let total_gaps = if count > 1 {
            (count - 1) as f32 * gap
        } else {
            0.0
        };
        let total_main: f32 = line_items.iter().map(|i| i.main_size).sum::<f32>() + total_gaps;
        let free_space = available_main - total_main;

        if free_space > 0.0 {
            let total_grow: f32 = line_items.iter().map(|i| i.flex_grow).sum();
            if total_grow > 0.0 {
                for item in line_items.iter_mut() {
                    let grow = free_space * (item.flex_grow / total_grow);
                    item.main_size = (item.main_size + grow)
                        .min(item.max_main)
                        .max(item.min_main);
                }
            }
        } else if free_space < 0.0 {
            let total_shrink: f32 = line_items
                .iter()
                .map(|i| i.flex_shrink * i.base_main_size)
                .sum();
            if total_shrink > 0.0 {
                for item in line_items.iter_mut() {
                    let factor = (item.flex_shrink * item.base_main_size) / total_shrink;
                    item.main_size = (item.main_size + free_space * factor)
                        .min(item.max_main)
                        .max(item.min_main);
                }
            }
        }
    }

    // ── Step 4b: Re-layout items whose resolved size differs from initial ──
    // Children were initially laid out at content_width. After grow/shrink the
    // actual main size may be smaller (shrink) or larger (grow). Re-layout each
    // child at its resolved size so that text wrapping, nested flex, etc. use
    // the correct available width.
    for item in &mut items {
        let initial_main = item.base_main_size;
        let resolved_main = item.main_size;

        // Tolerance: skip re-layout when the difference is negligible
        if (resolved_main - initial_main).abs() < 0.5 {
            continue;
        }

        let child_style = styles.get(item.node_id).cloned().unwrap_or_default();

        // Remove old child from tree (detach children list), we'll re-add
        tree.remove_child(box_id, item.box_id);

        let (child_w, child_h) = if is_row {
            (resolved_main, container_height)
        } else {
            (content_width, resolved_main)
        };

        let new_box = if child_style.is_flex_container() {
            crate::flex::layout_flex(
                doc,
                item.node_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_w,
                child_h,
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
                child_w,
                child_h,
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
                child_w,
                child_h,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        };

        item.box_id = new_box;
        tree.add_child(box_id, new_box);

        // Update cross size from re-layout
        let new_intrinsic = tree
            .get(new_box)
            .map(|b| b.margin_rect)
            .unwrap_or(Rect::zero());
        item.cross_size = if is_row {
            new_intrinsic.height
        } else {
            new_intrinsic.width
        };
    }

    // ── Step 5: Position items per line ──
    let mut cross_offset = 0.0f32;
    let mut line_cross_sizes: Vec<f32> = Vec::new();

    for line in &lines {
        let line_items = &items[line.start..line.end];
        let count = line_items.len();
        let total_gaps = if count > 1 {
            (count - 1) as f32 * gap
        } else {
            0.0
        };
        let used_main: f32 = line_items.iter().map(|i| i.main_size).sum::<f32>() + total_gaps;
        let remaining = available_main - used_main;

        // Justify content
        let (mut main_pos, extra_gap) = justify(style.justify_content, remaining, count);

        let line_cross: f32 = line_items
            .iter()
            .map(|i| i.cross_size)
            .fold(0.0f32, |a, b| a.max(b));

        for (i, idx) in (line.start..line.end).enumerate() {
            let item = &mut items[idx];
            // (x, y) is the desired LOCAL margin-edge position within the
            // flex container's content area (offsets from 0,0).
            let (x, y) = if is_row {
                (main_pos, cross_offset)
            } else {
                (cross_offset, main_pos)
            };

            if let Some(b) = tree.get_mut(item.box_id) {
                let dx = x - b.margin_rect.x;
                let dy = y - b.margin_rect.y;
                shift_box(b, dx, dy);
            }

            main_pos += item.main_size + gap + if i < count - 1 { extra_gap } else { 0.0 };
        }

        line_cross_sizes.push(line_cross);
        cross_offset += line_cross + cross_gap;
    }

    // ── Step 6: Align items on cross axis ──
    let total_cross = cross_offset
        - if !line_cross_sizes.is_empty() {
            cross_gap
        } else {
            0.0
        };
    let container_cross = if is_row {
        style
            .height
            .resolve_px(
                container_height,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(total_cross)
    } else {
        content_width
    };

    // Align-content (distributes space between lines)
    if lines.len() > 1 {
        let free_cross = container_cross - total_cross;
        let (mut cross_start, cross_extra) =
            align_content(style.align_content, free_cross, lines.len());

        for (li, line) in lines.iter().enumerate() {
            let delta = cross_start;
            for idx in line.start..line.end {
                let bid = items[idx].box_id;
                let (dx, dy) = if is_row { (0.0, delta) } else { (delta, 0.0) };
                if let Some(b) = tree.get_mut(bid) {
                    shift_box(b, dx, dy);
                }
            }
            cross_start += line_cross_sizes[li] + cross_gap + cross_extra;
        }
    }

    // Align-items per item within each line
    let mut _line_cross_y = 0.0f32;
    for (li, line) in lines.iter().enumerate() {
        let line_cross = line_cross_sizes[li];
        
        // Calculate the maximum baseline among all baseline-aligned items in this line
        let mut max_baseline = 0.0f32;
        for idx in line.start..line.end {
            let item = &items[idx];
            let child_style = styles.get(item.node_id).cloned().unwrap_or_default();
            let uses_baseline = match child_style.align_self {
                liquide_style_engine::computed::AlignSelf::Baseline => true,
                liquide_style_engine::computed::AlignSelf::Auto => {
                    matches!(style.align_items, AlignItems::Baseline)
                }
                _ => false,
            };
            if uses_baseline {
                max_baseline = max_baseline.max(item.baseline);
            }
        }
        
        for idx in line.start..line.end {
            let item = &items[idx];
            let child_style = styles.get(item.node_id).cloned().unwrap_or_default();
            let align = match child_style.align_self {
                liquide_style_engine::computed::AlignSelf::Auto => style.align_items,
                liquide_style_engine::computed::AlignSelf::FlexStart => AlignItems::FlexStart,
                liquide_style_engine::computed::AlignSelf::FlexEnd => AlignItems::FlexEnd,
                liquide_style_engine::computed::AlignSelf::Center => AlignItems::Center,
                liquide_style_engine::computed::AlignSelf::Baseline => AlignItems::Baseline,
                liquide_style_engine::computed::AlignSelf::Stretch => AlignItems::Stretch,
            };

            let cross_offset_val;
            if let Some(b) = tree.get_mut(item.box_id) {
                // Use margin_rect for cross size since line_cross is margin-box based
                let item_cross = if is_row {
                    b.margin_rect.height
                } else {
                    b.margin_rect.width
                };
                cross_offset_val = match align {
                    AlignItems::FlexStart => 0.0,
                    AlignItems::FlexEnd => line_cross - item_cross,
                    AlignItems::Center => (line_cross - item_cross) / 2.0,
                    AlignItems::Stretch => {
                        // Stretch content to fill line: delta = line_cross - current_margin_box
                        let stretch = (line_cross - item_cross).max(0.0);
                        let dw = if !is_row { stretch } else { 0.0 };
                        let dh = if is_row { stretch } else { 0.0 };
                        b.content_rect.width += dw;
                        b.content_rect.height += dh;
                        b.padding_rect.width += dw;
                        b.padding_rect.height += dh;
                        b.border_rect.width += dw;
                        b.border_rect.height += dh;
                        b.margin_rect.width += dw;
                        b.margin_rect.height += dh;
                        0.0
                    }
                    AlignItems::Baseline => {
                        // Align items so their baselines match
                        // Move item down by (max_baseline - item_baseline)
                        max_baseline - item.baseline
                    }
                };

                let (dx, dy) = if is_row {
                    (0.0, cross_offset_val)
                } else {
                    (cross_offset_val, 0.0)
                };
                shift_box(b, dx, dy);
            } else {
                cross_offset_val = 0.0;
            }
        }
        _line_cross_y += line_cross_sizes[li] + cross_gap;
    }

    // ── Step 7: Set container geometry ──
    let content_height = if is_row {
        style
            .height
            .resolve_px(
                container_height,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(total_cross.max(0.0))
    } else {
        let last_main: f32 = items.iter().map(|i| i.main_size).sum::<f32>()
            + if items.len() > 1 {
                (items.len() - 1) as f32 * gap
            } else {
                0.0
            };
        last_main
    };

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

// ── Internal types ──

struct FlexItem {
    box_id: LayoutBoxId,
    node_id: NodeId,
    flex_grow: f32,
    flex_shrink: f32,
    base_main_size: f32,
    main_size: f32,
    cross_size: f32,
    min_main: f32,
    max_main: f32,
    order: i32,
    /// Baseline offset from the cross-start edge (for baseline alignment).
    /// For text items, this is the distance from the top of the box to the first
    /// baseline. For non-text items, defaults to the bottom of margin box.
    baseline: f32,
}

struct FlexLine {
    start: usize,
    end: usize,
}

// ── Helpers ──

/// Reposition a layout box: move content_rect to (x, y) with size (w, h),
/// and propagate position/size deltas to padding/border/margin rects.
/// Shift all rects of a layout box by a position delta.
fn shift_box(b: &mut crate::tree::LayoutBox, dx: f32, dy: f32) {
    b.content_rect.x += dx;
    b.content_rect.y += dy;
    b.padding_rect.x += dx;
    b.padding_rect.y += dy;
    b.border_rect.x += dx;
    b.border_rect.y += dy;
    b.margin_rect.x += dx;
    b.margin_rect.y += dy;
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

/// Compute justify-content start offset and inter-item extra gap.
fn justify(jc: JustifyContent, remaining: f32, count: usize) -> (f32, f32) {
    if count == 0 {
        return (0.0, 0.0);
    }
    match jc {
        JustifyContent::FlexStart => (0.0, 0.0),
        JustifyContent::FlexEnd => (remaining.max(0.0), 0.0),
        JustifyContent::Center => (remaining.max(0.0) / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if count > 1 {
                (0.0, (remaining.max(0.0)) / (count - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        JustifyContent::SpaceAround => {
            let s = remaining.max(0.0) / count as f32;
            (s / 2.0, s)
        }
        JustifyContent::SpaceEvenly => {
            let s = remaining.max(0.0) / (count + 1) as f32;
            (s, s)
        }
    }
}

/// Compute align-content start offset and inter-line extra gap.
fn align_content(ac: AlignContent, free: f32, line_count: usize) -> (f32, f32) {
    if line_count <= 1 || free <= 0.0 {
        return (0.0, 0.0);
    }
    match ac {
        AlignContent::FlexStart => (0.0, 0.0),
        AlignContent::FlexEnd => (free, 0.0),
        AlignContent::Center => (free / 2.0, 0.0),
        AlignContent::SpaceBetween => (0.0, free / (line_count - 1) as f32),
        AlignContent::SpaceAround => {
            let s = free / line_count as f32;
            (s / 2.0, s)
        }
        AlignContent::Stretch => (0.0, free / line_count as f32),
    }
}
