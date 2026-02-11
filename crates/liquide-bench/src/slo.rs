//! Service Level Objective (SLO) definitions and validation.
//!
//! Defines the performance targets from the LiquiDE specification and
//! provides checking logic to validate benchmark results against them.

use serde::{Deserialize, Serialize};

use crate::measurement::BenchMetrics;

/// How to compare a measured value against a threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SloComparator {
    /// The measured value must be strictly less than the threshold.
    LessThan,
    /// The measured value must be less than or equal to the threshold.
    LessThanOrEqual,
    /// The measured value must be strictly greater than the threshold.
    GreaterThan,
    /// The measured value must be greater than or equal to the threshold.
    GreaterThanOrEqual,
}

impl SloComparator {
    /// Check whether `actual` satisfies this comparator against `threshold`.
    #[must_use]
    pub fn check(&self, actual: f64, threshold: f64) -> bool {
        match self {
            Self::LessThan => actual < threshold,
            Self::LessThanOrEqual => actual <= threshold,
            Self::GreaterThan => actual > threshold,
            Self::GreaterThanOrEqual => actual >= threshold,
        }
    }

    /// Human-readable symbol for this comparator.
    #[must_use]
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
        }
    }
}

impl std::fmt::Display for SloComparator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.symbol())
    }
}

/// A single service level objective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slo {
    /// Name of the metric this SLO applies to.
    pub metric_name: String,
    /// Threshold value.
    pub threshold: f64,
    /// How to compare the actual value against the threshold.
    pub comparator: SloComparator,
    /// Unit of measurement (e.g. "ms", "fps", "Mbps").
    pub unit: String,
}

impl Slo {
    /// Create a new SLO.
    #[must_use]
    pub fn new(
        metric_name: impl Into<String>,
        threshold: f64,
        comparator: SloComparator,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            metric_name: metric_name.into(),
            threshold,
            comparator,
            unit: unit.into(),
        }
    }

    /// Check whether the given actual value passes this SLO.
    #[must_use]
    pub fn check(&self, actual_value: f64) -> SloResult {
        let passed = self.comparator.check(actual_value, self.threshold);
        SloResult {
            slo: self.clone(),
            actual_value,
            passed,
        }
    }
}

impl std::fmt::Display for Slo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {:.2} {}",
            self.metric_name,
            self.comparator.symbol(),
            self.threshold,
            self.unit
        )
    }
}

/// The result of checking a single SLO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloResult {
    /// The SLO that was checked.
    pub slo: Slo,
    /// The actual measured value.
    pub actual_value: f64,
    /// Whether the SLO was met.
    pub passed: bool,
}

impl std::fmt::Display for SloResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.passed { "PASS" } else { "FAIL" };
        write!(
            f,
            "[{}] {} = {:.2} {} (threshold: {} {:.2} {})",
            status,
            self.slo.metric_name,
            self.actual_value,
            self.slo.unit,
            self.slo.comparator.symbol(),
            self.slo.threshold,
            self.slo.unit,
        )
    }
}

/// A set of SLOs to validate against benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloSet {
    /// The name of this SLO set (e.g. "lan", "wan").
    pub name: String,
    /// The individual SLOs in this set.
    pub slos: Vec<Slo>,
}

impl SloSet {
    /// Default LAN SLO set from the LiquiDE specification.
    #[must_use]
    pub fn default_lan() -> Self {
        Self {
            name: "lan".to_string(),
            slos: vec![
                Slo::new("input_to_photon_p50", 16.0, SloComparator::LessThan, "ms"),
                Slo::new("input_to_photon_p99", 25.0, SloComparator::LessThan, "ms"),
                Slo::new("first_frame", 500.0, SloComparator::LessThan, "ms"),
                Slo::new("cursor_p50", 5.0, SloComparator::LessThan, "ms"),
                Slo::new("fps", 60.0, SloComparator::GreaterThanOrEqual, "fps"),
            ],
        }
    }

    /// Default WAN SLO set from the LiquiDE specification.
    #[must_use]
    pub fn default_wan() -> Self {
        Self {
            name: "wan".to_string(),
            slos: vec![
                Slo::new("input_to_photon_p50", 70.0, SloComparator::LessThan, "ms"),
                Slo::new("fps", 60.0, SloComparator::GreaterThanOrEqual, "fps"),
            ],
        }
    }

    /// Check a single metric value against the matching SLO in this set.
    ///
    /// Returns `None` if no SLO exists for the given metric name.
    #[must_use]
    pub fn check(&self, metric_name: &str, value: f64) -> Option<SloResult> {
        self.slos
            .iter()
            .find(|s| s.metric_name == metric_name)
            .map(|s| s.check(value))
    }

    /// Check all SLOs against values from a `BenchMetrics` collection.
    ///
    /// For each SLO, the appropriate statistic is extracted from the metrics:
    /// - Metrics ending in `_p50` use the 50th percentile
    /// - Metrics ending in `_p99` use the 99th percentile
    /// - `fps` uses the mean
    /// - Other metrics use the mean
    #[must_use]
    pub fn check_all(&self, metrics: &BenchMetrics) -> Vec<SloResult> {
        let mut results = Vec::new();
        for slo in &self.slos {
            let value = Self::extract_metric_value(metrics, &slo.metric_name);
            if let Some(v) = value {
                results.push(slo.check(v));
            }
        }
        results
    }

    /// Extract the appropriate statistical value for a metric name.
    fn extract_metric_value(metrics: &BenchMetrics, metric_name: &str) -> Option<f64> {
        // For percentile-suffixed metrics, look up the base metric and
        // extract the percentile.
        if let Some(base) = metric_name.strip_suffix("_p50") {
            return metrics.get(base).and_then(|ts| {
                if ts.count() == 0 {
                    None
                } else {
                    Some(ts.percentile(0.50))
                }
            });
        }
        if let Some(base) = metric_name.strip_suffix("_p99") {
            return metrics.get(base).and_then(|ts| {
                if ts.count() == 0 {
                    None
                } else {
                    Some(ts.percentile(0.99))
                }
            });
        }
        // For other metrics, use the mean.
        metrics.get(metric_name).and_then(|ts| {
            if ts.count() == 0 {
                None
            } else {
                Some(ts.mean())
            }
        })
    }

    /// Whether all SLOs passed.
    #[must_use]
    pub fn all_passed(&self, results: &[SloResult]) -> bool {
        results.iter().all(|r| r.passed)
    }
}
