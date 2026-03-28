//! Per-frame layout statistics.
//!
//! Tracks how many nodes were actually laid out vs. served from cache
//! vs. skipped entirely (not dirty), plus total wall-clock time.

/// Per-frame statistics collected during a layout pass.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStatistics {
    /// Number of nodes that performed full layout computation.
    pub nodes_laid_out: u32,
    /// Number of nodes that returned a cached result.
    pub nodes_cache_hit: u32,
    /// Number of nodes skipped entirely (not dirty, no dirty descendants).
    pub nodes_skipped: u32,
    /// Total layout time in microseconds.
    pub layout_time_us: u64,
}

impl FrameStatistics {
    /// Create zeroed statistics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of nodes processed (laid out + cache hit + skipped).
    pub fn total_nodes(&self) -> u32 {
        self.nodes_laid_out + self.nodes_cache_hit + self.nodes_skipped
    }

    /// Cache hit rate as a fraction in [0.0, 1.0].
    ///
    /// Denominator is nodes_laid_out + nodes_cache_hit (excludes skipped,
    /// since those never even attempted a cache lookup).
    pub fn cache_hit_rate(&self) -> f32 {
        let attempted = self.nodes_laid_out + self.nodes_cache_hit;
        if attempted == 0 {
            0.0
        } else {
            self.nodes_cache_hit as f32 / attempted as f32
        }
    }

    /// Record a node that performed full layout.
    pub fn record_layout(&mut self) {
        self.nodes_laid_out += 1;
    }

    /// Record a node served from cache.
    pub fn record_cache_hit(&mut self) {
        self.nodes_cache_hit += 1;
    }

    /// Record a node that was skipped (not dirty).
    pub fn record_skipped(&mut self) {
        self.nodes_skipped += 1;
    }

    /// Merge another frame's statistics into this one.
    pub fn merge(&mut self, other: &FrameStatistics) {
        self.nodes_laid_out += other.nodes_laid_out;
        self.nodes_cache_hit += other.nodes_cache_hit;
        self.nodes_skipped += other.nodes_skipped;
        self.layout_time_us += other.layout_time_us;
    }

    /// Average layout time per node that was actually laid out, in microseconds.
    pub fn avg_layout_time_per_node_us(&self) -> f64 {
        if self.nodes_laid_out == 0 {
            0.0
        } else {
            self.layout_time_us as f64 / self.nodes_laid_out as f64
        }
    }
}
