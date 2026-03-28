//! Window stacking order management.
//!
//! Manages the z-order of windows across multiple layers (desktop, below,
//! normal, above, notification, overlay, fullscreen). Within each layer,
//! windows are sorted by last-raised timestamp.

use std::collections::HashMap;

/// The stacking layer a window belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StackLayer {
    /// Desktop background windows (e.g., desktop icons, wallpaper widgets).
    Desktop = 0,
    /// Windows explicitly set to be below normal windows.
    Below = 1,
    /// Normal application windows (default).
    Normal = 2,
    /// Windows explicitly set to be above normal (always-on-top).
    Above = 3,
    /// Notification popups.
    Notification = 4,
    /// Overlay elements (e.g., on-screen display, screenshot selection).
    Overlay = 5,
    /// Fullscreen windows.
    Fullscreen = 6,
}

impl Default for StackLayer {
    fn default() -> Self {
        Self::Normal
    }
}

/// Internal entry tracking a window's stacking state.
#[derive(Debug, Clone)]
struct StackEntry {
    layer: StackLayer,
    /// Monotonic counter value when this window was last raised.
    raised_at: u64,
    /// Window bounds for hit testing.
    bounds: (i32, i32, u32, u32),
}

/// Manages the z-order of windows across stacking layers.
#[derive(Debug)]
pub struct StackingOrder {
    /// All window entries, keyed by window_id.
    entries: HashMap<u64, StackEntry>,
    /// Sorted list of window IDs in stacking order (bottom to top).
    order: Vec<u64>,
    /// Monotonic counter for raise timestamps.
    counter: u64,
    /// Whether the order needs re-sorting.
    dirty: bool,
}

impl Default for StackingOrder {
    fn default() -> Self {
        Self::new()
    }
}

impl StackingOrder {
    /// Create an empty stacking order.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::new(),
            counter: 0,
            dirty: false,
        }
    }

    /// Add a window to the stacking order at the given layer.
    /// The window is placed at the top of its layer.
    pub fn add(&mut self, window_id: u64, layer: StackLayer, bounds: (i32, i32, u32, u32)) {
        self.counter += 1;
        let entry = StackEntry {
            layer,
            raised_at: self.counter,
            bounds,
        };
        self.entries.insert(window_id, entry);
        self.order.push(window_id);
        self.dirty = true;
    }

    /// Remove a window from the stacking order.
    pub fn remove(&mut self, window_id: u64) -> bool {
        if self.entries.remove(&window_id).is_some() {
            self.order.retain(|&id| id != window_id);
            true
        } else {
            false
        }
    }

    /// Raise a window to the top of its layer.
    pub fn raise(&mut self, window_id: u64) -> bool {
        if let Some(entry) = self.entries.get_mut(&window_id) {
            self.counter += 1;
            entry.raised_at = self.counter;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Lower a window to the bottom of its layer.
    pub fn lower(&mut self, window_id: u64) -> bool {
        if let Some(entry) = self.entries.get_mut(&window_id) {
            // Set raised_at to 0 so it sorts below everything else in its layer.
            entry.raised_at = 0;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Raise a window just above a sibling (both must be in the same layer).
    /// Returns false if either window doesn't exist or they are in different layers.
    pub fn raise_above(&mut self, window_id: u64, sibling_id: u64) -> bool {
        let sibling_raised = match self.entries.get(&sibling_id) {
            Some(e) => (e.layer, e.raised_at),
            None => return false,
        };
        let entry = match self.entries.get_mut(&window_id) {
            Some(e) => e,
            None => return false,
        };
        if entry.layer != sibling_raised.0 {
            return false;
        }
        entry.raised_at = sibling_raised.1 + 1;
        // Bump counter if needed.
        if entry.raised_at > self.counter {
            self.counter = entry.raised_at;
        }
        self.dirty = true;
        true
    }

    /// Lower a window just below a sibling (both must be in the same layer).
    /// Returns false if either window doesn't exist or they are in different layers.
    pub fn lower_below(&mut self, window_id: u64, sibling_id: u64) -> bool {
        let sibling_raised = match self.entries.get(&sibling_id) {
            Some(e) => (e.layer, e.raised_at),
            None => return false,
        };
        let entry = match self.entries.get_mut(&window_id) {
            Some(e) => e,
            None => return false,
        };
        if entry.layer != sibling_raised.0 {
            return false;
        }
        entry.raised_at = sibling_raised.1.saturating_sub(1);
        self.dirty = true;
        true
    }

    /// Move a window to a different stacking layer.
    pub fn set_layer(&mut self, window_id: u64, layer: StackLayer) -> bool {
        if let Some(entry) = self.entries.get_mut(&window_id) {
            if entry.layer != layer {
                entry.layer = layer;
                self.counter += 1;
                entry.raised_at = self.counter;
                self.dirty = true;
            }
            true
        } else {
            false
        }
    }

    /// Update a window's bounds (for hit testing).
    pub fn update_bounds(&mut self, window_id: u64, bounds: (i32, i32, u32, u32)) -> bool {
        if let Some(entry) = self.entries.get_mut(&window_id) {
            entry.bounds = bounds;
            true
        } else {
            false
        }
    }

    /// Get the layer of a window.
    pub fn get_layer(&self, window_id: u64) -> Option<StackLayer> {
        self.entries.get(&window_id).map(|e| e.layer)
    }

    /// Ensure the internal order is sorted.
    fn ensure_sorted(&mut self) {
        if !self.dirty {
            return;
        }
        let entries = &self.entries;
        self.order.sort_by(|&a, &b| {
            let ea = &entries[&a];
            let eb = &entries[&b];
            ea.layer
                .cmp(&eb.layer)
                .then(ea.raised_at.cmp(&eb.raised_at))
        });
        self.dirty = false;
    }

    /// Re-sort all windows according to their layer and raise timestamp.
    pub fn restack(&mut self) {
        self.dirty = true;
        self.ensure_sorted();
    }

    /// Returns windows in stacking order (bottom to top).
    pub fn iter_bottom_to_top(&mut self) -> Vec<u64> {
        self.ensure_sorted();
        self.order.clone()
    }

    /// Returns windows in stacking order (top to bottom).
    pub fn iter_top_to_bottom(&mut self) -> Vec<u64> {
        self.ensure_sorted();
        let mut result = self.order.clone();
        result.reverse();
        result
    }

    /// Hit test: find all windows at the given point, returned in
    /// top-to-bottom order (topmost window first).
    pub fn windows_at_point(&mut self, x: i32, y: i32) -> Vec<u64> {
        self.ensure_sorted();
        let mut result = Vec::new();
        for &wid in self.order.iter().rev() {
            if let Some(entry) = self.entries.get(&wid) {
                let (bx, by, bw, bh) = entry.bounds;
                if x >= bx && x < bx + bw as i32 && y >= by && y < by + bh as i32 {
                    result.push(wid);
                }
            }
        }
        result
    }

    /// Returns the topmost window (across all layers).
    pub fn topmost(&mut self) -> Option<u64> {
        self.ensure_sorted();
        self.order.last().copied()
    }

    /// Returns the topmost window in a specific layer.
    pub fn topmost_in_layer(&mut self, layer: StackLayer) -> Option<u64> {
        self.ensure_sorted();
        self.order
            .iter()
            .rev()
            .find(|&&id| self.entries.get(&id).is_some_and(|e| e.layer == layer))
            .copied()
    }

    /// Returns all windows in a given layer, sorted bottom-to-top.
    pub fn windows_in_layer(&mut self, layer: StackLayer) -> Vec<u64> {
        self.ensure_sorted();
        self.order
            .iter()
            .filter(|&&id| self.entries.get(&id).is_some_and(|e| e.layer == layer))
            .copied()
            .collect()
    }

    /// Returns the total number of managed windows.
    pub fn window_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the given window is being tracked.
    pub fn contains(&self, window_id: u64) -> bool {
        self.entries.contains_key(&window_id)
    }
}
