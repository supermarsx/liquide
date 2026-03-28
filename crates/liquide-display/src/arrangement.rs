use crate::display::{DisplayId, DisplayInfo};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Arrangement policies
// ---------------------------------------------------------------------------

/// Policy controlling how monitors are auto-arranged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrangementPolicy {
    /// Place all monitors side-by-side, left to right, sorted by connector name.
    SideBySide,
    /// Stack monitors vertically, top to bottom.
    Stacked,
    /// Mirror mode: all monitors at position (0, 0).
    Mirror,
    /// Custom per-monitor positions (used when the user manually drags monitors).
    Custom(Vec<MonitorPosition>),
}

impl Default for ArrangementPolicy {
    fn default() -> Self {
        ArrangementPolicy::SideBySide
    }
}

/// A monitor position entry for `ArrangementPolicy::Custom`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorPosition {
    pub id: DisplayId,
    pub x: i32,
    pub y: i32,
}

/// Result of `auto_arrange`: positions for each monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorArrangement {
    /// (display_id, x, y) for each monitor.
    pub positions: Vec<(DisplayId, i32, i32)>,
}

impl MonitorArrangement {
    /// Get the position assigned to a particular display.
    pub fn position_of(&self, id: DisplayId) -> Option<(i32, i32)> {
        self.positions
            .iter()
            .find(|(did, _, _)| *did == id)
            .map(|(_, x, y)| (*x, *y))
    }

    /// Apply this arrangement to a `DisplayArrangement`.
    pub fn apply_to(&self, arrangement: &mut DisplayArrangement) {
        for &(id, x, y) in &self.positions {
            arrangement.set_position(id, x, y);
        }
    }
}

/// Automatically arrange monitors according to the given policy.
///
/// For `SideBySide`, monitors are placed left-to-right sorted by connector name.
/// For `Stacked`, monitors are placed top-to-bottom sorted by connector name.
/// For `Mirror`, all monitors are at (0, 0).
/// For `Custom`, the provided positions are used directly.
pub fn auto_arrange(
    monitors: &[DisplayInfo],
    policy: &ArrangementPolicy,
) -> MonitorArrangement {
    let enabled: Vec<&DisplayInfo> = monitors.iter().filter(|d| d.enabled).collect();

    match policy {
        ArrangementPolicy::SideBySide => {
            let mut sorted: Vec<&DisplayInfo> = enabled;
            sorted.sort_by(|a, b| a.connector.cmp(&b.connector));
            let mut positions = Vec::new();
            let mut x = 0i32;
            for d in sorted {
                positions.push((d.id, x, 0));
                x += d.logical_width() as i32;
            }
            MonitorArrangement { positions }
        }
        ArrangementPolicy::Stacked => {
            let mut sorted: Vec<&DisplayInfo> = enabled;
            sorted.sort_by(|a, b| a.connector.cmp(&b.connector));
            let mut positions = Vec::new();
            let mut y = 0i32;
            for d in sorted {
                positions.push((d.id, 0, y));
                y += d.logical_height() as i32;
            }
            MonitorArrangement { positions }
        }
        ArrangementPolicy::Mirror => {
            let positions = enabled.iter().map(|d| (d.id, 0i32, 0i32)).collect();
            MonitorArrangement { positions }
        }
        ArrangementPolicy::Custom(custom) => {
            let positions = custom.iter().map(|mp| (mp.id, mp.x, mp.y)).collect();
            MonitorArrangement { positions }
        }
    }
}

/// Auto-arrange using the default `SideBySide` policy.
pub fn auto_arrange_default(monitors: &[DisplayInfo]) -> MonitorArrangement {
    auto_arrange(monitors, &ArrangementPolicy::SideBySide)
}

/// Snap all monitor positions in an arrangement to a pixel grid.
///
/// Each position `(x, y)` is rounded to the nearest multiple of `grid_size`.
/// A `grid_size` of 0 or 1 is a no-op.
pub fn snap_to_grid(arrangement: &mut MonitorArrangement, grid_size: u32) {
    if grid_size <= 1 {
        return;
    }
    let g = grid_size as i32;
    for (_, x, y) in &mut arrangement.positions {
        *x = ((*x + g / 2) / g) * g;
        *y = ((*y + g / 2) / g) * g;
    }
}

/// Describes a gap between monitors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapInfo {
    /// The gap rectangle: (x, y, width, height).
    pub rect: (i32, i32, u32, u32),
    /// IDs of the two displays adjacent to this gap.
    pub between: (DisplayId, DisplayId),
}

/// Detect gaps between adjacent monitors and return fix suggestions.
///
/// For each gap found, the second monitor in the `between` pair is shifted
/// to close the gap. Returns the list of gaps that were found.
pub fn fix_gaps(arrangement: &mut DisplayArrangement) -> Vec<GapInfo> {
    let mut found_gaps = Vec::new();
    let enabled: Vec<(DisplayId, i32, i32, i32, i32)> = arrangement
        .displays
        .iter()
        .filter(|d| d.enabled)
        .map(|d| {
            let (dx, dy, dw, dh) = d.bounds();
            (d.id, dx, dy, dx + dw as i32, dy + dh as i32)
        })
        .collect();

    // Find horizontal gaps between vertically overlapping displays.
    let mut by_left: Vec<usize> = (0..enabled.len()).collect();
    by_left.sort_by_key(|&i| enabled[i].1);

    for i in 0..by_left.len() {
        for j in (i + 1)..by_left.len() {
            let a = enabled[by_left[i]];
            let b = enabled[by_left[j]];
            // Vertical overlap?
            let vert_top = a.2.max(b.2);
            let vert_bot = a.4.min(b.4);
            if vert_top >= vert_bot {
                continue;
            }
            let gap_left = a.3;
            let gap_right = b.1;
            if gap_left >= gap_right {
                continue;
            }
            let gap_w = (gap_right - gap_left) as u32;
            let gap_h = (vert_bot - vert_top) as u32;
            let gap = GapInfo {
                rect: (gap_left, vert_top, gap_w, gap_h),
                between: (a.0, b.0),
            };
            found_gaps.push(gap);
            // Fix: shift the second display left to close the gap.
            if let Some(d) = arrangement.get_mut(b.0) {
                d.position.0 -= gap_w as i32;
            }
        }
    }

    // Find vertical gaps between horizontally overlapping displays.
    let mut by_top: Vec<usize> = (0..enabled.len()).collect();
    by_top.sort_by_key(|&i| enabled[i].2);

    for i in 0..by_top.len() {
        for j in (i + 1)..by_top.len() {
            let a = enabled[by_top[i]];
            let b = enabled[by_top[j]];
            let horz_left = a.1.max(b.1);
            let horz_right = a.3.min(b.3);
            if horz_left >= horz_right {
                continue;
            }
            let gap_top = a.4;
            let gap_bot = b.2;
            if gap_top >= gap_bot {
                continue;
            }
            let gap_w = (horz_right - horz_left) as u32;
            let gap_h = (gap_bot - gap_top) as u32;
            let gap = GapInfo {
                rect: (horz_left, gap_top, gap_w, gap_h),
                between: (a.0, b.0),
            };
            found_gaps.push(gap);
            if let Some(d) = arrangement.get_mut(b.0) {
                d.position.1 -= gap_h as i32;
            }
        }
    }

    found_gaps
}

/// Select the primary monitor from a list.
///
/// Strategy:
/// 1. If any monitor is already marked `primary`, return it.
/// 2. Otherwise, pick the monitor with the highest resolution (pixel count).
/// 3. Ties broken by lowest `id`.
pub fn primary_monitor(monitors: &[DisplayInfo]) -> Option<DisplayId> {
    let enabled: Vec<&DisplayInfo> = monitors.iter().filter(|d| d.enabled).collect();
    if enabled.is_empty() {
        return None;
    }
    // Already-primary?
    if let Some(d) = enabled.iter().find(|d| d.primary) {
        return Some(d.id);
    }
    // Highest resolution, lowest id for ties.
    enabled
        .iter()
        .max_by(|a, b| {
            a.resolution
                .pixel_count()
                .cmp(&b.resolution.pixel_count())
                .then(b.id.cmp(&a.id))
        })
        .map(|d| d.id)
}

/// Multi-monitor layout manager.
#[derive(Debug, Clone)]
pub struct DisplayArrangement {
    pub displays: Vec<DisplayInfo>,
}

impl DisplayArrangement {
    pub fn new(displays: Vec<DisplayInfo>) -> Self {
        Self { displays }
    }

    /// Find a display by ID.
    pub fn get(&self, id: DisplayId) -> Option<&DisplayInfo> {
        self.displays.iter().find(|d| d.id == id)
    }

    /// Find a display by ID (mutable).
    pub fn get_mut(&mut self, id: DisplayId) -> Option<&mut DisplayInfo> {
        self.displays.iter_mut().find(|d| d.id == id)
    }

    /// Set the position of a display in virtual desktop coordinates.
    pub fn set_position(&mut self, id: DisplayId, x: i32, y: i32) -> bool {
        if let Some(d) = self.get_mut(id) {
            d.position = (x, y);
            true
        } else {
            false
        }
    }

    /// Designate a display as primary (unsets primary on all others).
    pub fn set_primary(&mut self, id: DisplayId) -> bool {
        let exists = self.displays.iter().any(|d| d.id == id);
        if !exists {
            return false;
        }
        for d in &mut self.displays {
            d.primary = d.id == id;
        }
        true
    }

    /// Arrange the given displays horizontally left-to-right with the specified
    /// gap (in virtual desktop pixels) between them. The first display in `ids`
    /// is placed at `(start_x, start_y)`.
    pub fn align_horizontal(
        &mut self,
        ids: &[DisplayId],
        gap: i32,
        start_x: i32,
        start_y: i32,
    ) {
        let mut x = start_x;
        for &id in ids {
            if let Some(d) = self.get_mut(id) {
                d.position = (x, start_y);
                let w = d.logical_width() as i32;
                x += w + gap;
            }
        }
    }

    /// Arrange the given displays vertically top-to-bottom with the specified
    /// gap between them.
    pub fn align_vertical(
        &mut self,
        ids: &[DisplayId],
        gap: i32,
        start_x: i32,
        start_y: i32,
    ) {
        let mut y = start_y;
        for &id in ids {
            if let Some(d) = self.get_mut(id) {
                d.position = (start_x, y);
                let h = d.logical_height() as i32;
                y += h + gap;
            }
        }
    }

    /// Bounding rectangle of all enabled displays: (x, y, width, height).
    /// Returns `(0, 0, 0, 0)` if there are no enabled displays.
    pub fn bounds(&self) -> (i32, i32, u32, u32) {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut any = false;

        for d in &self.displays {
            if !d.enabled {
                continue;
            }
            any = true;
            let (dx, dy, dw, dh) = d.bounds();
            min_x = min_x.min(dx);
            min_y = min_y.min(dy);
            max_x = max_x.max(dx + dw as i32);
            max_y = max_y.max(dy + dh as i32);
        }

        if !any {
            return (0, 0, 0, 0);
        }

        (
            min_x,
            min_y,
            (max_x - min_x) as u32,
            (max_y - min_y) as u32,
        )
    }

    /// Find which display contains the given virtual desktop point.
    /// Returns the first match (by insertion order).
    pub fn display_at_point(&self, x: i32, y: i32) -> Option<DisplayId> {
        for d in &self.displays {
            if !d.enabled {
                continue;
            }
            let (dx, dy, dw, dh) = d.bounds();
            if x >= dx && x < dx + dw as i32 && y >= dy && y < dy + dh as i32 {
                return Some(d.id);
            }
        }
        None
    }

    /// Detect pairs of enabled displays whose bounding rectangles overlap.
    pub fn overlaps(&self) -> Vec<(DisplayId, DisplayId)> {
        let mut result = Vec::new();
        let enabled: Vec<&DisplayInfo> = self.displays.iter().filter(|d| d.enabled).collect();
        for i in 0..enabled.len() {
            for j in (i + 1)..enabled.len() {
                let (ax, ay, aw, ah) = enabled[i].bounds();
                let (bx, by, bw, bh) = enabled[j].bounds();
                if rects_overlap(ax, ay, aw, ah, bx, by, bw, bh) {
                    result.push((enabled[i].id, enabled[j].id));
                }
            }
        }
        result
    }

    /// Detect gaps between adjacent enabled displays. Returns a list of gap
    /// rectangles `(x, y, w, h)` found on the boundary of the arrangement's
    /// bounding rect that are not covered by any display.
    ///
    /// This scans horizontal and vertical strips between displays. It is an
    /// approximation: it checks the strip between each pair of horizontally or
    /// vertically adjacent displays.
    pub fn gaps(&self) -> Vec<(i32, i32, u32, u32)> {
        let enabled: Vec<&DisplayInfo> = self.displays.iter().filter(|d| d.enabled).collect();
        if enabled.len() < 2 {
            return Vec::new();
        }

        let (bx, by, bw, bh) = self.bounds();
        if bw == 0 || bh == 0 {
            return Vec::new();
        }

        // Rasterize the bounding rect into a coverage grid at 1-pixel granularity
        // would be expensive. Instead, we use a scanline approach on the
        // sorted display edges.

        let mut gap_rects = Vec::new();

        // Collect all display bounds.
        let bounds: Vec<(i32, i32, i32, i32)> = enabled
            .iter()
            .map(|d| {
                let (dx, dy, dw, dh) = d.bounds();
                (dx, dy, dx + dw as i32, dy + dh as i32)
            })
            .collect();

        // Check horizontal gaps: for each pair sorted by left edge, see if
        // there's uncovered horizontal space between them at their overlapping
        // vertical range.
        let mut by_left: Vec<usize> = (0..bounds.len()).collect();
        by_left.sort_by_key(|&i| bounds[i].0);

        for i in 0..by_left.len() {
            for j in (i + 1)..by_left.len() {
                let a = bounds[by_left[i]];
                let b = bounds[by_left[j]];
                // Check if there is vertical overlap between a and b.
                let vert_top = a.1.max(b.1);
                let vert_bot = a.3.min(b.3);
                if vert_top >= vert_bot {
                    continue; // no vertical overlap
                }
                // Check horizontal gap: right edge of a to left edge of b.
                let gap_left = a.2;
                let gap_right = b.0;
                if gap_left >= gap_right {
                    continue; // no gap (overlap or adjacent)
                }
                // Verify this gap region isn't covered by another display.
                let gap_rect = (gap_left, vert_top, gap_right, vert_bot);
                let covered = bounds.iter().any(|&(dx, dy, dr, db)| {
                    dx <= gap_rect.0
                        && dr >= gap_rect.2
                        && dy <= gap_rect.1
                        && db >= gap_rect.3
                });
                if !covered {
                    gap_rects.push((
                        gap_rect.0,
                        gap_rect.1,
                        (gap_rect.2 - gap_rect.0) as u32,
                        (gap_rect.3 - gap_rect.1) as u32,
                    ));
                }
            }
        }

        // Check vertical gaps similarly.
        let mut by_top: Vec<usize> = (0..bounds.len()).collect();
        by_top.sort_by_key(|&i| bounds[i].1);

        for i in 0..by_top.len() {
            for j in (i + 1)..by_top.len() {
                let a = bounds[by_top[i]];
                let b = bounds[by_top[j]];
                // Check horizontal overlap.
                let horz_left = a.0.max(b.0);
                let horz_right = a.2.min(b.2);
                if horz_left >= horz_right {
                    continue;
                }
                // Vertical gap.
                let gap_top = a.3;
                let gap_bot = b.1;
                if gap_top >= gap_bot {
                    continue;
                }
                let gap_rect = (horz_left, gap_top, horz_right, gap_bot);
                let covered = bounds.iter().any(|&(dx, dy, dr, db)| {
                    dx <= gap_rect.0
                        && dr >= gap_rect.2
                        && dy <= gap_rect.1
                        && db >= gap_rect.3
                });
                if !covered {
                    gap_rects.push((
                        gap_rect.0,
                        gap_rect.1,
                        (gap_rect.2 - gap_rect.0) as u32,
                        (gap_rect.3 - gap_rect.1) as u32,
                    ));
                }
            }
        }

        // Deduplicate.
        gap_rects.sort();
        gap_rects.dedup();

        // Suppress anything outside the bounding rect.
        let br = bx + bw as i32;
        let bb = by + bh as i32;
        gap_rects.retain(|&(gx, gy, gw, gh)| {
            gx >= bx && gy >= by && gx + gw as i32 <= br && gy + gh as i32 <= bb
        });

        gap_rects
    }
}

/// Check if two axis-aligned rectangles overlap (strictly — touching edges
/// are not considered overlap).
fn rects_overlap(
    ax: i32,
    ay: i32,
    aw: u32,
    ah: u32,
    bx: i32,
    by: i32,
    bw: u32,
    bh: u32,
) -> bool {
    let ar = ax + aw as i32;
    let ab = ay + ah as i32;
    let br = bx + bw as i32;
    let bb = by + bh as i32;
    ax < br && ar > bx && ay < bb && ab > by
}
