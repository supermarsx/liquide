//! System-wide metrics collection with thread-safe counters, gauges, and histograms.
//!
//! Modelled after Prometheus / Chrome DevTools performance metrics.
//! All counters use [`AtomicU64`] for lock-free updates from any thread.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// The kind of metric being tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    /// Monotonically increasing value (e.g. total frame count).
    Counter,
    /// Point-in-time measurement that can go up or down (e.g. memory used).
    Gauge,
    /// Distribution of observed values bucketed by magnitude.
    Histogram,
}

/// A single metric value with optional labels.
#[derive(Debug, Clone)]
pub struct Metric {
    /// Human-readable name (e.g. `"frame_count"`).
    pub name: String,
    /// What kind of metric this is.
    pub kind: MetricKind,
    /// Current value (for counters/gauges) or observation count (histograms).
    pub value: u64,
    /// Optional key-value labels for dimensional slicing.
    pub labels: Vec<(String, String)>,
}

/// A snapshot of all registered metrics at a point in time.
#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    pub metrics: Vec<Metric>,
}

// ---------------------------------------------------------------------------
// Well-known metric names
// ---------------------------------------------------------------------------

/// Total number of frames rendered since start.
pub const FRAME_COUNT: &str = "frame_count";
/// Frames that missed their deadline and were dropped.
pub const DROPPED_FRAMES: &str = "dropped_frames";
/// Number of paint (display-list build) calls.
pub const PAINT_CALLS: &str = "paint_calls";
/// Number of full layout recalculations.
pub const LAYOUT_RECALCS: &str = "layout_recalcs";
/// Number of style recalculations.
pub const STYLE_RECALCS: &str = "style_recalcs";
/// Current heap memory in use (bytes).
pub const MEMORY_USED_BYTES: &str = "memory_used_bytes";
/// Number of active compositor surfaces.
pub const SURFACE_COUNT: &str = "surface_count";
/// Number of running animations.
pub const ANIMATION_COUNT: &str = "animation_count";
/// Cache hits (lookup succeeded).
pub const CACHE_HITS: &str = "cache_hits";
/// Cache misses (lookup failed, had to recompute).
pub const CACHE_MISSES: &str = "cache_misses";

/// All well-known metric definitions: (name, kind).
const BUILTIN_METRICS: &[(&str, MetricKind)] = &[
    (FRAME_COUNT, MetricKind::Counter),
    (DROPPED_FRAMES, MetricKind::Counter),
    (PAINT_CALLS, MetricKind::Counter),
    (LAYOUT_RECALCS, MetricKind::Counter),
    (STYLE_RECALCS, MetricKind::Counter),
    (MEMORY_USED_BYTES, MetricKind::Gauge),
    (SURFACE_COUNT, MetricKind::Gauge),
    (ANIMATION_COUNT, MetricKind::Gauge),
    (CACHE_HITS, MetricKind::Counter),
    (CACHE_MISSES, MetricKind::Counter),
];

// ---------------------------------------------------------------------------
// Internal atomic metric storage
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct MetricEntry {
    kind: MetricKind,
    value: AtomicU64,
    labels: Vec<(String, String)>,
    /// For histograms: bucket boundaries (upper bound, count).
    histogram_buckets: Option<RwLock<Vec<(u64, AtomicU64)>>>,
}

// ---------------------------------------------------------------------------
// MetricsRegistry
// ---------------------------------------------------------------------------

/// Thread-safe registry of named metrics.
///
/// Counters and gauges are backed by [`AtomicU64`] for zero-contention updates.
/// Histograms maintain a sorted list of bucket boundaries with atomic counts.
#[derive(Debug)]
pub struct MetricsRegistry {
    entries: RwLock<HashMap<String, Arc<MetricEntry>>>,
}

impl MetricsRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Create a registry pre-populated with all well-known built-in metrics.
    pub fn with_builtins() -> Self {
        let reg = Self::new();
        for &(name, kind) in BUILTIN_METRICS {
            reg.register(name, kind);
        }
        reg
    }

    /// Register a new metric. If a metric with the same name already exists
    /// this is a no-op (the existing metric is kept).
    pub fn register(&self, name: &str, kind: MetricKind) {
        let mut map = self.entries.write().unwrap();
        if map.contains_key(name) {
            return;
        }
        let histogram_buckets = if kind == MetricKind::Histogram {
            Some(RwLock::new(Vec::new()))
        } else {
            None
        };
        map.insert(
            name.to_string(),
            Arc::new(MetricEntry {
                kind,
                value: AtomicU64::new(0),
                labels: Vec::new(),
                histogram_buckets,
            }),
        );
    }

    /// Register a metric with labels.
    pub fn register_with_labels(
        &self,
        name: &str,
        kind: MetricKind,
        labels: Vec<(String, String)>,
    ) {
        let mut map = self.entries.write().unwrap();
        if map.contains_key(name) {
            return;
        }
        let histogram_buckets = if kind == MetricKind::Histogram {
            Some(RwLock::new(Vec::new()))
        } else {
            None
        };
        map.insert(
            name.to_string(),
            Arc::new(MetricEntry {
                kind,
                value: AtomicU64::new(0),
                labels,
                histogram_buckets,
            }),
        );
    }

    /// Increment a counter by `delta`. No-op if the metric is not registered.
    pub fn increment(&self, name: &str, delta: u64) {
        if let Some(entry) = self.get_entry(name) {
            entry.value.fetch_add(delta, Ordering::Relaxed);
        }
    }

    /// Set a gauge to an absolute value.
    pub fn set(&self, name: &str, value: u64) {
        if let Some(entry) = self.get_entry(name) {
            entry.value.store(value, Ordering::Relaxed);
        }
    }

    /// Read the current value of a metric. Returns `None` if not registered.
    pub fn get(&self, name: &str) -> Option<u64> {
        self.get_entry(name)
            .map(|e| e.value.load(Ordering::Relaxed))
    }

    /// Observe a value for a histogram metric.
    /// The value is added to the appropriate bucket and the total count is
    /// incremented.
    pub fn observe(&self, name: &str, value: u64) {
        if let Some(entry) = self.get_entry(name) {
            entry.value.fetch_add(1, Ordering::Relaxed);
            if let Some(ref buckets_lock) = entry.histogram_buckets {
                let buckets = buckets_lock.read().unwrap();
                for (bound, count) in buckets.iter() {
                    if value <= *bound {
                        count.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                }
            }
        }
    }

    /// Configure histogram bucket boundaries. Must be called before observations.
    pub fn set_histogram_buckets(&self, name: &str, boundaries: &[u64]) {
        if let Some(entry) = self.get_entry(name) {
            if let Some(ref buckets_lock) = entry.histogram_buckets {
                let mut buckets = buckets_lock.write().unwrap();
                buckets.clear();
                for &b in boundaries {
                    buckets.push((b, AtomicU64::new(0)));
                }
            }
        }
    }

    /// Capture a snapshot of all registered metrics.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let map = self.entries.read().unwrap();
        let mut metrics: Vec<Metric> = map
            .iter()
            .map(|(name, entry)| Metric {
                name: name.clone(),
                kind: entry.kind,
                value: entry.value.load(Ordering::Relaxed),
                labels: entry.labels.clone(),
            })
            .collect();
        metrics.sort_by(|a, b| a.name.cmp(&b.name));
        MetricsSnapshot { metrics }
    }

    /// Returns the number of registered metrics.
    pub fn count(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Reset all counters/gauges to zero.
    pub fn reset(&self) {
        let map = self.entries.read().unwrap();
        for entry in map.values() {
            entry.value.store(0, Ordering::Relaxed);
        }
    }

    /// Check whether a metric with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.read().unwrap().contains_key(name)
    }

    // --- internal helpers ---

    fn get_entry(&self, name: &str) -> Option<Arc<MetricEntry>> {
        let map = self.entries.read().unwrap();
        map.get(name).cloned()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_registry_is_empty() {
        let reg = MetricsRegistry::new();
        assert_eq!(reg.count(), 0);
        assert!(reg.snapshot().metrics.is_empty());
    }

    #[test]
    fn register_and_get() {
        let reg = MetricsRegistry::new();
        reg.register("my_counter", MetricKind::Counter);
        assert!(reg.contains("my_counter"));
        assert_eq!(reg.get("my_counter"), Some(0));
    }

    #[test]
    fn increment_counter() {
        let reg = MetricsRegistry::new();
        reg.register("hits", MetricKind::Counter);
        reg.increment("hits", 1);
        reg.increment("hits", 5);
        assert_eq!(reg.get("hits"), Some(6));
    }

    #[test]
    fn set_gauge() {
        let reg = MetricsRegistry::new();
        reg.register("mem", MetricKind::Gauge);
        reg.set("mem", 1024);
        assert_eq!(reg.get("mem"), Some(1024));
        reg.set("mem", 2048);
        assert_eq!(reg.get("mem"), Some(2048));
    }

    #[test]
    fn unregistered_metric_returns_none() {
        let reg = MetricsRegistry::new();
        assert_eq!(reg.get("nonexistent"), None);
    }

    #[test]
    fn increment_unregistered_is_noop() {
        let reg = MetricsRegistry::new();
        reg.increment("ghost", 42);
        assert_eq!(reg.get("ghost"), None);
    }

    #[test]
    fn builtin_metrics_registered() {
        let reg = MetricsRegistry::with_builtins();
        assert_eq!(reg.count(), BUILTIN_METRICS.len());
        assert!(reg.contains(FRAME_COUNT));
        assert!(reg.contains(DROPPED_FRAMES));
        assert!(reg.contains(PAINT_CALLS));
        assert!(reg.contains(LAYOUT_RECALCS));
        assert!(reg.contains(STYLE_RECALCS));
        assert!(reg.contains(MEMORY_USED_BYTES));
        assert!(reg.contains(SURFACE_COUNT));
        assert!(reg.contains(ANIMATION_COUNT));
        assert!(reg.contains(CACHE_HITS));
        assert!(reg.contains(CACHE_MISSES));
    }

    #[test]
    fn snapshot_captures_all() {
        let reg = MetricsRegistry::new();
        reg.register("a", MetricKind::Counter);
        reg.register("b", MetricKind::Gauge);
        reg.increment("a", 10);
        reg.set("b", 42);
        let snap = reg.snapshot();
        assert_eq!(snap.metrics.len(), 2);
        let a = snap.metrics.iter().find(|m| m.name == "a").unwrap();
        assert_eq!(a.value, 10);
        assert_eq!(a.kind, MetricKind::Counter);
        let b = snap.metrics.iter().find(|m| m.name == "b").unwrap();
        assert_eq!(b.value, 42);
        assert_eq!(b.kind, MetricKind::Gauge);
    }

    #[test]
    fn snapshot_is_sorted() {
        let reg = MetricsRegistry::new();
        reg.register("zebra", MetricKind::Counter);
        reg.register("alpha", MetricKind::Counter);
        reg.register("mid", MetricKind::Counter);
        let snap = reg.snapshot();
        let names: Vec<&str> = snap.metrics.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zebra"]);
    }

    #[test]
    fn duplicate_register_is_noop() {
        let reg = MetricsRegistry::new();
        reg.register("x", MetricKind::Counter);
        reg.increment("x", 5);
        reg.register("x", MetricKind::Gauge); // should not reset
        assert_eq!(reg.get("x"), Some(5));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn reset_clears_values() {
        let reg = MetricsRegistry::new();
        reg.register("c1", MetricKind::Counter);
        reg.register("g1", MetricKind::Gauge);
        reg.increment("c1", 100);
        reg.set("g1", 999);
        reg.reset();
        assert_eq!(reg.get("c1"), Some(0));
        assert_eq!(reg.get("g1"), Some(0));
    }

    #[test]
    fn register_with_labels() {
        let reg = MetricsRegistry::new();
        reg.register_with_labels(
            "req_count",
            MetricKind::Counter,
            vec![("method".into(), "GET".into())],
        );
        reg.increment("req_count", 3);
        let snap = reg.snapshot();
        let m = &snap.metrics[0];
        assert_eq!(m.labels.len(), 1);
        assert_eq!(m.labels[0], ("method".into(), "GET".into()));
        assert_eq!(m.value, 3);
    }

    #[test]
    fn histogram_observe() {
        let reg = MetricsRegistry::new();
        reg.register("latency", MetricKind::Histogram);
        reg.set_histogram_buckets("latency", &[1000, 5000, 10_000, 50_000]);
        reg.observe("latency", 500); // bucket 1000
        reg.observe("latency", 3000); // bucket 5000
        reg.observe("latency", 9000); // bucket 10000
        // Total observation count stored in value
        assert_eq!(reg.get("latency"), Some(3));
    }

    #[test]
    fn contains_returns_false_for_missing() {
        let reg = MetricsRegistry::new();
        assert!(!reg.contains("nope"));
    }

    #[test]
    fn concurrent_increments() {
        use std::thread;
        let reg = Arc::new(MetricsRegistry::new());
        reg.register("counter", MetricKind::Counter);
        let mut handles = Vec::new();
        for _ in 0..8 {
            let r = reg.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    r.increment("counter", 1);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(reg.get("counter"), Some(8000));
    }

    #[test]
    fn metric_kind_debug() {
        assert_eq!(format!("{:?}", MetricKind::Counter), "Counter");
        assert_eq!(format!("{:?}", MetricKind::Gauge), "Gauge");
        assert_eq!(format!("{:?}", MetricKind::Histogram), "Histogram");
    }

    #[test]
    fn default_registry() {
        let reg = MetricsRegistry::default();
        assert_eq!(reg.count(), 0);
    }
}
