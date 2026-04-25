//! Central tiling manager with window tracking, layout computation, and
//! keyboard-driven navigation.

use liquide_compositor::geometry::Rect;

use crate::algorithms;
use crate::gaps::TilingGaps;
use crate::layout::{Direction, RotateDir, TilingLayout};
use crate::navigate::{self, WindowId};

/// Central tiling engine managing window order, layout selection, and gaps.
pub struct TilingEngine {
    /// Active layout algorithm.
    layout: TilingLayout,
    /// Windows in tiling order. Index 0 is the master window.
    windows: Vec<WindowId>,
    /// Gap configuration.
    gaps: TilingGaps,
    /// Width/height ratio for the master area (0.1 to 0.9).
    master_ratio: f32,
    /// Number of master windows (1-3 for Columns/Rows, ignored by other layouts).
    master_count: usize,
    /// Index of the currently focused window (if any).
    focused: Option<usize>,
    /// Cached layout positions from the last `compute_layout` call.
    cached_positions: Vec<Rect>,
    /// Available layouts for cycling (excludes Float and Custom).
    layout_cycle: Vec<TilingLayout>,
    /// Index into `layout_cycle` for the current layout.
    cycle_index: usize,
}

impl TilingEngine {
    /// Create a new tiling engine with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layout: TilingLayout::Columns,
            windows: Vec::new(),
            gaps: TilingGaps::default(),
            master_ratio: 0.55,
            master_count: 1,
            focused: None,
            cached_positions: Vec::new(),
            layout_cycle: vec![
                TilingLayout::Columns,
                TilingLayout::Rows,
                TilingLayout::Grid,
                TilingLayout::ThreeColumn,
                TilingLayout::Spiral,
                TilingLayout::Monocle,
            ],
            cycle_index: 0,
        }
    }

    /// Create an engine with a specific layout, gaps, and master ratio.
    #[must_use]
    pub fn with_config(layout: TilingLayout, gaps: TilingGaps, master_ratio: f32) -> Self {
        let cycle_index = match &layout {
            TilingLayout::Columns => 0,
            TilingLayout::Rows => 1,
            TilingLayout::Grid => 2,
            TilingLayout::ThreeColumn => 3,
            TilingLayout::Spiral => 4,
            TilingLayout::Monocle => 5,
            _ => 0,
        };
        Self {
            layout,
            master_ratio: master_ratio.clamp(0.1, 0.9),
            gaps,
            cycle_index,
            ..Self::new()
        }
    }

    // -----------------------------------------------------------------------
    // Core operations
    // -----------------------------------------------------------------------

    /// Compute window positions for the current layout within the given
    /// work area. Returns one `(WindowId, Rect)` pair per tiled window.
    #[must_use]
    pub fn compute_layout(&mut self, work_area: Rect) -> Vec<(WindowId, Rect)> {
        let positions = algorithms::compute_layout(
            &self.layout,
            self.windows.len(),
            work_area,
            self.master_ratio,
            self.master_count,
            &self.gaps,
        );

        self.cached_positions = positions.clone();

        self.windows.iter().copied().zip(positions).collect()
    }

    /// Add a window to the end of the tiling set.
    pub fn add_window(&mut self, window_id: WindowId) {
        if !self.windows.contains(&window_id) {
            self.windows.push(window_id);
            if self.focused.is_none() {
                self.focused = Some(0);
            }
        }
    }

    /// Remove a window from the tiling set.
    pub fn remove_window(&mut self, window_id: WindowId) {
        if let Some(pos) = self.windows.iter().position(|&w| w == window_id) {
            self.windows.remove(pos);
            // Adjust focus index.
            match self.focused {
                Some(f) if f == pos => {
                    if self.windows.is_empty() {
                        self.focused = None;
                    } else {
                        self.focused = Some(f.min(self.windows.len() - 1));
                    }
                }
                Some(f) if f > pos => {
                    self.focused = Some(f - 1);
                }
                _ => {}
            }
        }
    }

    /// Swap two windows' positions in the tiling order.
    pub fn swap_windows(&mut self, a: WindowId, b: WindowId) {
        let pos_a = self.windows.iter().position(|&w| w == a);
        let pos_b = self.windows.iter().position(|&w| w == b);
        if let (Some(ia), Some(ib)) = (pos_a, pos_b) {
            self.windows.swap(ia, ib);
        }
    }

    /// Move a window to the master position (index 0).
    pub fn promote_to_master(&mut self, window_id: WindowId) {
        if let Some(pos) = self.windows.iter().position(|&w| w == window_id) {
            if pos == 0 {
                return;
            }
            let wid = self.windows.remove(pos);
            self.windows.insert(0, wid);
            // Update focus to follow the promoted window.
            self.focused = Some(0);
        }
    }

    /// Rotate all windows in the given direction.
    pub fn rotate_windows(&mut self, direction: RotateDir) {
        if self.windows.len() < 2 {
            return;
        }
        match direction {
            RotateDir::Forward => {
                let last = self.windows.pop().unwrap();
                self.windows.insert(0, last);
            }
            RotateDir::Backward => {
                let first = self.windows.remove(0);
                self.windows.push(first);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Layout adjustment
    // -----------------------------------------------------------------------

    /// Increase the master area ratio by `delta`.
    pub fn increase_master_ratio(&mut self, delta: f32) {
        self.master_ratio = (self.master_ratio + delta).clamp(0.1, 0.9);
    }

    /// Decrease the master area ratio by `delta`.
    pub fn decrease_master_ratio(&mut self, delta: f32) {
        self.master_ratio = (self.master_ratio - delta).clamp(0.1, 0.9);
    }

    /// Increment the master window count (max 3).
    pub fn increment_master_count(&mut self) {
        if self.master_count < 3 {
            self.master_count += 1;
        }
    }

    /// Decrement the master window count (min 1).
    pub fn decrement_master_count(&mut self) {
        if self.master_count > 1 {
            self.master_count -= 1;
        }
    }

    /// Set the active layout.
    pub fn set_layout(&mut self, layout: TilingLayout) {
        // Update cycle index if possible.
        if let Some(idx) = self.layout_cycle.iter().position(|l| l == &layout) {
            self.cycle_index = idx;
        }
        self.layout = layout;
    }

    /// Cycle to the next layout in the predefined cycle.
    pub fn cycle_layout(&mut self) {
        if self.layout_cycle.is_empty() {
            return;
        }
        self.cycle_index = (self.cycle_index + 1) % self.layout_cycle.len();
        self.layout = self.layout_cycle[self.cycle_index].clone();
    }

    // -----------------------------------------------------------------------
    // Navigation (keyboard-driven)
    // -----------------------------------------------------------------------

    /// Focus the next window in the tiling order (wraps around).
    /// Returns the newly focused window ID.
    #[must_use]
    pub fn focus_next(&mut self) -> Option<WindowId> {
        if self.windows.is_empty() {
            return None;
        }
        let current = self.focused.unwrap_or(0);
        let next = navigate::next_index(current, self.windows.len());
        self.focused = Some(next);
        Some(self.windows[next])
    }

    /// Focus the previous window in the tiling order (wraps around).
    #[must_use]
    pub fn focus_prev(&mut self) -> Option<WindowId> {
        if self.windows.is_empty() {
            return None;
        }
        let current = self.focused.unwrap_or(0);
        let prev = navigate::prev_index(current, self.windows.len());
        self.focused = Some(prev);
        Some(self.windows[prev])
    }

    /// Focus the master window (index 0).
    #[must_use]
    pub fn focus_master(&mut self) -> Option<WindowId> {
        if self.windows.is_empty() {
            return None;
        }
        self.focused = Some(0);
        Some(self.windows[0])
    }

    /// Focus the window in the given direction from the currently focused
    /// window, based on cached layout positions.
    #[must_use]
    pub fn focus_direction(&mut self, dir: Direction) -> Option<WindowId> {
        let current = self.focused?;
        if self.cached_positions.len() != self.windows.len() {
            return None;
        }

        let target_idx = navigate::find_index_in_direction(dir, current, &self.cached_positions)?;
        self.focused = Some(target_idx);
        Some(self.windows[target_idx])
    }

    /// Swap the given window with the window in the specified direction,
    /// based on cached layout positions.
    pub fn swap_direction(&mut self, window_id: WindowId, dir: Direction) {
        let origin_idx = match self.windows.iter().position(|&w| w == window_id) {
            Some(i) => i,
            None => return,
        };
        if self.cached_positions.len() != self.windows.len() {
            return;
        }

        if let Some(target_idx) =
            navigate::find_index_in_direction(dir, origin_idx, &self.cached_positions)
        {
            self.windows.swap(origin_idx, target_idx);
            // Keep focus on the original window (which is now at target_idx).
            if self.focused == Some(origin_idx) {
                self.focused = Some(target_idx);
            } else if self.focused == Some(target_idx) {
                self.focused = Some(origin_idx);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// The active layout.
    #[must_use]
    pub fn layout(&self) -> &TilingLayout {
        &self.layout
    }

    /// The current window list in tiling order.
    #[must_use]
    pub fn windows(&self) -> &[WindowId] {
        &self.windows
    }

    /// The current gap configuration.
    #[must_use]
    pub fn gaps(&self) -> &TilingGaps {
        &self.gaps
    }

    /// Set the gap configuration.
    pub fn set_gaps(&mut self, gaps: TilingGaps) {
        self.gaps = gaps;
    }

    /// The current master ratio.
    #[must_use]
    pub fn master_ratio(&self) -> f32 {
        self.master_ratio
    }

    /// The current master count.
    #[must_use]
    pub fn master_count(&self) -> usize {
        self.master_count
    }

    /// The currently focused window index.
    #[must_use]
    pub fn focused_index(&self) -> Option<usize> {
        self.focused
    }

    /// The currently focused window ID.
    #[must_use]
    pub fn focused_window(&self) -> Option<WindowId> {
        self.focused.map(|i| self.windows[i])
    }

    /// Set focus to a specific window ID.
    pub fn set_focused(&mut self, window_id: WindowId) {
        if let Some(idx) = self.windows.iter().position(|&w| w == window_id) {
            self.focused = Some(idx);
        }
    }

    /// Number of tiled windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Whether the engine has any windows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// The cached positions from the last `compute_layout` call.
    #[must_use]
    pub fn cached_positions(&self) -> &[Rect] {
        &self.cached_positions
    }
}

impl Default for TilingEngine {
    fn default() -> Self {
        Self::new()
    }
}
