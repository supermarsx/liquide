//! Texture and image caching for CPU renderer.
//!
//! Caches decoded textures and rendered images to avoid redundant
//! decoding and rendering operations. Uses LRU eviction policy.

use std::collections::HashMap;
use std::sync::Arc;

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

/// Texture cache with LRU eviction policy.
pub struct TextureCache {
    /// Cached textures, keyed by texture ID (e.g., file path hash).
    textures: HashMap<String, CachedTexture>,
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
            current_time: 0,
            max_size_bytes,
            current_size_bytes: 0,
        }
    }

    /// Get a cached texture if it exists.
    pub fn get(&mut self, texture_id: &str) -> Option<CachedTexture> {
        if let Some(texture) = self.textures.get_mut(texture_id) {
            self.current_time += 1;
            texture.access_time = self.current_time;
            Some(texture.clone())
        } else {
            None
        }
    }

    /// Insert a texture into the cache.
    /// Evicts least-recently-used textures if necessary to stay under size limit.
    pub fn insert(&mut self, texture_id: String, data: Vec<u8>, width: u32, height: u32) {
        let texture_size = data.len();
        
        // Evict LRU entries if needed to make room
        while self.current_size_bytes + texture_size > self.max_size_bytes
            && !self.textures.is_empty()
        {
            self.evict_lru();
        }

        // Don't cache textures larger than the cache capacity
        if texture_size > self.max_size_bytes {
            return;
        }

        self.current_time += 1;
        let mut texture = CachedTexture::new(data, width, height);
        texture.access_time = self.current_time;

        // Remove old entry if it exists
        if let Some(old) = self.textures.remove(&texture_id) {
            self.current_size_bytes -= old.size_bytes();
        }

        self.current_size_bytes += texture_size;
        self.textures.insert(texture_id, texture);
    }

    /// Evict the least-recently-used texture.
    fn evict_lru(&mut self) {
        if let Some((lru_id, lru_texture)) = self
            .textures
            .iter()
            .min_by_key(|(_, texture)| texture.access_time)
            .map(|(id, texture)| (id.clone(), texture.clone()))
        {
            self.current_size_bytes -= lru_texture.size_bytes();
            self.textures.remove(&lru_id);
        }
    }

    /// Remove a specific texture from the cache.
    pub fn remove(&mut self, texture_id: &str) -> bool {
        if let Some(texture) = self.textures.remove(texture_id) {
            self.current_size_bytes -= texture.size_bytes();
            true
        } else {
            false
        }
    }

    /// Clear all cached textures.
    pub fn clear(&mut self) {
        self.textures.clear();
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
        self.textures.is_empty()
    }

    /// Get cache statistics for performance monitoring.
    #[must_use]
    pub fn stats(&self) -> TextureCacheStats {
        TextureCacheStats {
            entry_count: self.textures.len(),
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
}
