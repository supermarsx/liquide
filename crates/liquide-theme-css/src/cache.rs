//! CSS query caching for theme engine

use crate::property::PropertySet;
use std::collections::HashMap;
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
    fn new(
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> Self {
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

        // Evict if at capacity (simple FIFO eviction)
        if self.max_size > 0 && cache.len() >= self.max_size {
            // Remove first entry (FIFO)
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
                let mut stats = liquide_common::sync::write_or_recover(&self.stats);
                stats.evictions += 1;
            }
        }
        
        cache.insert(key, properties);
    }
    
    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = liquide_common::sync::write_or_recover(&self.cache);
        cache.clear();

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
    pub fn prewarm<F>(&self, queries: &[(String, Vec<String>, Option<String>, Vec<String>)], compute_fn: F)
    where
        F: Fn(&str, &[String], Option<&str>, &[String]) -> PropertySet,
    {
        for (element, classes, id, pseudo_classes) in queries {
            let properties = compute_fn(
                element,
                classes,
                id.as_deref(),
                pseudo_classes,
            );
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
}
