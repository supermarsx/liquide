//! Glyph cache — LRU cache for rasterized glyph bitmaps.
//!
//! Avoids re-rasterizing the same glyph at the same size/subpixel offset.
//! The cache is keyed by `(FontFaceId, glyph_id, size_key, subpixel_key)`.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::database::FontFaceId;
use crate::rasterize::GlyphBitmap;

/// A key that uniquely identifies a cached glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphCacheKey {
    pub face_id: FontFaceId,
    pub glyph_id: u32,
    /// Font size × 64 (to capture 1/64th px granularity).
    pub size_key: u32,
    /// Subpixel x offset × 4 (0..3 for quarter-pixel positioning).
    pub subpixel_x: u8,
    /// Subpixel y offset × 4.
    pub subpixel_y: u8,
}

impl GlyphCacheKey {
    /// Build a cache key from floating-point size and subpixel offset.
    #[must_use]
    pub fn new(face_id: FontFaceId, glyph_id: u32, size_px: f32, subpixel_x: f32, subpixel_y: f32) -> Self {
        Self {
            face_id,
            glyph_id,
            size_key: (size_px * 64.0) as u32,
            subpixel_x: ((subpixel_x.fract().abs() * 4.0) as u8).min(3),
            subpixel_y: ((subpixel_y.fract().abs() * 4.0) as u8).min(3),
        }
    }
}

/// LRU entry with access counter for eviction.
struct CacheEntry {
    bitmap: GlyphBitmap,
    last_access: u64,
}

/// Thread-safe LRU glyph cache.
pub struct GlyphCache {
    inner: Mutex<GlyphCacheInner>,
}

struct GlyphCacheInner {
    entries: HashMap<GlyphCacheKey, CacheEntry>,
    access_counter: u64,
    max_entries: usize,
    /// Total bytes of pixel data stored.
    total_bytes: usize,
    /// Maximum bytes before eviction.
    max_bytes: usize,
    hits: u64,
    misses: u64,
}

impl GlyphCache {
    /// Create a new glyph cache with the given capacity limits.
    #[must_use]
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(GlyphCacheInner {
                entries: HashMap::with_capacity(max_entries / 2),
                access_counter: 0,
                max_entries,
                total_bytes: 0,
                max_bytes,
                hits: 0,
                misses: 0,
            }),
        }
    }

    /// Create a cache with sensible defaults (8192 entries, 64 MiB).
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(8192, 64 * 1024 * 1024)
    }

    /// Look up a cached glyph bitmap.
    #[must_use]
    pub fn get(&self, key: &GlyphCacheKey) -> Option<GlyphBitmap> {
        let mut inner = liquide_common::sync::lock_or_recover(&self.inner);
        inner.access_counter += 1;
        let counter = inner.access_counter;
        if let Some(entry) = inner.entries.get_mut(key) {
            entry.last_access = counter;
            let bmp = entry.bitmap.clone();
            inner.hits += 1;
            Some(bmp)
        } else {
            inner.misses += 1;
            None
        }
    }

    /// Insert a glyph bitmap into the cache.
    pub fn insert(&self, key: GlyphCacheKey, bitmap: GlyphBitmap) {
        let mut inner = liquide_common::sync::lock_or_recover(&self.inner);
        let pixel_bytes = bitmap.pixels.len();

        // Evict if we're at capacity
        while (inner.entries.len() >= inner.max_entries || inner.total_bytes + pixel_bytes > inner.max_bytes)
            && !inner.entries.is_empty()
        {
            // Find the least recently used entry
            let lru_key = inner
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_access)
                .map(|(k, _)| *k);
            if let Some(lru_key) = lru_key {
                if let Some(removed) = inner.entries.remove(&lru_key) {
                    inner.total_bytes = inner.total_bytes.saturating_sub(removed.bitmap.pixels.len());
                }
            } else {
                break;
            }
        }

        inner.access_counter += 1;
        let counter = inner.access_counter;
        inner.total_bytes += pixel_bytes;
        inner.entries.insert(key, CacheEntry {
            bitmap,
            last_access: counter,
        });
    }

    /// Invalidate all cached glyphs for a specific font face (e.g., on font reload).
    pub fn invalidate_face(&self, face_id: FontFaceId) {
        let mut inner = liquide_common::sync::lock_or_recover(&self.inner);
        let keys_to_remove: Vec<GlyphCacheKey> = inner
            .entries
            .keys()
            .filter(|k| k.face_id == face_id)
            .copied()
            .collect();
        for key in keys_to_remove {
            if let Some(removed) = inner.entries.remove(&key) {
                inner.total_bytes = inner.total_bytes.saturating_sub(removed.bitmap.pixels.len());
            }
        }
    }

    /// Clear the entire cache.
    pub fn clear(&self) {
        let mut inner = liquide_common::sync::lock_or_recover(&self.inner);
        inner.entries.clear();
        inner.total_bytes = 0;
    }

    /// Get cache statistics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        let inner = liquide_common::sync::lock_or_recover(&self.inner);
        CacheStats {
            entries: inner.entries.len(),
            total_bytes: inner.total_bytes,
            hits: inner.hits,
            misses: inner.misses,
            hit_rate: if inner.hits + inner.misses > 0 {
                inner.hits as f64 / (inner.hits + inner.misses) as f64
            } else {
                0.0
            },
        }
    }
}

/// Cache performance statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub total_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_bitmap(glyph_id: u32) -> GlyphBitmap {
        GlyphBitmap {
            glyph_id,
            width: 10,
            height: 12,
            bearing_x: 1.0,
            bearing_y: 10.0,
            advance: 8.0,
            pixels: vec![128u8; 120],
            is_subpixel: false,
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = GlyphCache::with_defaults();
        let key = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        cache.insert(key, dummy_bitmap(65));
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn test_cache_miss() {
        let cache = GlyphCache::with_defaults();
        let key = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = GlyphCache::new(2, 1024 * 1024);
        let k1 = GlyphCacheKey::new(FontFaceId(1), 1, 16.0, 0.0, 0.0);
        let k2 = GlyphCacheKey::new(FontFaceId(1), 2, 16.0, 0.0, 0.0);
        let k3 = GlyphCacheKey::new(FontFaceId(1), 3, 16.0, 0.0, 0.0);
        cache.insert(k1, dummy_bitmap(1));
        cache.insert(k2, dummy_bitmap(2));
        // Access k2 to make it more recent
        let _ = cache.get(&k2);
        cache.insert(k3, dummy_bitmap(3));
        // k1 should have been evicted (LRU)
        assert!(cache.get(&k1).is_none());
        assert!(cache.get(&k2).is_some());
        assert!(cache.get(&k3).is_some());
    }

    #[test]
    fn test_cache_invalidate_face() {
        let cache = GlyphCache::with_defaults();
        let k1 = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        let k2 = GlyphCacheKey::new(FontFaceId(2), 65, 16.0, 0.0, 0.0);
        cache.insert(k1, dummy_bitmap(65));
        cache.insert(k2, dummy_bitmap(65));
        cache.invalidate_face(FontFaceId(1));
        assert!(cache.get(&k1).is_none());
        assert!(cache.get(&k2).is_some());
    }

    #[test]
    fn test_cache_stats() {
        let cache = GlyphCache::with_defaults();
        let key = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        cache.insert(key, dummy_bitmap(65));
        let _ = cache.get(&key); // hit
        let miss_key = GlyphCacheKey::new(FontFaceId(1), 66, 16.0, 0.0, 0.0);
        let _ = cache.get(&miss_key); // miss
        let stats = cache.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_cache_clear() {
        let cache = GlyphCache::with_defaults();
        let key = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        cache.insert(key, dummy_bitmap(65));
        cache.clear();
        assert!(cache.get(&key).is_none());
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn test_cache_byte_limit_eviction() {
        // Each dummy_bitmap has 120 bytes of pixel data
        let cache = GlyphCache::new(100, 200);
        let k1 = GlyphCacheKey::new(FontFaceId(1), 1, 16.0, 0.0, 0.0);
        let k2 = GlyphCacheKey::new(FontFaceId(1), 2, 16.0, 0.0, 0.0);
        cache.insert(k1, dummy_bitmap(1)); // 120 bytes
        cache.insert(k2, dummy_bitmap(2)); // 240 > 200, should evict k1
        assert!(cache.get(&k1).is_none());
        assert!(cache.get(&k2).is_some());
    }

    #[test]
    fn test_cache_key_subpixel_precision() {
        let k1 = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        let k2 = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.25, 0.0);
        let k3 = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.5, 0.0);
        assert_ne!(k1, k2);
        assert_ne!(k2, k3);
    }

    #[test]
    fn test_cache_key_different_sizes() {
        let k1 = GlyphCacheKey::new(FontFaceId(1), 65, 12.0, 0.0, 0.0);
        let k2 = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        assert_ne!(k1.size_key, k2.size_key);
    }

    #[test]
    fn test_cache_hit_rate() {
        let cache = GlyphCache::with_defaults();
        let key = GlyphCacheKey::new(FontFaceId(1), 65, 16.0, 0.0, 0.0);
        cache.insert(key, dummy_bitmap(65));
        let _ = cache.get(&key);
        let _ = cache.get(&key);
        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 0);
        assert!((stats.hit_rate - 1.0).abs() < f64::EPSILON);
    }
}
