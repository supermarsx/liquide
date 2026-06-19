//! Batched window operations — apply multiple move/resize/z-order changes
//! atomically with a single dirty-flag pass.
//!
//! This is the single most important optimisation for tiling relayout,
//! workspace switching, and snap operations: instead of N individual
//! `move_window` / `resize_window` calls each marking the shell dirty, a
//! [`WindowBatch`] collects all operations, coalesces redundant ones, and
//! applies them in one shot.
//!
//! ## NT-inspired enhancements
//!
//! The batch system draws inspiration from NT's `SetMultipleWindowPos` (SMWP)
//! pattern:
//!
//! - **[`CachedBatch`]**: A reusable batch stored on the Shell (analogous to
//!   NT's `gSMWP`) that avoids allocation for the common single-window case.
//! - **[`ValidRect`] / `compute_valid_rects`**: When windows move, compute
//!   which pixels can be bit-copied vs. invalidated (NT's `CalcValidRects`).
//! - **`validate_z_order`**: Check if a window is already in the correct
//!   z-position before relinking (NT's `ValidateZorder`).
//! - **[`BatchStats`]**: Statistics on batch optimisation and cache reuse.

use std::collections::HashMap;

use liquide_compositor::geometry::Rect;

use crate::window::WindowId;

use super::Shell;

// ---------------------------------------------------------------------------
// WindowOp / ZOrderOp
// ---------------------------------------------------------------------------

/// A deferred window operation.
#[derive(Debug, Clone)]
pub enum WindowOp {
    /// Move a window to a new position.
    Move { id: WindowId, x: f32, y: f32 },
    /// Resize a window.
    Resize {
        id: WindowId,
        width: f32,
        height: f32,
    },
    /// Move and resize a window in one operation.
    MoveResize {
        id: WindowId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    /// Change a window's z-order.
    SetZOrder { id: WindowId, position: ZOrderOp },
    /// Minimize a window.
    Minimize { id: WindowId },
    /// Maximize a window.
    Maximize { id: WindowId },
    /// Restore a window from minimized / maximized / fullscreen.
    Restore { id: WindowId },
    /// Show a hidden window.
    Show { id: WindowId },
    /// Hide a window (without changing state).
    Hide { id: WindowId },
    /// Set the window title.
    SetTitle { id: WindowId, title: String },
    /// Close a window.
    Close { id: WindowId },
}

/// Z-order placement directive.
#[derive(Debug, Clone, Copy)]
pub enum ZOrderOp {
    /// Move to the top of the z-order stack.
    Top,
    /// Move to the bottom of the z-order stack.
    Bottom,
    /// Place immediately above the given window.
    Above(WindowId),
    /// Place immediately below the given window.
    Below(WindowId),
}

// ---------------------------------------------------------------------------
// WindowBatch
// ---------------------------------------------------------------------------

/// A batch of window operations to apply atomically.
///
/// Build a batch using the convenience methods, then pass it to
/// [`Shell::apply_batch`] to execute all operations with a single
/// dirty-flag update.
pub struct WindowBatch {
    ops: Vec<WindowOp>,
}

impl WindowBatch {
    /// Create an empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    /// Create an empty batch with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            ops: Vec::with_capacity(cap),
        }
    }

    /// Push a raw operation.
    pub fn push(&mut self, op: WindowOp) {
        self.ops.push(op);
    }

    // -- Convenience builders -----------------------------------------------

    /// Enqueue a move.
    pub fn move_window(&mut self, id: WindowId, x: f32, y: f32) {
        self.ops.push(WindowOp::Move { id, x, y });
    }

    /// Enqueue a resize.
    pub fn resize_window(&mut self, id: WindowId, width: f32, height: f32) {
        self.ops.push(WindowOp::Resize { id, width, height });
    }

    /// Enqueue a combined move + resize.
    pub fn move_resize(&mut self, id: WindowId, x: f32, y: f32, width: f32, height: f32) {
        self.ops.push(WindowOp::MoveResize {
            id,
            x,
            y,
            width,
            height,
        });
    }

    /// Enqueue a raise to top.
    pub fn raise(&mut self, id: WindowId) {
        self.ops.push(WindowOp::SetZOrder {
            id,
            position: ZOrderOp::Top,
        });
    }

    /// Enqueue a lower to bottom.
    pub fn lower(&mut self, id: WindowId) {
        self.ops.push(WindowOp::SetZOrder {
            id,
            position: ZOrderOp::Bottom,
        });
    }

    /// Enqueue minimize.
    pub fn minimize(&mut self, id: WindowId) {
        self.ops.push(WindowOp::Minimize { id });
    }

    /// Enqueue maximize.
    pub fn maximize(&mut self, id: WindowId) {
        self.ops.push(WindowOp::Maximize { id });
    }

    /// Enqueue restore.
    pub fn restore(&mut self, id: WindowId) {
        self.ops.push(WindowOp::Restore { id });
    }

    /// Enqueue show.
    pub fn show(&mut self, id: WindowId) {
        self.ops.push(WindowOp::Show { id });
    }

    /// Enqueue hide.
    pub fn hide(&mut self, id: WindowId) {
        self.ops.push(WindowOp::Hide { id });
    }

    /// Enqueue a title change.
    pub fn set_title(&mut self, id: WindowId, title: impl Into<String>) {
        self.ops.push(WindowOp::SetTitle {
            id,
            title: title.into(),
        });
    }

    /// Enqueue close.
    pub fn close(&mut self, id: WindowId) {
        self.ops.push(WindowOp::Close { id });
    }

    // -- Introspection ------------------------------------------------------

    /// Number of operations in this batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the batch contains no operations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Borrow the operations slice.
    #[must_use]
    pub fn ops(&self) -> &[WindowOp] {
        &self.ops
    }

    // -- Workspace batch operations -----------------------------------------

    /// Enqueue operations for a workspace switch: hide all windows in the old
    /// workspace and show all windows in the new one.
    ///
    /// This is the most common batch shape in workspace transitions and maps
    /// naturally to a single `apply_batch` call for flicker-free switching.
    pub fn workspace_switch(&mut self, hide_windows: &[WindowId], show_windows: &[WindowId]) {
        self.ops.reserve(hide_windows.len() + show_windows.len());
        for &id in hide_windows {
            self.ops.push(WindowOp::Hide { id });
        }
        for &id in show_windows {
            self.ops.push(WindowOp::Show { id });
        }
    }

    /// Enqueue move-resize operations for a pre-computed tiling layout.
    ///
    /// Each `(WindowId, Rect)` pair maps a window to its tiled bounds.
    pub fn tile_layout(&mut self, layout: &[(WindowId, Rect)]) {
        self.ops.reserve(layout.len());
        for &(id, rect) in layout {
            self.ops.push(WindowOp::MoveResize {
                id,
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            });
        }
    }

    /// Enqueue cascaded window positions: each successive window is offset
    /// from the previous by `(offset, offset)` starting at `start`.
    ///
    /// Window sizes are not changed — only positions.
    pub fn cascade_windows(&mut self, windows: &[WindowId], offset: f32, start: (f32, f32)) {
        self.ops.reserve(windows.len());
        for (i, &id) in windows.iter().enumerate() {
            let x = start.0 + offset * i as f32;
            let y = start.1 + offset * i as f32;
            self.ops.push(WindowOp::Move { id, x, y });
        }
    }

    /// Enqueue operations to minimize a group of windows toward a common
    /// target rectangle (e.g. a dock icon or taskbar slot).
    ///
    /// Each window is first move-resized to `target` then minimized so the
    /// compositor can animate the shrink.
    pub fn stack_minimize(&mut self, windows: &[WindowId], target: Rect) {
        self.ops.reserve(windows.len() * 2);
        for &id in windows {
            self.ops.push(WindowOp::MoveResize {
                id,
                x: target.x,
                y: target.y,
                width: target.width,
                height: target.height,
            });
            self.ops.push(WindowOp::Minimize { id });
        }
    }

    /// Clear all operations, retaining the underlying allocation.
    pub fn clear(&mut self) {
        self.ops.clear();
    }

    /// Current Vec capacity (useful for cache sizing decisions).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.ops.capacity()
    }

    // -- Damage computation -------------------------------------------------

    /// Compute the damage region produced by the geometric ops in this batch
    /// given the current window bounds.
    ///
    /// For each window affected by a Move / Resize / MoveResize, the damage
    /// is the union of old bounds and new bounds.  Non-geometric ops (show,
    /// hide, minimize, etc.) contribute the current bounds of the affected
    /// window.
    ///
    /// Returns a list of axis-aligned damage rectangles — callers typically
    /// union-merge them into a region for compositor invalidation.
    pub fn compute_damage(&self, current_bounds: &HashMap<WindowId, Rect>) -> Vec<Rect> {
        let mut damage: Vec<Rect> = Vec::new();
        for op in &self.ops {
            match op {
                WindowOp::Move { id, x, y } => {
                    if let Some(&old) = current_bounds.get(id) {
                        damage.push(old);
                        damage.push(Rect::new(*x, *y, old.width, old.height));
                    }
                }
                WindowOp::Resize { id, width, height } => {
                    if let Some(&old) = current_bounds.get(id) {
                        damage.push(old);
                        damage.push(Rect::new(old.x, old.y, *width, *height));
                    }
                }
                WindowOp::MoveResize {
                    id,
                    x,
                    y,
                    width,
                    height,
                } => {
                    if let Some(&old) = current_bounds.get(id) {
                        damage.push(old);
                        damage.push(Rect::new(*x, *y, *width, *height));
                    }
                }
                WindowOp::Show { id }
                | WindowOp::Hide { id }
                | WindowOp::Minimize { id }
                | WindowOp::Maximize { id }
                | WindowOp::Restore { id }
                | WindowOp::Close { id } => {
                    if let Some(&bounds) = current_bounds.get(id) {
                        damage.push(bounds);
                    }
                }
                // Z-order and title changes don't move pixels.
                WindowOp::SetZOrder { .. } | WindowOp::SetTitle { .. } => {}
            }
        }
        damage
    }

    // -- Optimise -----------------------------------------------------------

    /// Coalesce redundant position / size operations on the same window.
    ///
    /// * Multiple `Move`s on the same window keep only the last position.
    /// * `Move` + `Resize` on the same window fuse into a single `MoveResize`.
    /// * Multiple `MoveResize`s keep only the last.
    /// * Non-geometric operations (minimize, close, etc.) are preserved as-is
    ///   in their original relative order.
    pub fn optimize(&mut self) {
        let mut last_pos: HashMap<WindowId, (Option<(f32, f32)>, Option<(f32, f32)>)> =
            HashMap::new();
        let mut other_ops: Vec<WindowOp> = Vec::new();

        for op in self.ops.drain(..) {
            match op {
                WindowOp::Move { id, x, y } => {
                    let entry = last_pos.entry(id).or_insert((None, None));
                    entry.0 = Some((x, y));
                }
                WindowOp::Resize { id, width, height } => {
                    let entry = last_pos.entry(id).or_insert((None, None));
                    entry.1 = Some((width, height));
                }
                WindowOp::MoveResize {
                    id,
                    x,
                    y,
                    width,
                    height,
                } => {
                    let entry = last_pos.entry(id).or_insert((None, None));
                    entry.0 = Some((x, y));
                    entry.1 = Some((width, height));
                }
                other => other_ops.push(other),
            }
        }

        // Emit coalesced move/resize ops (deterministic order by window id).
        let mut sorted_ids: Vec<WindowId> = last_pos.keys().copied().collect();
        sorted_ids.sort_by_key(|id| id.0);
        for id in sorted_ids {
            let (pos, size) = last_pos[&id];
            match (pos, size) {
                (Some((x, y)), Some((w, h))) => {
                    self.ops.push(WindowOp::MoveResize {
                        id,
                        x,
                        y,
                        width: w,
                        height: h,
                    });
                }
                (Some((x, y)), None) => {
                    self.ops.push(WindowOp::Move { id, x, y });
                }
                (None, Some((w, h))) => {
                    self.ops.push(WindowOp::Resize {
                        id,
                        width: w,
                        height: h,
                    });
                }
                (None, None) => {}
            }
        }

        // Append non-position ops in their original order.
        self.ops.extend(other_ops);
    }
}

impl Default for WindowBatch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CachedBatch — NT gSMWP pattern
// ---------------------------------------------------------------------------

/// A reusable [`WindowBatch`] that avoids per-frame allocation.
///
/// Modelled after NT's global `gSMWP` — the shell keeps one of these and
/// hands it out via [`CachedBatch::acquire`].  When the caller is done it
/// calls [`CachedBatch::release`] to return the batch to the cache.  If the
/// cached batch is already in use (re-entrant or concurrent), `acquire`
/// allocates a fresh one.
pub struct CachedBatch {
    inner: WindowBatch,
    in_use: bool,
    /// How many times the cached batch was reused instead of allocating.
    reuse_count: u64,
}

/// Maximum capacity to retain in the cache.  If the returned batch grew
/// beyond this we shrink it to avoid holding excess memory across frames.
const CACHED_BATCH_MAX_CAPACITY: usize = 8;

impl CachedBatch {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: WindowBatch::new(),
            in_use: false,
            reuse_count: 0,
        }
    }

    /// Acquire a batch.  Returns the cached batch if it is not already in use;
    /// otherwise allocates a fresh one.
    ///
    /// Callers **must** call [`release`](Self::release) when done, passing the
    /// batch back.
    pub fn acquire(&mut self) -> WindowBatch {
        if self.in_use {
            // Re-entrant path — allocate a throwaway batch.
            return WindowBatch::new();
        }
        self.in_use = true;
        self.reuse_count += 1;
        // Take the inner batch by swapping in an empty one.
        std::mem::take(&mut self.inner)
    }

    /// Return a batch to the cache for future reuse.
    ///
    /// If the batch's capacity exceeds [`CACHED_BATCH_MAX_CAPACITY`] it is
    /// shrunk to avoid holding excess memory.
    pub fn release(&mut self, mut batch: WindowBatch) {
        batch.clear();
        if batch.capacity() > CACHED_BATCH_MAX_CAPACITY {
            // Shrink by replacing with a right-sized allocation.
            batch = WindowBatch::with_capacity(CACHED_BATCH_MAX_CAPACITY);
        }
        self.inner = batch;
        self.in_use = false;
    }

    /// Whether the cached batch is currently checked out.
    #[must_use]
    pub fn is_in_use(&self) -> bool {
        self.in_use
    }

    /// Number of times `acquire` reused the cached allocation.
    #[must_use]
    pub fn reuse_count(&self) -> u64 {
        self.reuse_count
    }
}

impl Default for CachedBatch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ValidRect — NT CalcValidRects pattern
// ---------------------------------------------------------------------------

/// Describes a region of a window's pixel content that can be preserved via
/// a blit (memcpy) after the window moves, and the regions that must be
/// repainted.
///
/// Inspired by NT's `CalcValidRects` which determines, for each window in a
/// `SetMultipleWindowPos` batch, which pixels are still valid after the
/// position change and can be copied rather than redrawn.
#[derive(Debug, Clone)]
pub struct ValidRect {
    /// The window this applies to.
    pub window_id: WindowId,
    /// The rectangle (in new-position coordinates) whose pixels can be
    /// copied from the old framebuffer position.
    pub blit_rect: Rect,
    /// Horizontal copy offset: `new_x - old_x`.
    pub dx: f32,
    /// Vertical copy offset: `new_y - old_y`.
    pub dy: f32,
    /// Regions within the new bounds that are NOT covered by `blit_rect`
    /// and must be redrawn.
    pub invalidated: Vec<Rect>,
}

/// Compute valid-rect information for each geometric operation in a batch.
///
/// For each Move / MoveResize in the batch, if the window's old bounds
/// (from `current_bounds`) overlap with its new bounds, the overlapping
/// region is the `blit_rect` — those pixels can be memcpy'd.  The
/// remaining strips of the new bounds form the `invalidated` list.
///
/// Pure resizes (no position change) where the new size is larger get a
/// valid rect covering the old size and invalidation rects for the new
/// strips.  Pure resizes that shrink get a valid rect covering the new
/// (smaller) size with no invalidation.
pub fn compute_valid_rects(
    batch: &WindowBatch,
    current_bounds: &HashMap<WindowId, Rect>,
) -> Vec<ValidRect> {
    let mut results = Vec::new();

    for op in batch.ops() {
        let (id, new_bounds) = match op {
            WindowOp::Move { id, x, y } => {
                if let Some(&old) = current_bounds.get(id) {
                    (*id, Rect::new(*x, *y, old.width, old.height))
                } else {
                    continue;
                }
            }
            WindowOp::MoveResize {
                id,
                x,
                y,
                width,
                height,
            } => (*id, Rect::new(*x, *y, *width, *height)),
            WindowOp::Resize { id, width, height } => {
                if let Some(&old) = current_bounds.get(id) {
                    (*id, Rect::new(old.x, old.y, *width, *height))
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        let old_bounds = match current_bounds.get(&id) {
            Some(&b) => b,
            None => continue,
        };

        let dx = new_bounds.x - old_bounds.x;
        let dy = new_bounds.y - old_bounds.y;

        // The valid region is the intersection of old and new bounds,
        // expressed in new-position coordinates.
        if let Some(overlap) = old_bounds.intersection(&new_bounds) {
            // The blit_rect is the overlap in new-position space.
            let blit_rect = overlap;

            // Compute invalidated strips: parts of new_bounds not covered
            // by the overlap.
            let mut invalidated = Vec::new();

            // Top strip (above blit_rect).
            if blit_rect.y > new_bounds.y {
                invalidated.push(Rect::new(
                    new_bounds.x,
                    new_bounds.y,
                    new_bounds.width,
                    blit_rect.y - new_bounds.y,
                ));
            }
            // Bottom strip (below blit_rect).
            if blit_rect.bottom() < new_bounds.bottom() {
                invalidated.push(Rect::new(
                    new_bounds.x,
                    blit_rect.bottom(),
                    new_bounds.width,
                    new_bounds.bottom() - blit_rect.bottom(),
                ));
            }
            // Left strip (to the left of blit_rect, between top/bottom strips).
            if blit_rect.x > new_bounds.x {
                invalidated.push(Rect::new(
                    new_bounds.x,
                    blit_rect.y,
                    blit_rect.x - new_bounds.x,
                    blit_rect.height,
                ));
            }
            // Right strip (to the right of blit_rect, between top/bottom strips).
            if blit_rect.right() < new_bounds.right() {
                invalidated.push(Rect::new(
                    blit_rect.right(),
                    blit_rect.y,
                    new_bounds.right() - blit_rect.right(),
                    blit_rect.height,
                ));
            }

            results.push(ValidRect {
                window_id: id,
                blit_rect,
                dx,
                dy,
                invalidated,
            });
        } else {
            // No overlap — the entire new bounds must be redrawn.
            results.push(ValidRect {
                window_id: id,
                blit_rect: Rect::ZERO,
                dx,
                dy,
                invalidated: vec![new_bounds],
            });
        }
    }

    results
}

/// Pure-geometry valid-rect result for a SINGLE window move (t164-blit-move).
///
/// Unlike [`ValidRect`] (which is computed per-op over a whole
/// [`WindowBatch`]), this describes one window translating from `old` to `new`
/// and additionally exposes the OLD footprint the window UNCOVERED (the part of
/// the old bounds the new bounds no longer occupies) — the background that must
/// be repainted so the move leaves no ghost of the old position. It carries no
/// `WindowId` and reads no shell state, so the render worker can call it
/// directly to drive a blit-move.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveValidRect {
    /// The overlap of `old` and `new`, in NEW-position coordinates — the pixels
    /// that can be copied (blitted) from the old framebuffer position.
    pub blit_rect: Rect,
    /// Horizontal copy offset: `new.x - old.x`.
    pub dx: f32,
    /// Vertical copy offset: `new.y - old.y`.
    pub dy: f32,
    /// The L-shaped strips of `new` NOT covered by `blit_rect` — the
    /// newly-revealed leading edges that must be freshly rastered.
    pub new_strips: Vec<Rect>,
    /// The parts of `old` NOT covered by `new` — the background the window
    /// uncovered, which must be repainted (else a ghost of the old position
    /// survives).
    pub old_uncovered: Vec<Rect>,
}

/// Compute the [`MoveValidRect`] for a single window translating from `old` to
/// `new` (same size; a pure move). Pure geometry, no shell/window state.
///
/// `blit_rect` is `old ∩ new` (the rigidly-translatable pixels). `new_strips`
/// are the parts of `new` outside the overlap (using the exact same four-strip
/// decomposition as [`compute_valid_rects`]); `old_uncovered` is `old` minus
/// `new` (the revealed background). When `old` and `new` do not overlap at all,
/// `blit_rect` is [`Rect::ZERO`], all of `new` is a strip, and all of `old` is
/// uncovered — the caller then has no blit benefit and should fall back.
#[must_use]
pub fn compute_move_valid_rect(old: Rect, new: Rect) -> MoveValidRect {
    let dx = new.x - old.x;
    let dy = new.y - old.y;

    let Some(blit_rect) = old.intersection(&new) else {
        return MoveValidRect {
            blit_rect: Rect::ZERO,
            dx,
            dy,
            new_strips: vec![new],
            old_uncovered: vec![old],
        };
    };

    let new_strips = rect_minus(new, blit_rect);
    // The uncovered old footprint is `old` minus `new` (everything the window
    // used to cover that it no longer does).
    let old_uncovered = rect_minus(old, new);

    MoveValidRect {
        blit_rect,
        dx,
        dy,
        new_strips,
        old_uncovered,
    }
}

/// Subtract `cut` from `frag`, returning up to four axis-aligned remainder
/// rects (the parts of `frag` not covered by `cut`). Mirrors the renderer's
/// `subtract_rect` (occlusion.rs) so the two stay consistent. Empty when `cut`
/// fully covers `frag`; `[frag]` when they do not overlap.
fn rect_minus(frag: Rect, cut: Rect) -> Vec<Rect> {
    let mut out = Vec::new();
    let Some(overlap) = frag.intersection(&cut) else {
        out.push(frag);
        return out;
    };

    let fx0 = frag.x;
    let fy0 = frag.y;
    let fx1 = frag.right();
    let fy1 = frag.bottom();
    let ox0 = overlap.x;
    let oy0 = overlap.y;
    let ox1 = overlap.right();
    let oy1 = overlap.bottom();

    // Top strip (full width, above the overlap).
    if oy0 > fy0 {
        out.push(Rect::new(fx0, fy0, fx1 - fx0, oy0 - fy0));
    }
    // Bottom strip (full width, below the overlap).
    if oy1 < fy1 {
        out.push(Rect::new(fx0, oy1, fx1 - fx0, fy1 - oy1));
    }
    // Left strip (only the overlap's vertical extent).
    if ox0 > fx0 {
        out.push(Rect::new(fx0, oy0, ox0 - fx0, oy1 - oy0));
    }
    // Right strip (only the overlap's vertical extent).
    if ox1 < fx1 {
        out.push(Rect::new(ox1, oy0, fx1 - ox1, oy1 - oy0));
    }
    out
}

// ---------------------------------------------------------------------------
// Z-order validation — NT ValidateZorder pattern
// ---------------------------------------------------------------------------

/// Check whether `window_id` is already at the position described by
/// `target` in the given z-order list.
///
/// Returns `true` if the window is already in the correct z-position,
/// meaning the z-order relink can be skipped (no-op optimisation).
///
/// `current_order` is a slice of window IDs sorted by ascending z-order
/// (bottom to top).
#[must_use]
pub fn validate_z_order(
    window_id: WindowId,
    target: &ZOrderOp,
    current_order: &[WindowId],
) -> bool {
    let pos = current_order.iter().position(|&id| id == window_id);
    let pos = match pos {
        Some(p) => p,
        None => return false, // window not in the list at all
    };

    match target {
        ZOrderOp::Top => pos == current_order.len() - 1,
        ZOrderOp::Bottom => pos == 0,
        ZOrderOp::Above(ref_id) => {
            let ref_pos = current_order.iter().position(|&id| id == *ref_id);
            match ref_pos {
                Some(rp) => pos == rp + 1,
                None => false,
            }
        }
        ZOrderOp::Below(ref_id) => {
            let ref_pos = current_order.iter().position(|&id| id == *ref_id);
            match ref_pos {
                Some(rp) => rp > 0 && pos == rp - 1,
                None => false,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BatchStats
// ---------------------------------------------------------------------------

/// Statistics collected during batch optimisation and application.
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    /// Number of operations submitted before optimisation.
    pub ops_submitted: usize,
    /// Number of operations after optimisation (coalescing).
    pub ops_after_optimize: usize,
    /// Number of z-order operations that were already in the correct
    /// position and were skipped.
    pub z_order_skipped: usize,
    /// Number of valid-rect blit regions computed (windows whose pixels
    /// can be memcpy'd instead of redrawn).
    pub valid_rect_blits: usize,
    /// Lifetime reuse count of the [`CachedBatch`].
    pub cache_reuses: u64,
}

// ---------------------------------------------------------------------------
// Shell::apply_batch
// ---------------------------------------------------------------------------

impl Shell {
    /// Apply a batch of window operations atomically.
    ///
    /// The batch is first optimised (redundant move/resize ops coalesced),
    /// then each operation is applied against the window map.  The DOM dirty
    /// flag is set **once** at the end rather than per-operation, making this
    /// significantly cheaper than calling individual shell methods in a loop.
    pub fn apply_batch(&mut self, mut batch: WindowBatch) {
        if batch.is_empty() {
            return;
        }
        batch.optimize();
        let mut window_scene_changed = false;

        for op in batch.ops() {
            match op {
                WindowOp::Move { id, x, y } => {
                    if let Some(win) = self.windows.get_mut(id) {
                        win.bounds.x = *x;
                        win.bounds.y = *y;
                        window_scene_changed = true;
                    }
                }
                WindowOp::Resize { id, width, height } => {
                    if let Some(win) = self.windows.get_mut(id) {
                        win.bounds.width = *width;
                        win.bounds.height = *height;
                        window_scene_changed = true;
                    }
                }
                WindowOp::MoveResize {
                    id,
                    x,
                    y,
                    width,
                    height,
                } => {
                    if let Some(win) = self.windows.get_mut(id) {
                        win.bounds.x = *x;
                        win.bounds.y = *y;
                        win.bounds.width = *width;
                        win.bounds.height = *height;
                        window_scene_changed = true;
                    }
                }
                WindowOp::SetZOrder { id, position } => {
                    match position {
                        ZOrderOp::Top => {
                            if self.raise_window(*id).is_ok() {
                                window_scene_changed = true;
                            }
                        }
                        ZOrderOp::Bottom => {
                            if self.lower_window(*id).is_ok() {
                                window_scene_changed = true;
                            }
                        }
                        ZOrderOp::Above(ref_id) => {
                            // Place `id` just above `ref_id`.
                            let ref_z = self.windows.get(ref_id).map(|w| w.z_order).unwrap_or(0);
                            if let Some(win) = self.windows.get_mut(id) {
                                win.z_order = ref_z + 1;
                                window_scene_changed = true;
                            }
                            self.normalize_z_orders();
                        }
                        ZOrderOp::Below(ref_id) => {
                            // Place `id` just below `ref_id`.
                            let ref_z = self.windows.get(ref_id).map(|w| w.z_order).unwrap_or(0);
                            if let Some(win) = self.windows.get_mut(id) {
                                win.z_order = ref_z - 1;
                                window_scene_changed = true;
                            }
                            self.normalize_z_orders();
                        }
                    }
                }
                WindowOp::Minimize { id } => {
                    if self.minimize(*id).is_ok() {
                        window_scene_changed = true;
                    }
                }
                WindowOp::Maximize { id } => {
                    if self.maximize(*id).is_ok() {
                        window_scene_changed = true;
                    }
                }
                WindowOp::Restore { id } => {
                    if self.restore(*id).is_ok() {
                        window_scene_changed = true;
                    }
                }
                WindowOp::Show { id } => {
                    if let Some(win) = self.windows.get_mut(id) {
                        win.visible = true;
                        window_scene_changed = true;
                    }
                }
                WindowOp::Hide { id } => {
                    if let Some(win) = self.windows.get_mut(id) {
                        win.visible = false;
                        window_scene_changed = true;
                    }
                }
                WindowOp::SetTitle { id, title } => {
                    if let Some(win) = self.windows.get_mut(id) {
                        win.title.clone_from(title);
                        window_scene_changed = true;
                    }
                }
                WindowOp::Close { id } => {
                    if self.close_window(*id).is_ok() {
                        window_scene_changed = true;
                    }
                }
            }
        }

        // Single dirty-flag update for the entire batch.
        if window_scene_changed {
            self.mark_window_scene_dirty();
        }
        self.dom_dirty = true;
    }

    /// Tile all visible windows on the current workspace, applied as a single
    /// atomic batch.
    ///
    /// Single-sourced (t52-e3/e4): this now delegates to the canonical
    /// `tile_visible_windows_canonical`, which drives layout through the
    /// canonical `liquide_tiling::TilingEngine` (held in `chrome_tiling`) and
    /// applies the arrangement via the canonical batch path. The shell-side
    /// `TilingEngine::arrange*` compute methods are no longer a production code
    /// path. Visible-window collection, deterministic ordering, the empty-set
    /// early-out, and batched application all live in the canonical driver.
    pub fn tile_visible_windows(&mut self) {
        let _ = self.tile_visible_windows_canonical();
    }

    // -- Workspace switching (single-sourced onto liquide-workspaces) --------
    //
    // These methods make workspace switching REAL (fixes t49-e5-F01). Since
    // t52-e5 the switching policy (next/prev wrap, index navigation) is owned by
    // the canonical `liquide-workspaces::WorkspaceManager` *embedded inside*
    // `self.workspaces` (the shell `WorkspaceManager` adapter) — there is no
    // longer a separate `chrome_workspaces` manager to keep in lockstep, so the
    // old index-mirroring `sync_canonical_workspaces` step is gone. Visibility is
    // driven via the `WindowBatch::workspace_switch` builder (hide old members,
    // show new members) for flicker-free, batched switches. `self.workspaces`
    // ids are the shell's 0-based facade ids; the 1-based canonical mapping is
    // resolved inside the adapter.

    /// Drive the visibility transition after the active workspace has already
    /// changed in `self.workspaces`: hide the windows of `prev_members` (the
    /// previously-active workspace) and show the windows of the newly-active
    /// one, via a single batched `workspace_switch`.
    fn commit_workspace_switch(&mut self, prev_members: Vec<WindowId>) {
        self.mark_wired(crate::shell::WiringBit::Workspace);
        let show: Vec<WindowId> = self.workspaces.active().windows.clone();
        let mut batch = WindowBatch::with_capacity(prev_members.len() + show.len());
        batch.workspace_switch(&prev_members, &show);
        self.apply_batch(batch);
    }

    /// Switch to the next workspace. Real switch: the new workspace's windows
    /// become visible/interactive, the old ones hidden. Returns true if the
    /// active workspace changed.
    pub fn switch_workspace_next(&mut self) -> bool {
        let hide: Vec<WindowId> = self.workspaces.active().windows.clone();
        if !self.workspaces.switch_next() {
            return false;
        }
        self.commit_workspace_switch(hide);
        true
    }

    /// Switch to the previous workspace.
    pub fn switch_workspace_prev(&mut self) -> bool {
        let hide: Vec<WindowId> = self.workspaces.active().windows.clone();
        if !self.workspaces.switch_prev() {
            return false;
        }
        self.commit_workspace_switch(hide);
        true
    }

    /// Switch to a workspace by 0-based index. Returns true if the active
    /// workspace changed.
    pub fn switch_workspace_to_index(&mut self, index: usize) -> bool {
        let hide: Vec<WindowId> = self.workspaces.active().windows.clone();
        if !self.workspaces.switch_to_index(index) {
            return false;
        }
        self.commit_workspace_switch(hide);
        true
    }

    /// Move a window to the workspace at `target_idx` (0-based), then repatriate
    /// visibility: the window disappears from the current (active) workspace's
    /// rendered set unless the target is the active workspace. Returns true if
    /// the move succeeded.
    pub fn move_window_to_workspace_index(&mut self, id: WindowId, target_idx: usize) -> bool {
        if !self.windows.contains_key(&id) {
            return false;
        }
        let target_id = crate::workspace::WorkspaceId(target_idx as u32);
        let from_id = match self.workspaces.find_window(id) {
            Some(w) => w,
            None => return false,
        };
        if from_id == target_id {
            return true;
        }
        if self.workspaces.move_window(id, from_id, target_id).is_err() {
            return false;
        }

        // A window moved off the active workspace must stop rendering; one moved
        // onto it must start. Drive the visibility flag to match membership.
        let active = self.workspaces.active().id;
        let now_member = target_id == active;
        if let Some(win) = self.windows.get_mut(&id) {
            win.visible = now_member;
        }
        self.mark_window_scene_dirty();
        true
    }
}

// Make `normalize_z_orders` accessible from this module. It's already
// `fn normalize_z_orders(&mut self)` in `windows.rs` — Rust allows
// calling private methods on `Shell` from any file within the `shell`
// module hierarchy, so no visibility changes are needed.
