//! Cursor tracking state — position, shape, hardware/software cursor management.
//!
//! Software cursors are rendered through the imperative `cursor_flat_node`
//! (`SceneNodeKind::Cursor`) path; the OS hardware cursor is preferred when
//! available. (A `liquide-cursor-vector` HD-vector cache was scaffolded here but
//! never consumed by the software-cursor raster, so it was removed —
//! wire-or-remove.)

use liquide_compositor::scene::CursorShape;

/// Default software cursor size in logical pixels.
pub(super) const CURSOR_SIZE: f32 = 24.0;

/// Cursor tracking and hardware/software cursor management.
pub(super) struct CursorState {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) prev_x: f32,
    pub(super) prev_y: f32,
    pub(super) dirty: bool,
    pub(super) use_hardware: bool,
    pub(super) last_hw_shape: CursorShape,
    pub(super) hw_needs_sync: bool,
}

impl CursorState {
    pub(super) fn new(center_x: f32, center_y: f32) -> Self {
        Self {
            x: center_x,
            y: center_y,
            prev_x: center_x,
            prev_y: center_y,
            dirty: false,
            use_hardware: false,
            last_hw_shape: CursorShape::Arrow,
            hw_needs_sync: false,
        }
    }

    /// Update cursor position. Returns `true` if position meaningfully changed.
    /// Sets `dirty` flag on significant movement.
    pub(super) fn update_position(&mut self, new_x: f32, new_y: f32) -> bool {
        if (new_x - self.x).abs() > 0.1 || (new_y - self.y).abs() > 0.1 {
            self.x = new_x;
            self.y = new_y;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Set position directly (e.g. from a button click event that also carries coords).
    pub(super) fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    /// Snapshot current position as "previous" for cursor-only damage tracking.
    pub(super) fn sync_prev(&mut self) {
        self.prev_x = self.x;
        self.prev_y = self.y;
    }

    /// Request a hardware cursor shape sync on next loop iteration.
    pub(super) fn request_hw_shape_sync(&mut self, shape: CursorShape) {
        if shape != self.last_hw_shape {
            self.last_hw_shape = shape;
            self.hw_needs_sync = true;
        }
    }

    /// Consume the pending hardware sync request. Returns the shape to sync
    /// and clears the flag, or `None` if no sync needed.
    pub(super) fn consume_hw_sync(&mut self) -> Option<CursorShape> {
        if self.hw_needs_sync {
            self.hw_needs_sync = false;
            Some(self.last_hw_shape)
        } else {
            None
        }
    }
}
