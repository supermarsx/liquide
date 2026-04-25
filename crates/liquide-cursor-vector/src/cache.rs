//! Caching for rendered vector cursors

use crate::cursor_set::VectorCursor;
use crate::error::Result;
use crate::renderer::VectorCursorRenderer;
use liquide_cursor::CursorShape;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Cache key for rendered cursors
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    shape: CursorShape,
    size: u32,
    scale_x100: u32, // Scale * 100 to avoid floating point key
}

impl CacheKey {
    fn new(shape: CursorShape, size: u32, scale: f32) -> Self {
        Self {
            shape,
            size,
            scale_x100: (scale * 100.0) as u32,
        }
    }
}

/// Cached render result
#[derive(Debug, Clone)]
pub struct CachedCursor {
    /// RGBA8 pixel data
    pub pixels: Arc<Vec<u8>>,

    /// Physical width
    pub width: u32,

    /// Physical height
    pub height: u32,

    /// Hotspot X in pixels
    pub hotspot_x: u32,

    /// Hotspot Y in pixels
    pub hotspot_y: u32,
}

/// Thread-safe cache for rendered vector cursors
pub struct VectorCursorCache<'a> {
    renderer: VectorCursorRenderer<'a>,
    cache: RwLock<HashMap<CacheKey, Arc<CachedCursor>>>,
    max_entries: usize,
}

impl Default for VectorCursorCache<'_> {
    fn default() -> Self {
        Self::new(100)
    }
}

impl VectorCursorCache<'_> {
    /// Create a new cache with specified capacity
    pub fn new(max_entries: usize) -> Self {
        Self {
            renderer: VectorCursorRenderer::new(),
            cache: RwLock::new(HashMap::new()),
            max_entries,
        }
    }

    /// Get or render a cursor
    pub fn get_or_render(
        &self,
        cursor: &VectorCursor,
        shape: CursorShape,
        size: u32,
        scale: f32,
    ) -> Result<Arc<CachedCursor>> {
        let key = CacheKey::new(shape, size, scale);

        // Try to get from cache
        {
            let cache = liquide_common::sync::read_or_recover(&self.cache);
            if let Some(cached) = cache.get(&key) {
                return Ok(cached.clone());
            }
        }

        // Render new
        let pixels = self.renderer.render(cursor, size, scale)?;
        let physical_size = (size as f32 * scale) as u32;
        let (hotspot_x, hotspot_y) = cursor.hotspot_pixels(physical_size);

        let cached = Arc::new(CachedCursor {
            pixels: Arc::new(pixels),
            width: physical_size,
            height: physical_size,
            hotspot_x,
            hotspot_y,
        });

        // Store in cache
        {
            let mut cache = liquide_common::sync::write_or_recover(&self.cache);

            // Evict if necessary (simple LRU: clear oldest half)
            if cache.len() >= self.max_entries {
                let keys_to_remove: Vec<_> =
                    cache.keys().take(self.max_entries / 2).cloned().collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }

            cache.insert(key, cached.clone());
        }

        Ok(cached)
    }

    /// Pre-warm cache with common sizes
    pub fn prewarm(
        &self,
        cursors: &[(CursorShape, &VectorCursor)],
        sizes: &[u32],
        scales: &[f32],
    ) -> Result<()> {
        for (shape, cursor) in cursors {
            for &size in sizes {
                for &scale in scales {
                    self.get_or_render(cursor, *shape, size, scale)?;
                }
            }
        }
        Ok(())
    }

    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = liquide_common::sync::write_or_recover(&self.cache);
        cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache = liquide_common::sync::read_or_recover(&self.cache);
        let total_bytes: usize = cache.values().map(|c| c.pixels.len()).sum();

        CacheStats {
            entries: cache.len(),
            total_bytes,
            max_entries: self.max_entries,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached entries
    pub entries: usize,

    /// Total bytes used
    pub total_bytes: usize,

    /// Maximum entries
    pub max_entries: usize,
}

impl CacheStats {
    /// Get memory usage in megabytes
    pub fn memory_mb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get cache utilization percentage
    pub fn utilization(&self) -> f64 {
        (self.entries as f64 / self.max_entries as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor_set::VectorCursorSet;

    #[test]
    fn test_cache_hit() {
        let set = VectorCursorSet::load_default().unwrap();
        let cursor = set.get(CursorShape::Arrow).unwrap();

        let cache = VectorCursorCache::new(10);

        // First access - cache miss
        let first = cache
            .get_or_render(cursor, CursorShape::Arrow, 32, 1.0)
            .unwrap();

        // Second access - cache hit
        let second = cache
            .get_or_render(cursor, CursorShape::Arrow, 32, 1.0)
            .unwrap();

        // Should be the same Arc
        assert!(Arc::ptr_eq(&first.pixels, &second.pixels));
    }

    #[test]
    fn test_cache_eviction() {
        let set = VectorCursorSet::load_default().unwrap();
        let cursor = set.get(CursorShape::Arrow).unwrap();

        let cache = VectorCursorCache::new(5);

        // Fill cache beyond capacity
        for size in 16..26 {
            cache
                .get_or_render(cursor, CursorShape::Arrow, size, 1.0)
                .unwrap();
        }

        let stats = cache.stats();
        assert!(stats.entries <= 5);
    }

    #[test]
    fn test_prewarm() {
        let set = VectorCursorSet::load_default().unwrap();
        let arrow = set.get(CursorShape::Arrow).unwrap();
        let pointer = set.get(CursorShape::Pointer).unwrap();

        let cache = VectorCursorCache::new(20);

        let cursors = vec![(CursorShape::Arrow, arrow), (CursorShape::Pointer, pointer)];

        cache.prewarm(&cursors, &[24, 32, 48], &[1.0, 2.0]).unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entries, 12); // 2 shapes * 3 sizes * 2 scales
    }
}
