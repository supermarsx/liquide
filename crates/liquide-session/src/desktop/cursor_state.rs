//! Cursor tracking state — position, shape, hardware/software cursor management.
//!
//! When `liquide-cursor-vector` initializes successfully, software cursors
//! are rendered as high-definition vector images instead of basic rectangles.

use std::sync::Arc;

use liquide_compositor::scene::CursorShape;
use liquide_cursor_vector::cache::{CachedCursor, VectorCursorCache};
use liquide_cursor_vector::cursor_set::VectorCursorSet;
use tracing::{info, warn};

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
    /// Vector cursor set (loaded once on init, None if unavailable).
    cursor_set: Option<VectorCursorSet>,
    /// Per-shape RGBA pixel cache for software cursor rendering.
    #[allow(dead_code)]
    cursor_cache: Option<VectorCursorCache<'static>>,
}

impl CursorState {
    pub(super) fn new(center_x: f32, center_y: f32) -> Self {
        // Attempt to load the built-in vector cursor set.
        let (cursor_set, cursor_cache) = match VectorCursorSet::load_default() {
            Ok(set) => {
                info!(shapes = set.shapes().len(), "loaded vector cursor set");
                let cache = VectorCursorCache::new(64);
                (Some(set), Some(cache))
            }
            Err(e) => {
                warn!("vector cursors unavailable, using basic fallback: {}", e);
                (None, None)
            }
        };

        Self {
            x: center_x,
            y: center_y,
            prev_x: center_x,
            prev_y: center_y,
            dirty: false,
            use_hardware: false,
            last_hw_shape: CursorShape::Arrow,
            hw_needs_sync: false,
            cursor_set,
            cursor_cache,
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

    /// Get cached vector cursor RGBA pixels for the given shape.
    ///
    /// Returns `None` if vector cursors aren't available or the shape
    /// isn't in the cursor set — caller should fall back to the basic
    /// `SceneNodeKind::Cursor` rectangle.
    #[allow(dead_code)]
    pub(super) fn vector_cursor(&self, shape: CursorShape) -> Option<Arc<CachedCursor>> {
        let set = self.cursor_set.as_ref()?;
        let cache = self.cursor_cache.as_ref()?;
        let cursor = set.get(shape).ok()?;
        cache
            .get_or_render(cursor, shape, CURSOR_SIZE as u32, 1.0)
            .ok()
    }

    /// Whether vector cursors are available.
    #[allow(dead_code)]
    pub(super) fn has_vector_cursors(&self) -> bool {
        self.cursor_set.is_some()
    }
}
