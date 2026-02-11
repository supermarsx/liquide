//! System metrics collection and aggregation.

use serde::{Deserialize, Serialize};

/// A single metrics data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    /// Timestamp (epoch seconds).
    pub timestamp: u64,
    /// Metric value.
    pub value: f64,
}

/// Named metric time series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSeries {
    /// Metric name.
    pub name: String,
    /// Server name (or "aggregate").
    pub server: String,
    /// Data points.
    pub points: Vec<MetricPoint>,
}

/// Aggregate metrics snapshot across all servers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub total_sessions: u32,
    pub total_users: u32,
    pub avg_fps: f32,
    pub avg_latency_ms: f32,
    pub total_bandwidth_in_bps: u64,
    pub total_bandwidth_out_bps: u64,
    pub timestamp: u64,
}

/// Metrics collector storing time-series data.
pub struct MetricsCollector {
    snapshots: Vec<MetricsSnapshot>,
    series: Vec<MetricSeries>,
    max_retention_points: usize,
}

impl MetricsCollector {
    /// Create a new collector.
    #[must_use]
    pub fn new(max_retention_points: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            series: Vec::new(),
            max_retention_points,
        }
    }

    /// Record a snapshot.
    pub fn record_snapshot(&mut self, snapshot: MetricsSnapshot) {
        self.snapshots.push(snapshot);
        if self.snapshots.len() > self.max_retention_points {
            self.snapshots.remove(0);
        }
    }

    /// Record a metric data point into a named series.
    pub fn record_point(&mut self, name: &str, server: &str, timestamp: u64, value: f64) {
        let series = self
            .series
            .iter_mut()
            .find(|s| s.name == name && s.server == server);

        match series {
            Some(s) => {
                s.points.push(MetricPoint { timestamp, value });
                if s.points.len() > self.max_retention_points {
                    s.points.remove(0);
                }
            }
            None => {
                self.series.push(MetricSeries {
                    name: name.to_string(),
                    server: server.to_string(),
                    points: vec![MetricPoint { timestamp, value }],
                });
            }
        }
    }

    /// Get the latest snapshot.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<&MetricsSnapshot> {
        self.snapshots.last()
    }

    /// Get all snapshots.
    #[must_use]
    pub fn snapshots(&self) -> &[MetricsSnapshot] {
        &self.snapshots
    }

    /// Get a specific series.
    #[must_use]
    pub fn get_series(&self, name: &str, server: &str) -> Option<&MetricSeries> {
        self.series
            .iter()
            .find(|s| s.name == name && s.server == server)
    }

    /// Total snapshot count.
    #[must_use]
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Total series count.
    #[must_use]
    pub fn series_count(&self) -> usize {
        self.series.len()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        // Default: ~17,280 points = 24h at 5s intervals.
        Self::new(17_280)
    }
}
