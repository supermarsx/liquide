//! Layout calculation caching for CPU renderer.
//!
//! Caches computed layouts to avoid expensive recalculation when
//! element properties haven't changed. Only recalculates when invalidated.

use liquide_compositor::geometry::Rect;
use std::collections::HashMap;

/// Cached layout information for a single element.
#[derive(Debug, Clone, Copy)]
pub struct LayoutCache {
    /// Computed bounding box in absolute coordinates.
    pub bounds: Rect,
    /// Whether this layout is currently valid.
    pub valid: bool,
    /// Layout generation counter (increments on each global invalidation).
    pub generation: u64,
}

impl LayoutCache {
    /// Create a new layout cache entry.
    #[must_use]
    pub fn new(bounds: Rect, generation: u64) -> Self {
        Self {
            bounds,
            valid: true,
            generation,
        }
    }

    /// Mark this layout as invalid (requires recalculation).
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Check if this layout is still valid for the given generation.
    #[must_use]
    pub fn is_valid(&self, current_generation: u64) -> bool {
        self.valid && self.generation == current_generation
    }
}

/// Manages layout caches for multiple elements with efficient invalidation.
pub struct LayoutCacheManager {
    /// Per-element layout caches, keyed by element ID.
    caches: HashMap<u32, LayoutCache>,
    /// Current layout generation counter.
    /// Incremented on global invalidation (e.g., viewport resize).
    generation: u64,
}

impl LayoutCacheManager {
    /// Create a new layout cache manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            caches: HashMap::new(),
            generation: 0,
        }
    }

    /// Get cached layout for an element, if valid.
    #[must_use]
    pub fn get(&self, element_id: u32) -> Option<Rect> {
        self.caches
            .get(&element_id)
            .filter(|cache| cache.is_valid(self.generation))
            .map(|cache| cache.bounds)
    }

    /// Store computed layout for an element.
    pub fn insert(&mut self, element_id: u32, bounds: Rect) {
        self.caches
            .insert(element_id, LayoutCache::new(bounds, self.generation));
    }

    /// Invalidate a specific element's layout.
    pub fn invalidate(&mut self, element_id: u32) {
        if let Some(cache) = self.caches.get_mut(&element_id) {
            cache.invalidate();
        }
    }

    /// Invalidate all layouts (e.g., on global change like viewport resize).
    pub fn invalidate_all(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        // No need to iterate — generation mismatch will invalidate all caches
    }

    /// Remove layouts for elements no longer in the scene.
    pub fn retain(&mut self, active_ids: &[u32]) {
        self.caches.retain(|id, _| active_ids.contains(id));
    }

    /// Clear all cached layouts.
    pub fn clear(&mut self) {
        self.caches.clear();
        self.generation = 0;
    }

    /// Get the number of cached layouts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.caches.len()
    }

    /// Check if there are no cached layouts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.caches.is_empty()
    }

    /// Get cache hit statistics for performance monitoring.
    #[must_use]
    pub fn stats(&self) -> LayoutCacheStats {
        let valid_count = self
            .caches
            .values()
            .filter(|c| c.is_valid(self.generation))
            .count();

        LayoutCacheStats {
            total_entries: self.caches.len(),
            valid_entries: valid_count,
            invalid_entries: self.caches.len() - valid_count,
            generation: self.generation,
        }
    }
}

impl Default for LayoutCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about layout cache performance.
#[derive(Debug, Clone, Copy)]
pub struct LayoutCacheStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub invalid_entries: usize,
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_cache_basic() {
        let mut manager = LayoutCacheManager::new();
        let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);

        manager.insert(1, bounds);
        assert_eq!(manager.get(1), Some(bounds));
    }

    #[test]
    fn test_layout_cache_invalidation() {
        let mut manager = LayoutCacheManager::new();
        let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);

        manager.insert(1, bounds);
        manager.invalidate(1);
        assert_eq!(manager.get(1), None);
    }

    #[test]
    fn test_layout_cache_global_invalidation() {
        let mut manager = LayoutCacheManager::new();

        manager.insert(1, Rect::new(0.0, 0.0, 100.0, 50.0));
        manager.insert(2, Rect::new(100.0, 0.0, 100.0, 50.0));

        manager.invalidate_all();

        assert_eq!(manager.get(1), None);
        assert_eq!(manager.get(2), None);
    }

    #[test]
    fn test_layout_cache_retain() {
        let mut manager = LayoutCacheManager::new();

        manager.insert(1, Rect::new(0.0, 0.0, 100.0, 50.0));
        manager.insert(2, Rect::new(100.0, 0.0, 100.0, 50.0));
        manager.insert(3, Rect::new(200.0, 0.0, 100.0, 50.0));

        manager.retain(&[1, 3]);

        assert_eq!(manager.len(), 2);
        assert!(manager.get(2).is_none()); // Element 2 removed
    }
}
