//! Measurement collection and statistical analysis.
//!
//! Provides time-series data collection with support for common statistical
//! operations: min, max, mean, percentiles, and standard deviation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A single measurement sample.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sample {
    /// Timestamp in microseconds (relative to benchmark start).
    pub timestamp_us: u64,
    /// The measured value.
    pub value: f64,
}

impl Sample {
    /// Create a new sample.
    #[must_use]
    pub fn new(timestamp_us: u64, value: f64) -> Self {
        Self {
            timestamp_us,
            value,
        }
    }
}

/// A time series of measurement samples.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimeSeries {
    samples: Vec<Sample>,
}

impl TimeSeries {
    /// Create an empty time series.
    #[must_use]
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Record a new sample.
    pub fn record(&mut self, timestamp_us: u64, value: f64) {
        self.samples.push(Sample::new(timestamp_us, value));
    }

    /// Number of samples recorded.
    #[must_use]
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// Whether the time series is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Minimum value, or `f64::NAN` if empty.
    #[must_use]
    pub fn min(&self) -> f64 {
        self.samples
            .iter()
            .map(|s| s.value)
            .fold(f64::INFINITY, f64::min)
    }

    /// Maximum value, or `f64::NAN` if empty.
    #[must_use]
    pub fn max(&self) -> f64 {
        self.samples
            .iter()
            .map(|s| s.value)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Arithmetic mean, or `f64::NAN` if empty.
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return f64::NAN;
        }
        let sum: f64 = self.samples.iter().map(|s| s.value).sum();
        sum / self.samples.len() as f64
    }

    /// Percentile value using the nearest-rank method.
    ///
    /// `p` must be in the range `0.0..=1.0`. Returns `f64::NAN` if empty.
    #[must_use]
    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return f64::NAN;
        }
        let mut values: Vec<f64> = self.samples.iter().map(|s| s.value).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p = p.clamp(0.0, 1.0);
        let rank = (p * (values.len() as f64 - 1.0)).round() as usize;
        let rank = rank.min(values.len() - 1);
        values[rank]
    }

    /// Population standard deviation, or `f64::NAN` if empty.
    #[must_use]
    pub fn std_dev(&self) -> f64 {
        if self.samples.is_empty() {
            return f64::NAN;
        }
        let mean = self.mean();
        let variance: f64 = self
            .samples
            .iter()
            .map(|s| {
                let diff = s.value - mean;
                diff * diff
            })
            .sum::<f64>()
            / self.samples.len() as f64;
        variance.sqrt()
    }

    /// All raw samples.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }
}

/// A collection of named time series for benchmark metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchMetrics {
    series: HashMap<String, TimeSeries>,
}

impl BenchMetrics {
    /// Create an empty metrics collection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            series: HashMap::new(),
        }
    }

    /// Record a sample for the given metric.
    pub fn record(&mut self, metric_name: &str, timestamp_us: u64, value: f64) {
        self.series
            .entry(metric_name.to_string())
            .or_default()
            .record(timestamp_us, value);
    }

    /// Get the time series for a metric, if it exists.
    #[must_use]
    pub fn get(&self, metric_name: &str) -> Option<&TimeSeries> {
        self.series.get(metric_name)
    }

    /// All recorded metric names.
    #[must_use]
    pub fn metric_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.series.keys().cloned().collect();
        names.sort();
        names
    }

    /// Compute a summary for each recorded metric.
    #[must_use]
    pub fn summary(&self) -> Vec<MetricSummary> {
        let mut names: Vec<&String> = self.series.keys().collect();
        names.sort();
        names
            .into_iter()
            .map(|name| {
                let ts = &self.series[name];
                MetricSummary {
                    name: name.clone(),
                    count: ts.count(),
                    min: ts.min(),
                    max: ts.max(),
                    mean: ts.mean(),
                    p50: ts.percentile(0.50),
                    p95: ts.percentile(0.95),
                    p99: ts.percentile(0.99),
                    std_dev: ts.std_dev(),
                }
            })
            .collect()
    }
}

/// Statistical summary of a single metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSummary {
    /// Metric name.
    pub name: String,
    /// Number of samples.
    pub count: usize,
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// Arithmetic mean.
    pub mean: f64,
    /// 50th percentile (median).
    pub p50: f64,
    /// 95th percentile.
    pub p95: f64,
    /// 99th percentile.
    pub p99: f64,
    /// Standard deviation.
    pub std_dev: f64,
}

impl std::fmt::Display for MetricSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: n={} min={:.2} max={:.2} mean={:.2} p50={:.2} p95={:.2} p99={:.2} stddev={:.2}",
            self.name,
            self.count,
            self.min,
            self.max,
            self.mean,
            self.p50,
            self.p95,
            self.p99,
            self.std_dev,
        )
    }
}
