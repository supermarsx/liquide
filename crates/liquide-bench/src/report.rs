//! Benchmark report generation and formatting.
//!
//! Produces structured reports from benchmark results, including JSON
//! serialization and human-readable summary text.

use serde::{Deserialize, Serialize};

use crate::measurement::MetricSummary;
use crate::slo::SloResult;

/// Metadata about a benchmark report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    /// ISO 8601 timestamp of when the benchmark was run.
    pub timestamp: String,
    /// Hostname of the machine that ran the benchmark.
    pub hostname: String,
    /// Suite selection that was used.
    pub suite: String,
    /// Network profile name.
    pub network_profile: String,
    /// Duration in seconds.
    pub duration_secs: u64,
}

/// Result of a single benchmark suite run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchResult {
    /// Name of the suite (e.g. "compositor", "encoder", "protocol").
    pub suite_name: String,
    /// Workload profile label.
    pub workload: String,
    /// Number of samples collected.
    pub samples: u32,
    /// Statistical summaries for each metric.
    pub metrics: Vec<MetricSummary>,
    /// SLO check results.
    pub slo_results: Vec<SloResult>,
    /// Whether all SLOs passed.
    pub passed: bool,
}

impl BenchResult {
    /// Get the metric summary for a given metric name.
    #[must_use]
    pub fn metric(&self, name: &str) -> Option<&MetricSummary> {
        self.metrics.iter().find(|m| m.name == name)
    }
}

/// A complete benchmark report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    /// Report metadata.
    pub metadata: ReportMetadata,
    /// Results from each benchmark suite.
    pub results: Vec<BenchResult>,
}

impl BenchReport {
    /// Create a new empty report with metadata.
    #[must_use]
    pub fn new(metadata: ReportMetadata) -> Self {
        Self {
            metadata,
            results: Vec::new(),
        }
    }

    /// Add a benchmark result.
    pub fn add_result(&mut self, result: BenchResult) {
        self.results.push(result);
    }

    /// Whether all suites passed their SLOs.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }

    /// Total number of SLO violations across all suites.
    #[must_use]
    pub fn violation_count(&self) -> usize {
        self.results
            .iter()
            .flat_map(|r| &r.slo_results)
            .filter(|s| !s.passed)
            .count()
    }

    /// Serialize the report to JSON.
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::BenchError::Serialization(e.to_string()))
    }

    /// Produce a human-readable summary of the report.
    #[must_use]
    pub fn summary_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "=== LiquiDE Benchmark Report ===",
        ));
        lines.push(format!("Timestamp: {}", self.metadata.timestamp));
        lines.push(format!("Host:      {}", self.metadata.hostname));
        lines.push(format!("Suite:     {}", self.metadata.suite));
        lines.push(format!("Network:   {}", self.metadata.network_profile));
        lines.push(format!("Duration:  {}s", self.metadata.duration_secs));
        lines.push(String::new());

        for result in &self.results {
            let status = if result.passed { "PASS" } else { "FAIL" };
            lines.push(format!(
                "--- {} [{}] (workload: {}, samples: {}) ---",
                result.suite_name, status, result.workload, result.samples,
            ));

            for metric in &result.metrics {
                lines.push(format!("  {metric}"));
            }

            if !result.slo_results.is_empty() {
                lines.push("  SLOs:".to_string());
                for slo_result in &result.slo_results {
                    lines.push(format!("    {slo_result}"));
                }
            }
            lines.push(String::new());
        }

        let overall = if self.all_passed() {
            "ALL PASSED"
        } else {
            "FAILED"
        };
        lines.push(format!(
            "Overall: {} ({} violations)",
            overall,
            self.violation_count()
        ));

        lines.join("\n")
    }
}
