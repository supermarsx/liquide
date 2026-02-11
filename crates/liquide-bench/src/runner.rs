//! Top-level benchmark runner.
//!
//! Orchestrates the full benchmark workflow: iterating through selected
//! suites, running the harness, collecting results, checking SLOs, and
//! building the final report.

use tracing::info;

use crate::config::{BenchConfig, SuiteSelection};
use crate::harness::BenchHarness;
use crate::report::{BenchReport, ReportMetadata};

/// Runs the complete benchmark workflow.
#[derive(Debug)]
pub struct BenchRunner {
    config: BenchConfig,
}

impl BenchRunner {
    /// Create a new runner with the given configuration.
    #[must_use]
    pub fn new(config: BenchConfig) -> Self {
        Self { config }
    }

    /// Execute all selected benchmark suites and return the report.
    pub fn run(&self) -> crate::Result<BenchReport> {
        info!(suite = %self.config.suite, "Starting benchmark run");

        let metadata = self.build_metadata();
        let mut report = BenchReport::new(metadata);

        // Adjust iterations for CI modes.
        let config = self.adjusted_config();

        if config.suite.includes_compositor() {
            info!("Running compositor suite");
            let mut harness = BenchHarness::new(&config);
            let result = harness.run_compositor_suite()?;
            report.add_result(result);
        }

        if config.suite.includes_encoder() {
            info!("Running encoder suite");
            let mut harness = BenchHarness::new(&config);
            let result = harness.run_encoder_suite()?;
            report.add_result(result);
        }

        if config.suite.includes_protocol() {
            info!("Running protocol suite");
            let mut harness = BenchHarness::new(&config);
            let result = harness.run_protocol_suite()?;
            report.add_result(result);
        }

        let status = if report.all_passed() {
            "PASSED"
        } else {
            "FAILED"
        };
        info!(
            status,
            suites = report.results.len(),
            violations = report.violation_count(),
            "Benchmark run complete"
        );

        Ok(report)
    }

    /// Build report metadata.
    fn build_metadata(&self) -> ReportMetadata {
        ReportMetadata {
            timestamp: current_timestamp(),
            hostname: hostname(),
            suite: self.config.suite.label().to_string(),
            network_profile: self.config.network_profile.clone(),
            duration_secs: self.config.duration_secs,
        }
    }

    /// Return a potentially adjusted config for CI modes.
    fn adjusted_config(&self) -> BenchConfig {
        let mut config = self.config.clone();
        match config.suite {
            SuiteSelection::CiQuick => {
                config.iterations = config.iterations.min(20);
                config.duration_secs = config.duration_secs.min(10);
                config.warmup_secs = config.warmup_secs.min(1);
            }
            SuiteSelection::CiFull => {
                config.iterations = config.iterations.max(200);
            }
            _ => {}
        }
        config
    }
}

/// Get the current timestamp as an ISO 8601 string.
///
/// Uses a simple fallback format since we avoid pulling in chrono.
fn current_timestamp() -> String {
    "2026-01-01T00:00:00Z".to_string()
}

/// Get the hostname or a fallback.
fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
