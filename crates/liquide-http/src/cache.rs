//! A tiny capped, URL-keyed in-memory response cache.
//!
//! Map tiles are requested over and over as the user pans back and forth, so a
//! repeat `fetch(url)` should serve the previously-downloaded bytes instead of
//! hitting the network again. This is a straightforward LRU: on insertion past
//! capacity the least-recently-*used* entry is evicted, so the cache can never
//! grow without bound.
//!
//! It is intentionally simple and lock-free at this level — the
//! [`HttpClient`](crate::HttpClient) wraps it in a `Mutex` since it is touched
//! both from the calling thread (lookups on `fetch`) and the runtime threads
//! (insertions on completion).

use bytes::Bytes;
use std::collections::HashMap;

/// An LRU cache mapping a request URL to its response body.
#[derive(Debug)]
pub struct ResponseCache {
    capacity: usize,
    /// URL -> (body, tick-when-last-used). A monotonically increasing `clock`
    /// stamps each access; the smallest stamp is the LRU victim.
    entries: HashMap<String, (Bytes, u64)>,
    clock: u64,
}

impl ResponseCache {
    /// Create a cache holding at most `capacity` entries. A capacity of `0`
    /// disables caching (every `get` misses, `put` is a no-op).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            clock: 0,
        }
    }

    /// Number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up `url`, marking it most-recently-used on a hit. Returns a cheap
    /// (refcounted) clone of the body.
    pub fn get(&mut self, url: &str) -> Option<Bytes> {
        self.clock += 1;
        let now = self.clock;
        let entry = self.entries.get_mut(url)?;
        entry.1 = now;
        Some(entry.0.clone())
    }

    /// Insert (or refresh) `url`'s body, evicting the least-recently-used entry
    /// first if the cache is at capacity. A no-op when capacity is `0`.
    pub fn put(&mut self, url: String, body: Bytes) {
        if self.capacity == 0 {
            return;
        }
        self.clock += 1;
        let now = self.clock;
        // If we're at capacity and this is a NEW key, evict the LRU victim.
        if !self.entries.contains_key(&url) && self.entries.len() >= self.capacity {
            if let Some(victim) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, stamp))| *stamp)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&victim);
            }
        }
        self.entries.insert(url, (body, now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_what_was_put() {
        let mut cache = ResponseCache::new(4);
        cache.put("a".into(), Bytes::from_static(b"alpha"));
        assert_eq!(cache.get("a").as_deref(), Some(&b"alpha"[..]));
        assert!(cache.get("missing").is_none());
    }

    #[test]
    fn zero_capacity_never_caches() {
        let mut cache = ResponseCache::new(0);
        cache.put("a".into(), Bytes::from_static(b"x"));
        assert!(cache.get("a").is_none());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn evicts_least_recently_used_at_capacity() {
        let mut cache = ResponseCache::new(2);
        cache.put("a".into(), Bytes::from_static(b"a"));
        cache.put("b".into(), Bytes::from_static(b"b"));
        // Touch "a" so "b" becomes the LRU victim.
        assert!(cache.get("a").is_some());
        cache.put("c".into(), Bytes::from_static(b"c"));
        assert!(cache.get("a").is_some(), "a was recently used, kept");
        assert!(cache.get("b").is_none(), "b was LRU, evicted");
        assert!(cache.get("c").is_some(), "c was just inserted");
        assert_eq!(cache.len(), 2);
    }
}
