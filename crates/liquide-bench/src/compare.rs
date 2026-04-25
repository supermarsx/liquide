//! Benchmark comparison and regression detection.
//!
//! Compares two benchmark reports (baseline vs current) and detects
//! performance regressions based on configurable thresholds.

use serde::{Deserialize, Serialize};

use crate::report::BenchReport;

/// Comparison of a single metric between baseline and current runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comparison {
    /// Name of the metric being compared.
    pub metric_name: String,
    /// Suite the metric belongs to.
    pub suite_name: String,
    /// Baseline value.
    pub baseline_value: f64,
    /// Current value.
    pub current_value: f64,
    /// Percentage change from baseline ((current - baseline) / baseline * 100).
    pub change_percent: f64,
    /// Whether this change is considered a regression.
    pub regression: bool,
}

impl std::fmt::Display for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let direction = if self.change_percent > 0.0 { "+" } else { "" };
        let status = if self.regression { "REGRESSION" } else { "ok" };
        write!(
            f,
            "[{}] {}/{}: {:.2} -> {:.2} ({}{:.1}%)",
            status,
            self.suite_name,
            self.metric_name,
            self.baseline_value,
            self.current_value,
            direction,
            self.change_percent,
        )
    }
}

/// Threshold for deciding whether a metric change is a regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionThreshold {
    /// Name of the metric.
    pub metric_name: String,
    /// Maximum acceptable regression percentage (positive = worse).
    ///
    /// For latency metrics, a positive change is a regression.
    /// For throughput metrics, a negative change is a regression.
    pub max_regression_percent: f64,
    /// Whether higher values are worse (true for latency, false for throughput).
    pub higher_is_worse: bool,
}

impl RegressionThreshold {
    /// Create a threshold where higher values are worse (latency-style).
    #[must_use]
    pub fn latency(metric_name: impl Into<String>, max_percent: f64) -> Self {
        Self {
            metric_name: metric_name.into(),
            max_regression_percent: max_percent,
            higher_is_worse: true,
        }
    }

    /// Create a threshold where lower values are worse (throughput-style).
    #[must_use]
    pub fn throughput(metric_name: impl Into<String>, max_percent: f64) -> Self {
        Self {
            metric_name: metric_name.into(),
            max_regression_percent: max_percent,
            higher_is_worse: false,
        }
    }
}

/// Report comparing baseline and current benchmark runs.
#[derive(Debug, Clone)]
pub struct ComparisonReport {
    /// The baseline report.
    pub baseline: BenchReport,
    /// The current report.
    pub current: BenchReport,
    /// Regression thresholds to apply.
    pub thresholds: Vec<RegressionThreshold>,
}

impl ComparisonReport {
    /// Create a new comparison report.
    #[must_use]
    pub fn new(
        baseline: BenchReport,
        current: BenchReport,
        thresholds: Vec<RegressionThreshold>,
    ) -> Self {
        Self {
            baseline,
            current,
            thresholds,
        }
    }

    /// Default regression thresholds for standard metrics.
    #[must_use]
    pub fn default_thresholds() -> Vec<RegressionThreshold> {
        vec![
            RegressionThreshold::latency("compose_time", 10.0),
            RegressionThreshold::latency("damage_compute_time", 10.0),
            RegressionThreshold::latency("input_to_photon", 5.0),
            RegressionThreshold::latency("cursor", 5.0),
            RegressionThreshold::latency("encode_time", 10.0),
            RegressionThreshold::latency("serialize_time_us", 10.0),
            RegressionThreshold::latency("deserialize_time_us", 10.0),
            RegressionThreshold::latency("rtt", 5.0),
            RegressionThreshold::latency("first_frame", 10.0),
            RegressionThreshold::throughput("fps", 5.0),
            RegressionThreshold::throughput("compression_ratio", 10.0),
            RegressionThreshold::throughput("encode_throughput_mbps", 10.0),
            RegressionThreshold::throughput("messages_per_sec", 10.0),
            RegressionThreshold::throughput("bandwidth_mbps", 5.0),
        ]
    }

    /// Compare all metrics across matching suites.
    #[must_use]
    pub fn compare(&self) -> Vec<Comparison> {
        let mut comparisons = Vec::new();

        for baseline_result in &self.baseline.results {
            // Find matching suite in current.
            let current_result = self
                .current
                .results
                .iter()
                .find(|r| r.suite_name == baseline_result.suite_name);

            let Some(current_result) = current_result else {
                continue;
            };

            for baseline_metric in &baseline_result.metrics {
                let current_metric = current_result
                    .metrics
                    .iter()
                    .find(|m| m.name == baseline_metric.name);

                let Some(current_metric) = current_metric else {
                    continue;
                };

                let baseline_value = baseline_metric.mean;
                let current_value = current_metric.mean;

                let change_percent = if baseline_value.abs() > f64::EPSILON {
                    (current_value - baseline_value) / baseline_value * 100.0
                } else {
                    0.0
                };

                let regression = self.is_regression(&baseline_metric.name, change_percent);

                comparisons.push(Comparison {
                    metric_name: baseline_metric.name.clone(),
                    suite_name: baseline_result.suite_name.clone(),
                    baseline_value,
                    current_value,
                    change_percent,
                    regression,
                });
            }
        }

        comparisons
    }

    /// Whether any metric shows a regression.
    #[must_use]
    pub fn has_regressions(&self) -> bool {
        self.compare().iter().any(|c| c.regression)
    }

    /// Check whether a given change percentage constitutes a regression for
    /// the named metric.
    fn is_regression(&self, metric_name: &str, change_percent: f64) -> bool {
        let threshold = self
            .thresholds
            .iter()
            .find(|t| t.metric_name == metric_name);

        let Some(threshold) = threshold else {
            // No threshold defined; not a regression.
            return false;
        };

        if threshold.higher_is_worse {
            // For latency: positive change = worse.
            change_percent > threshold.max_regression_percent
        } else {
            // For throughput: negative change = worse.
            change_percent < -threshold.max_regression_percent
        }
    }

    /// Produce a human-readable summary of the comparison.
    #[must_use]
    pub fn summary_text(&self) -> String {
        let comparisons = self.compare();
        let mut lines = Vec::new();

        lines.push("=== Benchmark Comparison ===".to_string());
        lines.push(format!(
            "Baseline: {} ({})",
            self.baseline.metadata.timestamp, self.baseline.metadata.suite
        ));
        lines.push(format!(
            "Current:  {} ({})",
            self.current.metadata.timestamp, self.current.metadata.suite
        ));
        lines.push(String::new());

        let regressions: Vec<&Comparison> = comparisons.iter().filter(|c| c.regression).collect();

        if regressions.is_empty() {
            lines.push("No regressions detected.".to_string());
        } else {
            lines.push(format!("Regressions detected: {}", regressions.len()));
            for r in &regressions {
                lines.push(format!("  {r}"));
            }
        }

        lines.push(String::new());
        lines.push("All comparisons:".to_string());
        for c in &comparisons {
            lines.push(format!("  {c}"));
        }

        lines.join("\n")
    }
}
