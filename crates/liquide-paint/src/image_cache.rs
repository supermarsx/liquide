//! Async image cache for background-image URL loading.
//!
//! Tracks the load state of images referenced by `background-image: url(...)`.
//! The painter reads the cache to decide whether to emit a fully loaded image
//! or a placeholder. Callers are responsible for driving actual I/O.

use std::collections::HashMap;

/// Load state of a cached image.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageCacheEntry {
    /// A load has been requested but has not completed yet.
    Pending,
    /// The image was successfully loaded.
    Loaded {
        width: u32,
        height: u32,
        data_id: u64,
    },
    /// The image failed to load.
    Failed,
}

/// A simple LRU image cache keyed by URL string.
pub struct ImageCache {
    entries: HashMap<String, ImageCacheEntry>,
    /// Access-order list for LRU eviction (most recently accessed at the end).
    access_order: Vec<String>,
    max_entries: usize,
}

impl ImageCache {
    /// Create a new cache with the given maximum number of entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: Vec::new(),
            max_entries,
        }
    }

    /// Request loading of an image URL.
    ///
    /// Returns `true` if the URL was newly inserted (i.e. the caller should
    /// initiate a load). Returns `false` if the URL already has an entry.
    pub fn request_load(&mut self, url: &str) -> bool {
        if self.entries.contains_key(url) {
            self.touch(url);
            return false;
        }
        self.evict_if_needed();
        self.entries
            .insert(url.to_string(), ImageCacheEntry::Pending);
        self.access_order.push(url.to_string());
        true
    }

    /// Mark a previously requested URL as successfully loaded.
    pub fn mark_loaded(&mut self, url: &str, width: u32, height: u32, data_id: u64) {
        self.entries.insert(
            url.to_string(),
            ImageCacheEntry::Loaded {
                width,
                height,
                data_id,
            },
        );
        self.touch(url);
    }

    /// Mark a previously requested URL as failed.
    pub fn mark_failed(&mut self, url: &str) {
        self.entries
            .insert(url.to_string(), ImageCacheEntry::Failed);
        self.touch(url);
    }

    /// Look up the cache entry for a URL (read-only).
    pub fn get(&self, url: &str) -> Option<&ImageCacheEntry> {
        self.entries.get(url)
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Move a URL to the most-recently-used position.
    fn touch(&mut self, url: &str) {
        if let Some(pos) = self.access_order.iter().position(|u| u == url) {
            self.access_order.remove(pos);
        }
        self.access_order.push(url.to_string());
    }

    /// Evict the least-recently-used entry if at capacity.
    fn evict_if_needed(&mut self) {
        while self.entries.len() >= self.max_entries && !self.access_order.is_empty() {
            let evicted = self.access_order.remove(0);
            self.entries.remove(&evicted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_load_inserts_pending() {
        let mut cache = ImageCache::new(16);
        assert!(cache.request_load("https://example.com/bg.png"));
        assert_eq!(
            cache.get("https://example.com/bg.png"),
            Some(&ImageCacheEntry::Pending)
        );
    }

    #[test]
    fn request_load_returns_false_if_already_present() {
        let mut cache = ImageCache::new(16);
        assert!(cache.request_load("a.png"));
        assert!(!cache.request_load("a.png"));
    }

    #[test]
    fn mark_loaded_transitions_entry() {
        let mut cache = ImageCache::new(16);
        cache.request_load("img.png");
        cache.mark_loaded("img.png", 100, 200, 42);
        assert_eq!(
            cache.get("img.png"),
            Some(&ImageCacheEntry::Loaded {
                width: 100,
                height: 200,
                data_id: 42,
            })
        );
    }

    #[test]
    fn mark_failed_transitions_entry() {
        let mut cache = ImageCache::new(16);
        cache.request_load("bad.png");
        cache.mark_failed("bad.png");
        assert_eq!(cache.get("bad.png"), Some(&ImageCacheEntry::Failed));
    }

    #[test]
    fn lru_eviction() {
        let mut cache = ImageCache::new(2);
        cache.request_load("a.png");
        cache.request_load("b.png");
        // Cache is full, inserting c should evict a (least recently used)
        cache.request_load("c.png");
        assert!(cache.get("a.png").is_none(), "a.png should be evicted");
        assert!(cache.get("b.png").is_some());
        assert!(cache.get("c.png").is_some());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn lru_touch_on_access_prevents_eviction() {
        let mut cache = ImageCache::new(2);
        cache.request_load("a.png");
        cache.request_load("b.png");
        // Touch a by requesting it again
        cache.request_load("a.png"); // returns false but touches
        // Now b is LRU — inserting c should evict b
        cache.request_load("c.png");
        assert!(
            cache.get("a.png").is_some(),
            "a.png should survive (was touched)"
        );
        assert!(cache.get("b.png").is_none(), "b.png should be evicted");
        assert!(cache.get("c.png").is_some());
    }

    #[test]
    fn empty_cache() {
        let cache = ImageCache::new(10);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert!(cache.get("anything").is_none());
    }
}
