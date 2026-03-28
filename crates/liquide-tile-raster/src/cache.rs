//! LRU tile cache: keeps recently-used tile pixel data in memory.

use std::collections::HashMap;
use crate::tile::TileId;

/// Statistics for the tile cache.
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    /// Number of tiles currently in the cache.
    pub entries: usize,
    /// Maximum number of tiles the cache can hold.
    pub capacity: usize,
    /// Total number of cache hits.
    pub hits: u64,
    /// Total number of cache misses.
    pub misses: u64,
    /// Total number of evictions.
    pub evictions: u64,
    /// Total bytes used by cached pixel data.
    pub bytes_used: usize,
}

impl CacheStats {
    /// Hit rate as a fraction (0.0-1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// A node in the LRU doubly-linked list.
struct CacheEntry {
    /// Tile pixel data.
    pixels: Vec<u8>,
    /// Previous entry in LRU order (more recently used).
    prev: Option<TileId>,
    /// Next entry in LRU order (less recently used).
    next: Option<TileId>,
}

/// LRU cache for tile pixel data.
///
/// Stores up to `capacity` tile pixel buffers. When the cache is full,
/// the least-recently-used tile is evicted.
pub struct TileCache {
    /// Maximum number of tiles to keep.
    capacity: usize,
    /// Map from tile ID to cache entry.
    entries: HashMap<TileId, CacheEntry>,
    /// Most recently used tile (head of LRU list).
    head: Option<TileId>,
    /// Least recently used tile (tail of LRU list).
    tail: Option<TileId>,
    /// Accumulated statistics.
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl TileCache {
    /// Create a new tile cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::with_capacity(capacity),
            head: None,
            tail: None,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// Retrieve cached pixel data for a tile.
    ///
    /// Returns `Some(&[u8])` if the tile is cached, `None` otherwise.
    /// Moves the accessed tile to the front of the LRU list.
    pub fn get(&mut self, tile_id: TileId) -> Option<&[u8]> {
        if self.entries.contains_key(&tile_id) {
            self.hits += 1;
            self.move_to_front(tile_id);
            Some(&self.entries[&tile_id].pixels)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Check if a tile is in the cache without updating LRU order.
    pub fn contains(&self, tile_id: TileId) -> bool {
        self.entries.contains_key(&tile_id)
    }

    /// Store pixel data for a tile, evicting the LRU entry if at capacity.
    pub fn put(&mut self, tile_id: TileId, pixels: Vec<u8>) {
        // If already present, update in place and move to front.
        if self.entries.contains_key(&tile_id) {
            self.detach(tile_id);
            self.entries.get_mut(&tile_id).unwrap().pixels = pixels;
            self.push_front(tile_id);
            return;
        }

        // Evict if at capacity.
        while self.entries.len() >= self.capacity && self.capacity > 0 {
            self.evict_lru();
        }

        if self.capacity == 0 {
            return;
        }

        // Insert new entry at front.
        let entry = CacheEntry {
            pixels,
            prev: None,
            next: None,
        };
        self.entries.insert(tile_id, entry);
        self.push_front(tile_id);
    }

    /// Evict the least-recently-used tile from the cache.
    pub fn evict_lru(&mut self) {
        if let Some(lru_id) = self.tail {
            self.detach(lru_id);
            self.entries.remove(&lru_id);
            self.evictions += 1;
        }
    }

    /// Remove a specific tile from the cache.
    pub fn remove(&mut self, tile_id: TileId) {
        if self.entries.contains_key(&tile_id) {
            self.detach(tile_id);
            self.entries.remove(&tile_id);
        }
    }

    /// Clear all cached tiles.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.head = None;
        self.tail = None;
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let bytes_used: usize = self.entries.values().map(|e| e.pixels.len()).sum();
        CacheStats {
            entries: self.entries.len(),
            capacity: self.capacity,
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            bytes_used,
        }
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Detach a node from the doubly-linked list.
    fn detach(&mut self, id: TileId) {
        let (prev, next) = {
            let entry = match self.entries.get(&id) {
                Some(e) => e,
                None => return,
            };
            (entry.prev, entry.next)
        };

        // Update previous node's next pointer.
        if let Some(prev_id) = prev {
            if let Some(prev_entry) = self.entries.get_mut(&prev_id) {
                prev_entry.next = next;
            }
        } else {
            // This was the head.
            self.head = next;
        }

        // Update next node's prev pointer.
        if let Some(next_id) = next {
            if let Some(next_entry) = self.entries.get_mut(&next_id) {
                next_entry.prev = prev;
            }
        } else {
            // This was the tail.
            self.tail = prev;
        }

        // Clear the detached node's links.
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.prev = None;
            entry.next = None;
        }
    }

    /// Push a node to the front of the LRU list (most recently used).
    fn push_front(&mut self, id: TileId) {
        if let Some(old_head) = self.head {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.next = Some(old_head);
                entry.prev = None;
            }
            if let Some(head_entry) = self.entries.get_mut(&old_head) {
                head_entry.prev = Some(id);
            }
        } else {
            // List was empty; this is both head and tail.
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.next = None;
                entry.prev = None;
            }
            self.tail = Some(id);
        }
        self.head = Some(id);
    }

    /// Move an existing node to the front of the LRU list.
    fn move_to_front(&mut self, id: TileId) {
        if self.head == Some(id) {
            return; // Already at front.
        }
        self.detach(id);
        self.push_front(id);
    }
}

impl Default for TileCache {
    fn default() -> Self {
        // Default capacity: enough for a 4K display at 256px tiles
        // (4096/256 * 2160/256 = 16 * 9 = 144 tiles, double for scrolling)
        Self::new(288)
    }
}

impl std::fmt::Debug for TileCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stats = self.stats();
        f.debug_struct("TileCache")
            .field("entries", &stats.entries)
            .field("capacity", &stats.capacity)
            .field("hit_rate", &format!("{:.1}%", stats.hit_rate() * 100.0))
            .field("bytes_used", &stats.bytes_used)
            .finish()
    }
}
