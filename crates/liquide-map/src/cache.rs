//! A bounded LRU cache of decoded/fetched tile payloads keyed by [`TileId`].
//!
//! Re-panning back over a region must NOT re-fetch tiles it already has, so the
//! tile manager keeps every fetched tile here. The cache is capped (capacity in
//! entries) and evicts the LEAST-RECENTLY-USED tile when full, so a long pan
//! across the world can't grow memory without bound.
//!
//! The payload is generic so the cache works for the raw fetched bytes
//! (`bytes::Bytes`) and, in tests, for any cheap stand-in.

use std::collections::HashMap;

use crate::slippy::TileId;

/// A bounded, least-recently-used cache of tile payloads.
#[derive(Debug)]
pub struct TileCache<V> {
    capacity: usize,
    /// `key -> (value, last_use_stamp)`.
    entries: HashMap<TileId, (V, u64)>,
    /// Monotonic access counter; the smallest stamp is the LRU entry.
    clock: u64,
}

impl<V> TileCache<V> {
    /// A cache holding at most `capacity` tiles. A capacity of 0 disables
    /// caching (every `put` is dropped, every `get` misses).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            clock: 0,
        }
    }

    /// Number of tiles currently cached.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds no tiles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The configured capacity (max entries).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Whether the cache currently holds `key` (does NOT count as a use).
    #[must_use]
    pub fn contains(&self, key: &TileId) -> bool {
        self.entries.contains_key(key)
    }

    /// Borrow the cached value for `key`, marking it most-recently-used.
    pub fn get(&mut self, key: &TileId) -> Option<&V> {
        self.clock += 1;
        let stamp = self.clock;
        if let Some(slot) = self.entries.get_mut(key) {
            slot.1 = stamp;
            Some(&slot.0)
        } else {
            None
        }
    }

    /// Insert (or replace) the value for `key`, marking it most-recently-used and
    /// evicting the LRU entry first if the cache is at capacity. With capacity 0
    /// nothing is stored.
    pub fn put(&mut self, key: TileId, value: V) {
        if self.capacity == 0 {
            return;
        }
        self.clock += 1;
        let stamp = self.clock;
        // Inserting a brand-new key when full → evict the LRU first. Replacing an
        // existing key never grows the map, so it never needs an eviction.
        if !self.entries.contains_key(&key) && self.entries.len() >= self.capacity {
            self.evict_lru();
        }
        self.entries.insert(key, (value, stamp));
    }

    /// Remove and return the least-recently-used entry's key, if any.
    fn evict_lru(&mut self) {
        if let Some((&lru_key, _)) = self.entries.iter().min_by_key(|(_, (_, stamp))| *stamp) {
            self.entries.remove(&lru_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid(x: i64) -> TileId {
        TileId::new(3, x, 0)
    }

    #[test]
    fn put_then_get_returns_the_value_and_dedupes() {
        let mut cache: TileCache<u32> = TileCache::new(4);
        cache.put(tid(0), 100);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&tid(0)), Some(&100));
        // Re-putting the SAME key does not grow the cache (de-dupe on re-pan).
        cache.put(tid(0), 100);
        assert_eq!(cache.len(), 1, "re-putting an existing key must not grow");
        // A miss for an absent key.
        assert_eq!(cache.get(&tid(99)), None);
    }

    #[test]
    fn evicts_least_recently_used_when_full() {
        let mut cache: TileCache<u32> = TileCache::new(2);
        cache.put(tid(0), 0);
        cache.put(tid(1), 1);
        // Touch tile 0 so tile 1 becomes the LRU.
        assert_eq!(cache.get(&tid(0)), Some(&0));
        // Insert a third tile → the LRU (tile 1) is evicted, tile 0 survives.
        cache.put(tid(2), 2);
        assert_eq!(cache.len(), 2);
        assert!(cache.contains(&tid(0)), "recently-used tile 0 must survive");
        assert!(!cache.contains(&tid(1)), "LRU tile 1 must be evicted");
        assert!(cache.contains(&tid(2)));
    }

    #[test]
    fn capacity_zero_never_caches() {
        let mut cache: TileCache<u32> = TileCache::new(0);
        cache.put(tid(0), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.get(&tid(0)), None);
    }
}
