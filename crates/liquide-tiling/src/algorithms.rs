//! Layout computation algorithms for each tiling mode.

use liquide_compositor::geometry::Rect;

use crate::gaps::TilingGaps;
use crate::layout::{TileZone, TilingLayout};

/// Compute window rectangles for the given layout, window count, work area,
/// master ratio, master count, and gap config.
///
/// Returns one `Rect` per window, in the same order as the window list.
#[must_use]
pub fn compute_layout(
    layout: &TilingLayout,
    window_count: usize,
    work_area: Rect,
    master_ratio: f32,
    master_count: usize,
    gaps: &TilingGaps,
) -> Vec<Rect> {
    if window_count == 0 {
        return Vec::new();
    }

    let eff = gaps.effective(window_count);
    let usable = eff.usable_area(work_area);
    let g = eff.inner;

    match layout {
        TilingLayout::Columns => {
            layout_columns(window_count, usable, master_ratio, master_count, g)
        }
        TilingLayout::Rows => layout_rows(window_count, usable, master_ratio, master_count, g),
        TilingLayout::Grid => layout_grid(window_count, usable, g),
        TilingLayout::ThreeColumn => layout_three_column(window_count, usable, master_ratio, g),
        TilingLayout::Spiral => layout_spiral(window_count, usable, master_ratio, g),
        TilingLayout::Monocle => layout_monocle(window_count, usable),
        TilingLayout::Float => layout_float(window_count, usable),
        TilingLayout::Custom(zones) => layout_custom(window_count, work_area, zones, g),
    }
}

/// Master-stack with master on the left, stack on the right.
/// Supports multiple master windows (stacked vertically in the master column).
fn layout_columns(n: usize, area: Rect, ratio: f32, masters: usize, gap: f32) -> Vec<Rect> {
    let mc = masters.min(n);

    if mc == n {
        // All windows are masters — split the entire area vertically.
        return split_vertical(n, area, gap);
    }

    let master_w = area.width * ratio - gap / 2.0;
    let stack_w = area.width - master_w - gap;
    let stack_count = n - mc;

    let mut rects = Vec::with_capacity(n);

    // Master column: mc windows split vertically.
    let master_area = Rect::new(area.x, area.y, master_w, area.height);
    rects.extend(split_vertical(mc, master_area, gap));

    // Stack column: remaining windows split vertically.
    let stack_area = Rect::new(area.x + master_w + gap, area.y, stack_w, area.height);
    rects.extend(split_vertical(stack_count, stack_area, gap));

    rects
}

/// Master-stack with master on top, stack on the bottom.
fn layout_rows(n: usize, area: Rect, ratio: f32, masters: usize, gap: f32) -> Vec<Rect> {
    let mc = masters.min(n);

    if mc == n {
        return split_horizontal(n, area, gap);
    }

    let master_h = area.height * ratio - gap / 2.0;
    let stack_h = area.height - master_h - gap;
    let stack_count = n - mc;

    let mut rects = Vec::with_capacity(n);

    let master_area = Rect::new(area.x, area.y, area.width, master_h);
    rects.extend(split_horizontal(mc, master_area, gap));

    let stack_area = Rect::new(area.x, area.y + master_h + gap, area.width, stack_h);
    rects.extend(split_horizontal(stack_count, stack_area, gap));

    rects
}

/// Equal-sized grid. Uses ceil(sqrt(n)) columns and distributes windows
/// row by row. The last row may have fewer columns.
fn layout_grid(n: usize, area: Rect, gap: f32) -> Vec<Rect> {
    let cols = (n as f32).sqrt().ceil() as usize;
    let rows = (n + cols - 1) / cols;

    let total_h_gap = gap * (rows as f32 - 1.0).max(0.0);
    let total_w_gap = gap * (cols as f32 - 1.0).max(0.0);
    let cell_h = (area.height - total_h_gap) / rows as f32;

    let mut rects = Vec::with_capacity(n);
    let mut idx = 0;

    for r in 0..rows {
        let remaining = n - idx;
        let cols_this_row = if r == rows - 1 {
            remaining
        } else {
            cols.min(remaining)
        };
        let this_gap = gap * (cols_this_row as f32 - 1.0).max(0.0);
        let cell_w = (area.width - this_gap) / cols_this_row as f32;

        for c in 0..cols_this_row {
            let x = area.x + c as f32 * (cell_w + gap);
            let y = area.y + r as f32 * (cell_h + gap);
            rects.push(Rect::new(x, y, cell_w, cell_h));
            idx += 1;
        }
    }
    // Avoid unused variable warning from total_w_gap used only implicitly.
    let _ = total_w_gap;
    rects
}

/// Three-column layout: left stack | center master | right stack.
/// Non-master windows alternate between left and right stacks.
fn layout_three_column(n: usize, area: Rect, ratio: f32, gap: f32) -> Vec<Rect> {
    if n == 1 {
        return vec![area];
    }

    if n == 2 {
        // Two windows: master (center) and one side.
        let side_ratio = (1.0 - ratio) / 2.0;
        let side_w = area.width * side_ratio - gap / 2.0;
        let center_w = area.width - side_w - gap;
        return vec![
            Rect::new(
                area.x + side_w + gap,
                area.y,
                center_w - side_w - gap,
                area.height,
            ),
            Rect::new(area.x, area.y, side_w, area.height),
        ];
    }

    let side_ratio = (1.0 - ratio) / 2.0;
    let center_w = area.width * ratio;
    let side_w = (area.width - center_w - 2.0 * gap) / 2.0;

    let left_x = area.x;
    let center_x = area.x + side_w + gap;
    let right_x = center_x + center_w + gap;

    // Distribute non-master windows into left and right stacks.
    let mut left_count = 0usize;
    let mut right_count = 0usize;
    for i in 1..n {
        if i % 2 == 1 {
            left_count += 1;
        } else {
            right_count += 1;
        }
    }

    let mut rects = Vec::with_capacity(n);

    // Index 0 = master (center).
    rects.push(Rect::new(center_x, area.y, center_w, area.height));

    // Interleave left and right assignments.
    let mut left_idx = 0usize;
    let mut right_idx = 0usize;
    for i in 1..n {
        if i % 2 == 1 {
            // Left stack.
            let h = stack_item_height(area.height, left_count, left_idx, gap);
            let y = stack_item_y(area.y, area.height, left_count, left_idx, gap);
            rects.push(Rect::new(left_x, y, side_w, h));
            left_idx += 1;
        } else {
            // Right stack.
            let h = stack_item_height(area.height, right_count, right_idx, gap);
            let y = stack_item_y(area.y, area.height, right_count, right_idx, gap);
            rects.push(Rect::new(right_x, y, side_w, h));
            right_idx += 1;
        }
    }

    let _ = side_ratio;
    rects
}

/// Fibonacci spiral layout. Each split alternates between horizontal and
/// vertical, producing a spiral pattern.
fn layout_spiral(n: usize, area: Rect, ratio: f32, gap: f32) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(n);
    let mut remaining = area;

    for i in 0..n {
        if i == n - 1 {
            rects.push(remaining);
            break;
        }

        if i % 2 == 0 {
            // Split left/right.
            let w = remaining.width * ratio - gap / 2.0;
            rects.push(Rect::new(remaining.x, remaining.y, w, remaining.height));
            remaining = Rect::new(
                remaining.x + w + gap,
                remaining.y,
                remaining.width - w - gap,
                remaining.height,
            );
        } else {
            // Split top/bottom.
            let h = remaining.height * ratio - gap / 2.0;
            rects.push(Rect::new(remaining.x, remaining.y, remaining.width, h));
            remaining = Rect::new(
                remaining.x,
                remaining.y + h + gap,
                remaining.width,
                remaining.height - h - gap,
            );
        }
    }
    rects
}

/// Monocle layout: every window occupies the full area.
fn layout_monocle(n: usize, area: Rect) -> Vec<Rect> {
    vec![area; n]
}

/// Float layout: no tiling. Returns centered default-sized rectangles.
/// In practice the shell overrides these with actual window positions.
fn layout_float(n: usize, area: Rect) -> Vec<Rect> {
    let default_w = (area.width * 0.5).min(800.0);
    let default_h = (area.height * 0.5).min(600.0);
    let mut rects = Vec::with_capacity(n);

    for i in 0..n {
        let offset = 30.0 * i as f32;
        let x = area.x + (area.width - default_w) / 2.0 + offset;
        let y = area.y + (area.height - default_h) / 2.0 + offset;
        rects.push(Rect::new(x, y, default_w, default_h));
    }
    rects
}

/// Custom zone layout. Windows are assigned to zones in order; if there
/// are more windows than zone capacity, extras are placed in the last zone.
fn layout_custom(n: usize, work_area: Rect, zones: &[TileZone], gap: f32) -> Vec<Rect> {
    if zones.is_empty() {
        return layout_monocle(n, work_area);
    }

    // Pre-compute pixel rects for each zone.
    let zone_rects: Vec<Rect> = zones
        .iter()
        .map(|z| {
            Rect::new(
                work_area.x + z.rect.x * work_area.width,
                work_area.y + z.rect.y * work_area.height,
                z.rect.w * work_area.width,
                z.rect.h * work_area.height,
            )
        })
        .collect();

    // Assign windows to zones.
    let mut assignments: Vec<Vec<usize>> = vec![Vec::new(); zones.len()];
    let mut zone_idx = 0;
    for win_idx in 0..n {
        // Advance to the next zone if current is full.
        while zone_idx < zones.len() - 1 {
            let max = zones[zone_idx].max_windows.unwrap_or(u32::MAX) as usize;
            if assignments[zone_idx].len() >= max {
                zone_idx += 1;
            } else {
                break;
            }
        }
        assignments[zone_idx].push(win_idx);
    }

    // Build result.
    let mut rects = vec![Rect::ZERO; n];
    for (zi, win_indices) in assignments.iter().enumerate() {
        if win_indices.is_empty() {
            continue;
        }
        let zr = zone_rects[zi];
        let sub_rects = split_vertical(win_indices.len(), zr, gap);
        for (j, &wi) in win_indices.iter().enumerate() {
            rects[wi] = sub_rects[j];
        }
    }
    rects
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split an area into `n` equal vertical strips (stacked top to bottom).
fn split_vertical(n: usize, area: Rect, gap: f32) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![area];
    }
    let total_gap = gap * (n as f32 - 1.0);
    let h = (area.height - total_gap) / n as f32;
    (0..n)
        .map(|i| {
            let y = area.y + i as f32 * (h + gap);
            Rect::new(area.x, y, area.width, h)
        })
        .collect()
}

/// Split an area into `n` equal horizontal strips (stacked left to right).
fn split_horizontal(n: usize, area: Rect, gap: f32) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![area];
    }
    let total_gap = gap * (n as f32 - 1.0);
    let w = (area.width - total_gap) / n as f32;
    (0..n)
        .map(|i| {
            let x = area.x + i as f32 * (w + gap);
            Rect::new(x, area.y, w, area.height)
        })
        .collect()
}

/// Compute the height of item at `idx` in a stack of `count` items.
fn stack_item_height(total_h: f32, count: usize, _idx: usize, gap: f32) -> f32 {
    if count == 0 {
        return total_h;
    }
    let total_gap = gap * (count as f32 - 1.0).max(0.0);
    (total_h - total_gap) / count as f32
}

/// Compute the y position of item at `idx` in a stack of `count` items.
fn stack_item_y(base_y: f32, total_h: f32, count: usize, idx: usize, gap: f32) -> f32 {
    let h = stack_item_height(total_h, count, idx, gap);
    base_y + idx as f32 * (h + gap)
}
