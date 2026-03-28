//! LayoutCache — per-node layout result cache with generational eviction.
//!
//! Each node can have multiple cached entries (one per distinct set of
//! parent constraints it was laid out with).  A generation counter is
//! bumped once per frame; entries from older generations are evicted to
//! bound memory.

use std::collections::HashMap;

use crate::constraints::LayoutConstraints;
use crate::result::LayoutResult;

/// A node identifier (matches `liquide_dom::NodeId`).
pub type NodeId = u64;

/// A single cache entry: constraints → result, tagged with a generation.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub constraints: LayoutConstraints,
    pub result: LayoutResult,
    pub generation: u64,
}

/// Per-node layout result cache.
///
/// Stores up to `max_entries_per_node` results per node (most nodes are
/// only ever laid out under one or two constraint configurations).
/// Old entries are evicted when the generation counter advances.
pub struct LayoutCache {
    cache: HashMap<NodeId, Vec<CacheEntry>>,
    generation: u64,
    max_entries_per_node: usize,
    hit_count: u64,
    miss_count: u64,
}

impl LayoutCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            generation: 0,
            max_entries_per_node: 4,
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// Create a cache with a custom per-node entry limit.
    pub fn with_max_entries(max_entries_per_node: usize) -> Self {
        Self {
            max_entries_per_node: max_entries_per_node.max(1),
            ..Self::new()
        }
    }

    /// Current generation counter.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Advance to the next generation and evict stale entries.
    ///
    /// Call this once per frame, before the layout pass starts.  Entries
    /// from generations older than `keep_generations` frames ago are removed.
    pub fn advance_generation(&mut self, keep_generations: u64) {
        self.generation += 1;
        let cutoff = self.generation.saturating_sub(keep_generations);
        self.cache.retain(|_, entries| {
            entries.retain(|e| e.generation >= cutoff);
            !entries.is_empty()
        });
    }

    /// Exact-match lookup: returns the cached result only if the
    /// constraints match exactly (bitwise equal floats).
    pub fn lookup(&mut self, node_id: NodeId, constraints: &LayoutConstraints) -> Option<&LayoutResult> {
        let entries = self.cache.get(&node_id)?;
        for entry in entries {
            if entry.constraints == *constraints {
                self.hit_count += 1;
                return Some(&entry.result);
            }
        }
        self.miss_count += 1;
        None
    }

    /// Fuzzy lookup: returns the cached result if any stored constraints
    /// are within `tolerance` pixels on every dimension.
    ///
    /// This is useful because floating-point layout arithmetic can produce
    /// constraints that differ by sub-pixel amounts across frames.
    pub fn lookup_fuzzy(
        &mut self,
        node_id: NodeId,
        constraints: &LayoutConstraints,
        tolerance: f32,
    ) -> Option<&LayoutResult> {
        let entries = self.cache.get(&node_id)?;
        for entry in entries {
            if entry.constraints.approx_eq(constraints, tolerance) {
                self.hit_count += 1;
                return Some(&entry.result);
            }
        }
        self.miss_count += 1;
        None
    }

    /// Store a layout result for the given node and constraints.
    ///
    /// If the node already has an entry with the same constraints, it is
    /// replaced.  If the per-node limit is reached, the oldest entry is
    /// evicted.
    pub fn store(
        &mut self,
        node_id: NodeId,
        constraints: LayoutConstraints,
        result: LayoutResult,
    ) {
        let generation = self.generation;
        let max = self.max_entries_per_node;

        let entries = self.cache.entry(node_id).or_insert_with(|| Vec::with_capacity(2));

        // Replace existing entry with same constraints.
        for entry in entries.iter_mut() {
            if entry.constraints == constraints {
                entry.result = result;
                entry.generation = generation;
                return;
            }
        }

        // Evict oldest entry if at capacity.
        if entries.len() >= max {
            // Find the entry with the smallest generation.
            let oldest_idx = entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.generation)
                .map(|(i, _)| i)
                .unwrap_or(0);
            entries.swap_remove(oldest_idx);
        }

        entries.push(CacheEntry {
            constraints,
            result,
            generation,
        });
    }

    /// Clear all cached entries for a single node.
    pub fn invalidate(&mut self, node_id: NodeId) {
        self.cache.remove(&node_id);
    }

    /// Clear cached entries for a node and all its descendants.
    ///
    /// `children_fn` should return the direct child node IDs for a given
    /// node.  The traversal is iterative (no stack overflow for deep trees).
    pub fn invalidate_subtree<F>(&mut self, node_id: NodeId, children_fn: F)
    where
        F: Fn(NodeId) -> Vec<NodeId>,
    {
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            self.cache.remove(&id);
            let kids = children_fn(id);
            stack.extend(kids);
        }
    }

    /// Clear the entire cache.
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
    }

    /// Number of nodes that have at least one cached entry.
    pub fn node_count(&self) -> usize {
        self.cache.len()
    }

    /// Total number of cache entries across all nodes.
    pub fn entry_count(&self) -> usize {
        self.cache.values().map(|v| v.len()).sum()
    }

    /// Cumulative cache hit count since creation (or last reset).
    pub fn hit_count(&self) -> u64 {
        self.hit_count
    }

    /// Cumulative cache miss count since creation (or last reset).
    pub fn miss_count(&self) -> u64 {
        self.miss_count
    }

    /// Cache hit rate as a fraction in [0.0, 1.0].
    ///
    /// Returns 0.0 if no lookups have been performed.
    pub fn hit_rate(&self) -> f32 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.hit_count as f32 / total as f32
        }
    }

    /// Reset hit/miss counters.
    pub fn reset_stats(&mut self) {
        self.hit_count = 0;
        self.miss_count = 0;
    }

    /// Maximum entries per node.
    pub fn max_entries_per_node(&self) -> usize {
        self.max_entries_per_node
    }

    /// Check whether a node has any cached entries.
    pub fn has_entries(&self, node_id: NodeId) -> bool {
        self.cache.get(&node_id).is_some_and(|v| !v.is_empty())
    }

    /// Get all cache entries for a node (for debugging / inspection).
    pub fn entries_for(&self, node_id: NodeId) -> &[CacheEntry] {
        self.cache.get(&node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self::new()
    }
}
