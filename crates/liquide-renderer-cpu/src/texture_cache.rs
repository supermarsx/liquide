//! Texture and image caching for CPU renderer.
//!
//! Caches decoded textures and rendered images to avoid redundant
//! decoding and rendering operations. Uses LRU eviction policy.

use std::collections::HashMap;
use std::sync::Arc;

/// Repeat mode identity used by realized pattern cache entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternRepeatMode {
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
    Space,
    Round,
}

/// Cache key for a realized repeated image tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatternCacheKey {
    source_key: u64,
    tile_width: u32,
    tile_height: u32,
    repeat: PatternRepeatMode,
    scale_x_bits: u32,
    scale_y_bits: u32,
}

impl PatternCacheKey {
    #[must_use]
    pub fn new(
        source_key: u64,
        tile_width: u32,
        tile_height: u32,
        repeat: PatternRepeatMode,
        scale_x: f32,
        scale_y: f32,
    ) -> Self {
        Self {
            source_key,
            tile_width,
            tile_height,
            repeat,
            scale_x_bits: cache_float_bits(scale_x),
            scale_y_bits: cache_float_bits(scale_y),
        }
    }

    #[must_use]
    pub fn source_key(&self) -> u64 {
        self.source_key
    }

    #[must_use]
    pub fn tile_dimensions(&self) -> (u32, u32) {
        (self.tile_width, self.tile_height)
    }
}

#[inline]
fn cache_float_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else if value.is_nan() {
        f32::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

/// Cached texture data — raw RGBA8 pixels.
#[derive(Clone)]
pub struct CachedTexture {
    /// Raw pixel data in RGBA8 format.
    pub data: Arc<Vec<u8>>,
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
    /// Last access timestamp (for LRU eviction).
    access_time: u64,
}

impl CachedTexture {
    /// Create a new cached texture.
    #[must_use]
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            data: Arc::new(data),
            width,
            height,
            access_time: 0,
        }
    }

    /// Get the size in bytes of this cached texture.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}

/// Hash a string texture ID to a `u64` key using FNV-1a.
#[inline]
fn hash_texture_id(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Compute the numeric key for an image ID (equivalent to `format!("img_{}", id)`
/// but without allocation). Uses a fixed high bit to avoid collisions with
/// string-hashed keys.
#[inline]
pub fn image_texture_key(image_id: u64) -> u64 {
    // Set bit 63 so image keys never collide with FNV-1a string hashes
    // (FNV-1a distributes across all 64 bits but statistically this is safe
    // for the small key-space we deal with).
    image_id | (1u64 << 63)
}

/// Texture cache with LRU eviction policy.
pub struct TextureCache {
    /// Cached textures, keyed by numeric texture ID.
    textures: HashMap<u64, CachedTexture>,
    /// Realized scaled pattern tiles, keyed by source image and repeat geometry.
    patterns: HashMap<PatternCacheKey, CachedTexture>,
    /// Current access timestamp counter.
    current_time: u64,
    /// Maximum cache size in bytes (default: 256 MB).
    max_size_bytes: usize,
    /// Current cache size in bytes.
    current_size_bytes: usize,
}

impl TextureCache {
    /// Create a new texture cache with default size limit (256 MB).
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(256 * 1024 * 1024)
    }

    /// Create a new texture cache with specified size limit.
    #[must_use]
    pub fn with_capacity(max_size_bytes: usize) -> Self {
        Self {
            textures: HashMap::new(),
            patterns: HashMap::new(),
            current_time: 0,
            max_size_bytes,
            current_size_bytes: 0,
        }
    }

    /// Get a cached realized pattern tile.
    pub(crate) fn get_pattern(&mut self, key: &PatternCacheKey) -> Option<CachedTexture> {
        if let Some(texture) = self.patterns.get_mut(key) {
            self.current_time += 1;
            texture.access_time = self.current_time;
            Some(CachedTexture {
                data: Arc::clone(&texture.data),
                width: texture.width,
                height: texture.height,
                access_time: texture.access_time,
            })
        } else {
            None
        }
    }

    /// Get a cached texture by numeric key.
    ///
    /// Returns a cheap clone — pixel data is `Arc`-shared, only the
    /// metadata wrapper (width/height/access_time) is copied.
    pub fn get_by_key(&mut self, key: u64) -> Option<CachedTexture> {
        if let Some(texture) = self.textures.get_mut(&key) {
            self.current_time += 1;
            texture.access_time = self.current_time;
            Some(CachedTexture {
                data: Arc::clone(&texture.data),
                width: texture.width,
                height: texture.height,
                access_time: texture.access_time,
            })
        } else {
            None
        }
    }

    /// Get a cached texture by string ID (hashed to u64 internally).
    pub fn get(&mut self, texture_id: &str) -> Option<CachedTexture> {
        self.get_by_key(hash_texture_id(texture_id))
    }

    /// Insert a texture by numeric key.
    pub fn insert_by_key(&mut self, key: u64, data: Vec<u8>, width: u32, height: u32) {
        let texture_size = data.len();

        if let Some(old) = self.textures.remove(&key) {
            self.current_size_bytes -= old.size_bytes();
        }
        self.remove_patterns_for_source_key(key);

        if !self.reserve_for_insert(texture_size) {
            return;
        }

        self.current_time += 1;
        let mut texture = CachedTexture::new(data, width, height);
        texture.access_time = self.current_time;

        self.current_size_bytes += texture_size;
        self.textures.insert(key, texture);
    }

    /// Insert a realized pattern tile into the cache.
    pub(crate) fn insert_pattern(
        &mut self,
        key: PatternCacheKey,
        data: Vec<u8>,
        width: u32,
        height: u32,
    ) {
        let texture_size = data.len();

        if let Some(old) = self.patterns.remove(&key) {
            self.current_size_bytes -= old.size_bytes();
        }

        if !self.reserve_for_insert(texture_size) {
            return;
        }

        self.current_time += 1;
        let mut texture = CachedTexture::new(data, width, height);
        texture.access_time = self.current_time;

        self.current_size_bytes += texture_size;
        self.patterns.insert(key, texture);
    }

    /// Insert a texture into the cache by string ID.
    /// Evicts least-recently-used textures if necessary to stay under size limit.
    pub fn insert(&mut self, texture_id: String, data: Vec<u8>, width: u32, height: u32) {
        self.insert_by_key(hash_texture_id(&texture_id), data, width, height);
    }

    /// Evict the least-recently-used texture.
    fn evict_lru(&mut self) {
        let lru_texture = self
            .textures
            .iter()
            .min_by_key(|(_, texture)| texture.access_time)
            .map(|(id, texture)| (*id, texture.access_time));
        let lru_pattern = self
            .patterns
            .iter()
            .min_by_key(|(_, texture)| texture.access_time)
            .map(|(key, texture)| (*key, texture.access_time));

        match (lru_texture, lru_pattern) {
            (Some((id, texture_time)), Some((key, pattern_time))) => {
                if texture_time <= pattern_time {
                    self.remove_texture_entry(id);
                } else {
                    self.remove_pattern_entry(&key);
                }
            }
            (Some((id, _)), None) => self.remove_texture_entry(id),
            (None, Some((key, _))) => self.remove_pattern_entry(&key),
            (None, None) => {}
        }
    }

    fn reserve_for_insert(&mut self, entry_size: usize) -> bool {
        if entry_size > self.max_size_bytes {
            return false;
        }

        while self.current_size_bytes + entry_size > self.max_size_bytes
            && (!self.textures.is_empty() || !self.patterns.is_empty())
        {
            self.evict_lru();
        }

        self.current_size_bytes + entry_size <= self.max_size_bytes
    }

    fn remove_texture_entry(&mut self, key: u64) {
        if let Some(old) = self.textures.remove(&key) {
            self.current_size_bytes -= old.size_bytes();
            self.remove_patterns_for_source_key(key);
        }
    }

    fn remove_pattern_entry(&mut self, key: &PatternCacheKey) {
        if let Some(old) = self.patterns.remove(key) {
            self.current_size_bytes -= old.size_bytes();
        }
    }

    fn remove_patterns_for_source_key(&mut self, source_key: u64) {
        let keys: Vec<PatternCacheKey> = self
            .patterns
            .keys()
            .filter(|key| key.source_key() == source_key)
            .copied()
            .collect();
        for key in keys {
            self.remove_pattern_entry(&key);
        }
    }

    /// Remove a specific texture from the cache by string ID.
    pub fn remove(&mut self, texture_id: &str) -> bool {
        let key = hash_texture_id(texture_id);
        if let Some(texture) = self.textures.remove(&key) {
            self.current_size_bytes -= texture.size_bytes();
            self.remove_patterns_for_source_key(key);
            true
        } else {
            false
        }
    }

    /// Clear all cached textures.
    pub fn clear(&mut self) {
        self.textures.clear();
        self.patterns.clear();
        self.current_size_bytes = 0;
        self.current_time = 0;
    }

    /// Get the number of cached textures.
    #[must_use]
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty() && self.patterns.is_empty()
    }

    /// Get the number of cached realized pattern tiles.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn pattern_len(&self) -> usize {
        self.patterns.len()
    }

    /// Get cache statistics for performance monitoring.
    #[must_use]
    pub fn stats(&self) -> TextureCacheStats {
        TextureCacheStats {
            entry_count: self.textures.len() + self.patterns.len(),
            size_bytes: self.current_size_bytes,
            max_size_bytes: self.max_size_bytes,
            utilization: (self.current_size_bytes as f64 / self.max_size_bytes as f64) * 100.0,
        }
    }
}

impl Default for TextureCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about texture cache performance.
#[derive(Debug, Clone, Copy)]
pub struct TextureCacheStats {
    pub entry_count: usize,
    pub size_bytes: usize,
    pub max_size_bytes: usize,
    pub utilization: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_cache_basic() {
        let mut cache = TextureCache::new();
        let data = vec![0u8; 1024];

        cache.insert("texture1".to_string(), data.clone(), 32, 32);

        let cached = cache.get("texture1");
        assert!(cached.is_some());

        let cached = cached.unwrap();
        assert_eq!(cached.width, 32);
        assert_eq!(cached.height, 32);
    }

    #[test]
    fn test_texture_cache_lru_eviction() {
        let mut cache = TextureCache::with_capacity(2048);

        // Insert 3 textures of 1KB each
        cache.insert("tex1".to_string(), vec![0u8; 1024], 32, 32);
        cache.insert("tex2".to_string(), vec![0u8; 1024], 32, 32);
        cache.insert("tex3".to_string(), vec![0u8; 1024], 32, 32);

        // Cache should have evicted tex1 (LRU)
        assert_eq!(cache.len(), 2);
        assert!(cache.get("tex1").is_none());
        assert!(cache.get("tex2").is_some());
        assert!(cache.get("tex3").is_some());
    }

    #[test]
    fn test_texture_cache_access_updates_lru() {
        let mut cache = TextureCache::with_capacity(2048);

        cache.insert("tex1".to_string(), vec![0u8; 1024], 32, 32);
        cache.insert("tex2".to_string(), vec![0u8; 1024], 32, 32);

        // Access tex1 to make it more recent
        cache.get("tex1");

        // Insert tex3, should evict tex2 (now the LRU)
        cache.insert("tex3".to_string(), vec![0u8; 1024], 32, 32);

        assert!(cache.get("tex1").is_some());
        assert!(cache.get("tex2").is_none());
        assert!(cache.get("tex3").is_some());
    }

    #[test]
    fn test_texture_cache_stats() {
        let mut cache = TextureCache::with_capacity(4096);

        cache.insert("tex1".to_string(), vec![0u8; 1024], 32, 32);
        cache.insert("tex2".to_string(), vec![0u8; 1024], 32, 32);

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.size_bytes, 2048);
    }

    #[test]
    fn test_pattern_cache_key_distinguishes_identity_size_repeat_and_scale() {
        let base = PatternCacheKey::new(1, 8, 8, PatternRepeatMode::Repeat, 1.0, 1.0);

        assert_ne!(
            base,
            PatternCacheKey::new(2, 8, 8, PatternRepeatMode::Repeat, 1.0, 1.0)
        );
        assert_ne!(
            base,
            PatternCacheKey::new(1, 16, 8, PatternRepeatMode::Repeat, 1.0, 1.0)
        );
        assert_ne!(
            base,
            PatternCacheKey::new(1, 8, 8, PatternRepeatMode::RepeatX, 1.0, 1.0)
        );
        assert_ne!(
            base,
            PatternCacheKey::new(1, 8, 8, PatternRepeatMode::Repeat, 2.0, 1.0)
        );
    }

    #[test]
    fn test_pattern_cache_lru_eviction() {
        let mut cache = TextureCache::with_capacity(2048);
        let key1 = PatternCacheKey::new(1, 16, 16, PatternRepeatMode::Repeat, 1.0, 1.0);
        let key2 = PatternCacheKey::new(2, 16, 16, PatternRepeatMode::Repeat, 1.0, 1.0);
        let key3 = PatternCacheKey::new(3, 16, 16, PatternRepeatMode::Repeat, 1.0, 1.0);

        cache.insert_pattern(key1, vec![0u8; 1024], 16, 16);
        cache.insert_pattern(key2, vec![0u8; 1024], 16, 16);
        assert!(cache.get_pattern(&key1).is_some());
        cache.insert_pattern(key3, vec![0u8; 1024], 16, 16);

        assert_eq!(cache.pattern_len(), 2);
        assert!(cache.get_pattern(&key1).is_some());
        assert!(cache.get_pattern(&key2).is_none());
        assert!(cache.get_pattern(&key3).is_some());
    }

    #[test]
    fn test_source_texture_insert_invalidates_pattern_tiles() {
        let mut cache = TextureCache::with_capacity(4096);
        let source_key = image_texture_key(42);
        let pattern_key =
            PatternCacheKey::new(source_key, 8, 8, PatternRepeatMode::Repeat, 1.0, 1.0);

        cache.insert_pattern(pattern_key, vec![0u8; 256], 8, 8);
        assert_eq!(cache.pattern_len(), 1);

        cache.insert_by_key(source_key, vec![255u8; 16], 2, 2);
        assert_eq!(cache.pattern_len(), 0);
        assert!(cache.get_pattern(&pattern_key).is_none());
    }
}
