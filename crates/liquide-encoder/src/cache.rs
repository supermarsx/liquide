//! Tile payload cache with eviction for content-addressable deduplication.
//!
//! Caches compressed tile payloads keyed by CRC-32C. When the client already
//! has a tile in its cache, the server sends a `Copy` reference instead of
//! the full payload. Uses a Hot/Warm/Cold eviction scheme based on MRU access.

use std::collections::BTreeMap;

/// Temperature classification for cache entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Temperature {
    /// Recently accessed — keep.
    Hot,
    /// Not recently accessed — candidate for eviction.
    Warm,
    /// Cold — will be evicted first.
    Cold,
}

/// A cached tile payload.
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The compressed tile payload.
    payload: Vec<u8>,
    /// Last access frame sequence number.
    last_access: u64,
    /// Temperature classification.
    temperature: Temperature,
}

/// Tile payload cache with Hot/Warm/Cold eviction.
pub struct TilePayloadCache {
    entries: BTreeMap<u32, CacheEntry>,
    max_entries: usize,
    current_frame: u64,
    /// Frames since last access before demotion to Warm.
    warm_threshold: u64,
    /// Frames since last access before demotion to Cold.
    cold_threshold: u64,
}

impl TilePayloadCache {
    /// Create a new cache with the given maximum number of entries.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            max_entries,
            current_frame: 0,
            warm_threshold: 30,
            cold_threshold: 120,
        }
    }

    /// Advance to the next frame. Updates temperature classifications.
    pub fn advance_frame(&mut self) {
        self.current_frame += 1;

        // Update temperatures
        let frame = self.current_frame;
        let warm_th = self.warm_threshold;
        let cold_th = self.cold_threshold;

        for entry in self.entries.values_mut() {
            let age = frame.saturating_sub(entry.last_access);
            entry.temperature = if age < warm_th {
                Temperature::Hot
            } else if age < cold_th {
                Temperature::Warm
            } else {
                Temperature::Cold
            };
        }
    }

    /// Look up a cached payload by CRC. Returns `Some(&payload)` and marks as Hot.
    pub fn get(&mut self, crc: u32) -> Option<&[u8]> {
        let frame = self.current_frame;
        if let Some(entry) = self.entries.get_mut(&crc) {
            entry.last_access = frame;
            entry.temperature = Temperature::Hot;
            Some(&entry.payload)
        } else {
            None
        }
    }

    /// Check if a CRC is cached (without updating access time).
    #[must_use]
    pub fn contains(&self, crc: u32) -> bool {
        self.entries.contains_key(&crc)
    }

    /// Insert a compressed payload into the cache. Evicts cold entries if full.
    pub fn insert(&mut self, crc: u32, payload: Vec<u8>) {
        if self.entries.contains_key(&crc) {
            // Update existing entry
            if let Some(entry) = self.entries.get_mut(&crc) {
                entry.payload = payload;
                entry.last_access = self.current_frame;
                entry.temperature = Temperature::Hot;
            }
            return;
        }

        // Evict if at capacity
        while self.entries.len() >= self.max_entries {
            if !self.evict_one() {
                break;
            }
        }

        self.entries.insert(
            crc,
            CacheEntry {
                payload,
                last_access: self.current_frame,
                temperature: Temperature::Hot,
            },
        );
    }

    /// Evict one entry, preferring Cold → Warm → oldest Hot.
    /// Returns false if cache is empty.
    fn evict_one(&mut self) -> bool {
        // Find best eviction candidate
        let mut cold_key: Option<u32> = None;
        let mut warm_key: Option<u32> = None;
        let mut oldest_key: Option<(u32, u64)> = None;

        for (&crc, entry) in &self.entries {
            match entry.temperature {
                Temperature::Cold => {
                    if cold_key.is_none() {
                        cold_key = Some(crc);
                    }
                }
                Temperature::Warm => {
                    if warm_key.is_none() {
                        warm_key = Some(crc);
                    }
                }
                Temperature::Hot => {
                    if let Some((_, oldest_access)) = oldest_key {
                        if entry.last_access < oldest_access {
                            oldest_key = Some((crc, entry.last_access));
                        }
                    } else {
                        oldest_key = Some((crc, entry.last_access));
                    }
                }
            }
        }

        let key = cold_key
            .or(warm_key)
            .or(oldest_key.map(|(k, _)| k));

        if let Some(k) = key {
            self.entries.remove(&k);
            true
        } else {
            false
        }
    }

    /// Number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
