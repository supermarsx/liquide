//! Dirty rectangle tracking for partial redraws.
//!
//! Maintains a list of screen regions that need rerendering,
//! allowing the renderer to skip unchanged areas for better performance.

use liquide_compositor::geometry::Rect;

/// Maximum number of dirty rects before switching to full damage.
const MAX_DIRTY_RECTS: usize = 32;

/// A single dirty rectangle — a region that needs rerendering.
#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub rect: Rect,
    /// Frame number when this rect was marked dirty.
    pub frame: u64,
}

impl DirtyRect {
    /// Create a new dirty rectangle.
    #[must_use]
    pub fn new(rect: Rect, frame: u64) -> Self {
        Self { rect, frame }
    }

    /// Check if this dirty rect intersects another rect.
    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.rect.intersects(other)
    }
}

/// Manages dirty rectangles with intelligent merging and culling.
pub struct DirtyRectManager {
    /// Current set of dirty rectangles.
    dirty_rects: Vec<DirtyRect>,
    /// Current frame number.
    current_frame: u64,
    /// Screen dimensions for bounds checking.
    screen_width: u32,
    screen_height: u32,
    /// Whether the entire screen is dirty (skips fine-grained tracking).
    full_damage: bool,
}

impl DirtyRectManager {
    /// Create a new dirty rect manager.
    #[must_use]
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            dirty_rects: Vec::new(),
            current_frame: 0,
            screen_width,
            screen_height,
            full_damage: true, // Start with full redraw
        }
    }

    /// Mark a region as dirty.
    pub fn mark_dirty(&mut self, x: f32, y: f32, width: f32, height: f32) {
        if self.full_damage {
            return; // Already doing full redraw
        }

        let rect = Rect::new(x, y, width, height);

        // Clip to screen bounds
        let rect = self.clip_to_screen(rect);

        // Don't add zero-area rects
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }

        // Check if this rect can be merged with existing dirty rects
        let mut merged = false;
        for dirty in &mut self.dirty_rects {
            if Self::should_merge(&dirty.rect, &rect) {
                dirty.rect = Self::merge_rects(&dirty.rect, &rect);
                dirty.frame = self.current_frame;
                merged = true;
                break;
            }
        }

        if !merged {
            self.dirty_rects
                .push(DirtyRect::new(rect, self.current_frame));
        }

        // If we have too many dirty rects, switch to full damage
        if self.dirty_rects.len() > MAX_DIRTY_RECTS {
            self.mark_full_damage();
        }
    }

    /// Mark the entire screen as dirty (full redraw required).
    pub fn mark_full_damage(&mut self) {
        self.full_damage = true;
        self.dirty_rects.clear();
    }

    /// Get the current dirty rectangles.
    #[must_use]
    pub fn dirty_rects(&self) -> &[DirtyRect] {
        &self.dirty_rects
    }

    /// Check if the entire screen is dirty.
    #[must_use]
    pub fn is_full_damage(&self) -> bool {
        self.full_damage
    }

    /// Check if a specific rect intersects any dirty regions.
    #[must_use]
    pub fn intersects_dirty(&self, rect: &Rect) -> bool {
        if self.full_damage {
            return true;
        }

        self.dirty_rects.iter().any(|dirty| dirty.intersects(rect))
    }

    /// Clear all dirty rects and advance to the next frame.
    pub fn clear(&mut self) {
        self.dirty_rects.clear();
        self.full_damage = false;
        self.current_frame = self.current_frame.wrapping_add(1);
    }

    /// Update screen dimensions (triggers full damage).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.mark_full_damage();
    }

    /// Clip a rectangle to screen bounds.
    fn clip_to_screen(&self, rect: Rect) -> Rect {
        let screen_rect = Rect::new(
            0.0,
            0.0,
            self.screen_width as f32,
            self.screen_height as f32,
        );

        if let Some(clipped) = rect.intersection(&screen_rect) {
            clipped
        } else {
            Rect::new(0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Check if two rectangles should be merged.
    /// Merge if they overlap or are very close together.
    fn should_merge(a: &Rect, b: &Rect) -> bool {
        // Expand rects slightly for proximity check
        let threshold = 16.0;
        let expanded_a = Rect::new(
            a.x - threshold,
            a.y - threshold,
            a.width + threshold * 2.0,
            a.height + threshold * 2.0,
        );

        expanded_a.intersects(b)
    }

    /// Merge two rectangles into their bounding box.
    fn merge_rects(a: &Rect, b: &Rect) -> Rect {
        let x1 = a.x.min(b.x);
        let y1 = a.y.min(b.y);
        let x2 = (a.x + a.width).max(b.x + b.width);
        let y2 = (a.y + a.height).max(b.y + b.height);

        Rect::new(x1, y1, x2 - x1, y2 - y1)
    }

    /// Get statistics about dirty rect tracking.
    #[must_use]
    pub fn stats(&self) -> DirtyRectStats {
        let total_dirty_area: f32 = self
            .dirty_rects
            .iter()
            .map(|dr| dr.rect.width * dr.rect.height)
            .sum();

        let screen_area = (self.screen_width * self.screen_height) as f32;
        let coverage = if self.full_damage {
            100.0
        } else {
            (total_dirty_area / screen_area) * 100.0
        };

        DirtyRectStats {
            rect_count: self.dirty_rects.len(),
            total_dirty_area,
            screen_area,
            coverage_percent: coverage,
            full_damage: self.full_damage,
            frame: self.current_frame,
        }
    }
}

/// Statistics about dirty rectangle tracking.
#[derive(Debug, Clone, Copy)]
pub struct DirtyRectStats {
    pub rect_count: usize,
    pub total_dirty_area: f32,
    pub screen_area: f32,
    pub coverage_percent: f32,
    pub full_damage: bool,
    pub frame: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_rect_basic() {
        let mut manager = DirtyRectManager::new(1920, 1080);
        manager.clear(); // Clear initial full damage

        manager.mark_dirty(10.0, 10.0, 100.0, 100.0);

        assert_eq!(manager.dirty_rects().len(), 1);
        assert!(!manager.is_full_damage());
    }

    #[test]
    fn test_dirty_rect_merging() {
        let mut manager = DirtyRectManager::new(1920, 1080);
        manager.clear();

        // Mark two overlapping regions
        manager.mark_dirty(10.0, 10.0, 100.0, 100.0);
        manager.mark_dirty(50.0, 50.0, 100.0, 100.0);

        // Should be merged into one rect
        assert_eq!(manager.dirty_rects().len(), 1);
    }

    #[test]
    fn test_dirty_rect_full_damage_threshold() {
        let mut manager = DirtyRectManager::new(1920, 1080);
        manager.clear();

        // Add many widely-spaced dirty rects to trigger full damage (no merging)
        // Use a grid pattern to fit 40 rects on screen
        for i in 0..40 {
            let row = i / 10;
            let col = i % 10;
            manager.mark_dirty(col as f32 * 180.0, row as f32 * 250.0, 40.0, 40.0);
        }

        assert!(manager.is_full_damage());
    }

    #[test]
    fn test_dirty_rect_clipping() {
        let mut manager = DirtyRectManager::new(1920, 1080);
        manager.clear();

        // Mark a rect that extends beyond screen bounds
        manager.mark_dirty(1800.0, 1000.0, 300.0, 300.0);

        assert_eq!(manager.dirty_rects().len(), 1);
        let rect = manager.dirty_rects()[0].rect;

        // Should be clipped to screen bounds
        assert!(rect.x + rect.width <= 1920.0);
        assert!(rect.y + rect.height <= 1080.0);
    }

    #[test]
    fn test_dirty_rect_intersection_check() {
        let mut manager = DirtyRectManager::new(1920, 1080);
        manager.clear();

        manager.mark_dirty(100.0, 100.0, 200.0, 200.0);

        let intersecting = Rect::new(150.0, 150.0, 100.0, 100.0);
        let non_intersecting = Rect::new(400.0, 400.0, 100.0, 100.0);

        assert!(manager.intersects_dirty(&intersecting));
        assert!(!manager.intersects_dirty(&non_intersecting));
    }
}
