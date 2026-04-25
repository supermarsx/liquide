//! CSS query caching for theme engine

use crate::property::PropertySet;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

/// Cache key for style queries
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct CacheKey {
    element: String,
    classes: Vec<String>,
    id: Option<String>,
    pseudo_classes: Vec<String>,
}

impl CacheKey {
    fn new(element: &str, classes: &[String], id: Option<&str>, pseudo_classes: &[String]) -> Self {
        Self {
            element: element.to_string(),
            classes: classes.to_vec(),
            id: id.map(|s| s.to_string()),
            pseudo_classes: pseudo_classes.to_vec(),
        }
    }
}

/// Thread-safe cache for CSS query results
#[derive(Debug, Clone)]
pub struct QueryCache {
    /// Cached query results
    cache: Arc<RwLock<HashMap<CacheKey, PropertySet>>>,

    /// Insertion order for real FIFO eviction. `HashMap::keys()` has no
    /// ordering guarantee, so a `HashMap`-only implementation ends up
    /// evicting an arbitrary entry on each overflow. We pair the map with a
    /// `VecDeque<CacheKey>` that tracks the exact order in which keys were
    /// inserted, so `insert` evicts the oldest entry deterministically.
    order: Arc<RwLock<VecDeque<CacheKey>>>,

    /// Maximum cache size
    max_size: usize,

    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total queries
    pub total_queries: u64,

    /// Cache hits
    pub cache_hits: u64,

    /// Cache misses
    pub cache_misses: u64,

    /// Cache evictions
    pub evictions: u64,
}

impl CacheStats {
    /// Get cache hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.total_queries == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_queries as f64
        }
    }
}

impl QueryCache {
    /// Create a new query cache
    ///
    /// # Arguments
    /// * `max_size` - Maximum number of cached queries (0 = unlimited)
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            order: Arc::new(RwLock::new(VecDeque::new())),
            max_size,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get a cached query result
    pub fn get(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> Option<PropertySet> {
        let key = CacheKey::new(element, classes, id, pseudo_classes);

        // Update stats
        {
            let mut stats = liquide_common::sync::write_or_recover(&self.stats);
            stats.total_queries += 1;
        }

        // Try to get from cache
        let cache = liquide_common::sync::read_or_recover(&self.cache);
        if let Some(properties) = cache.get(&key) {
            let mut stats = liquide_common::sync::write_or_recover(&self.stats);
            stats.cache_hits += 1;
            Some(properties.clone())
        } else {
            let mut stats = liquide_common::sync::write_or_recover(&self.stats);
            stats.cache_misses += 1;
            None
        }
    }

    /// Insert a query result into the cache
    pub fn insert(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
        properties: PropertySet,
    ) {
        let key = CacheKey::new(element, classes, id, pseudo_classes);

        let mut cache = liquide_common::sync::write_or_recover(&self.cache);
        let mut order = liquide_common::sync::write_or_recover(&self.order);

        // If the key is already present, refresh its value but keep its original
        // insertion slot — we don't reset FIFO position on re-insert, which
        // keeps eviction deterministic and predictable under churn.
        if cache.contains_key(&key) {
            cache.insert(key, properties);
            return;
        }

        // Evict the actual oldest entry when at capacity (true FIFO via
        // `VecDeque<CacheKey>`; the previous `HashMap::keys().next()`
        // approach produced implementation-defined order).
        if self.max_size > 0 && cache.len() >= self.max_size {
            if let Some(oldest) = order.pop_front() {
                cache.remove(&oldest);
                let mut stats = liquide_common::sync::write_or_recover(&self.stats);
                stats.evictions += 1;
            }
        }

        order.push_back(key.clone());
        cache.insert(key, properties);
    }

    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = liquide_common::sync::write_or_recover(&self.cache);
        let mut order = liquide_common::sync::write_or_recover(&self.order);
        cache.clear();
        order.clear();

        // Reset stats except total queries
        let mut stats = liquide_common::sync::write_or_recover(&self.stats);
        stats.cache_hits = 0;
        stats.cache_misses = 0;
        stats.evictions = 0;
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        let cache = liquide_common::sync::read_or_recover(&self.cache);
        cache.len()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let stats = liquide_common::sync::read_or_recover(&self.stats);
        stats.clone()
    }

    /// Pre-warm cache with common queries
    pub fn prewarm<F>(
        &self,
        queries: &[(String, Vec<String>, Option<String>, Vec<String>)],
        compute_fn: F,
    ) where
        F: Fn(&str, &[String], Option<&str>, &[String]) -> PropertySet,
    {
        for (element, classes, id, pseudo_classes) in queries {
            let properties = compute_fn(element, classes, id.as_deref(), pseudo_classes);
            self.insert(element, classes, id.as_deref(), pseudo_classes, properties);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = QueryCache::new(100);

        let props = PropertySet::new();
        cache.insert("button", &[], None, &[], props.clone());

        let result = cache.get("button", &[], None, &[]);
        assert!(result.is_some());

        let stats = cache.stats();
        assert_eq!(stats.total_queries, 1);
        assert_eq!(stats.cache_hits, 1);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = QueryCache::new(2);

        let props = PropertySet::new();
        cache.insert("button", &[], None, &[], props.clone());
        cache.insert("input", &[], None, &[], props.clone());
        cache.insert("div", &[], None, &[], props.clone()); // Should evict button

        assert_eq!(cache.size(), 2);

        let stats = cache.stats();
        assert_eq!(stats.evictions, 1);
    }

    /// FIFO eviction must drop the **oldest** key first, not an arbitrary one.
    /// Regression: previous implementation used `HashMap::keys().next()`
    /// which has no ordering guarantee.
    #[test]
    fn fifo_eviction_is_order_preserving() {
        let cache = QueryCache::new(3);
        let props = PropertySet::new();
        cache.insert("a", &[], None, &[], props.clone());
        cache.insert("b", &[], None, &[], props.clone());
        cache.insert("c", &[], None, &[], props.clone());
        // Fourth insert must evict "a" (the oldest), not "b" or "c".
        cache.insert("d", &[], None, &[], props.clone());

        assert!(
            cache.get("a", &[], None, &[]).is_none(),
            "oldest entry should have been evicted"
        );
        assert!(cache.get("b", &[], None, &[]).is_some());
        assert!(cache.get("c", &[], None, &[]).is_some());
        assert!(cache.get("d", &[], None, &[]).is_some());
    }

    #[test]
    fn test_cache_hit_rate() {
        let cache = QueryCache::new(100);

        let props = PropertySet::new();
        cache.insert("button", &[], None, &[], props.clone());

        // 1 hit
        cache.get("button", &[], None, &[]);

        // 2 misses
        cache.get("input", &[], None, &[]);
        cache.get("div", &[], None, &[]);

        let stats = cache.stats();
        assert_eq!(stats.total_queries, 3);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 2);
        assert!((stats.hit_rate() - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_cache_clear_resets_stats() {
        let cache = QueryCache::new(100);
        let props = PropertySet::new();
        cache.insert("button", &[], None, &[], props.clone());
        cache.get("button", &[], None, &[]);
        cache.get("missing", &[], None, &[]);

        cache.clear();
        assert_eq!(cache.size(), 0);
        let stats = cache.stats();
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
    }

    #[test]
    fn test_cache_unlimited() {
        let cache = QueryCache::new(0); // 0 = unlimited
        let props = PropertySet::new();
        for i in 0..100 {
            cache.insert(&format!("el{i}"), &[], None, &[], props.clone());
        }
        assert_eq!(cache.size(), 100);
        let stats = cache.stats();
        assert_eq!(stats.evictions, 0);
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats {
            total_queries: 10,
            cache_hits: 7,
            cache_misses: 3,
            evictions: 0,
        };
        let rate = stats.hit_rate();
        assert!((rate - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_stats_hit_rate_zero() {
        let stats = CacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);
    }
}
