//! Conformance report generation.

use serde::{Deserialize, Serialize};

use crate::case::{CaseResult, Outcome};
use crate::suite::SuiteName;

/// Results for a single suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteResult {
    /// Which suite was executed.
    pub suite: SuiteName,
    /// Individual case results.
    pub cases: Vec<CaseResult>,
}

impl SuiteResult {
    /// Create a new suite result.
    #[must_use]
    pub fn new(suite: SuiteName) -> Self {
        Self {
            suite,
            cases: Vec::new(),
        }
    }

    /// Add a case result.
    pub fn add(&mut self, result: CaseResult) {
        self.cases.push(result);
    }

    /// Number of passed tests.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.outcome == Outcome::Pass)
            .count()
    }

    /// Number of failed tests.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.outcome == Outcome::Fail)
            .count()
    }

    /// Number of skipped tests.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.outcome == Outcome::Skip)
            .count()
    }

    /// Total number of test cases.
    #[must_use]
    pub fn total(&self) -> usize {
        self.cases.len()
    }

    /// Whether all non-skipped tests passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.failed() == 0
    }
}

/// Full conformance report across all suites.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// Target server that was tested.
    pub server: String,
    /// Timestamp (epoch seconds) when the run started.
    pub timestamp: u64,
    /// Protocol version tested against.
    pub protocol_version: String,
    /// Per-suite results.
    pub suites: Vec<SuiteResult>,
}

impl ConformanceReport {
    /// Create a new empty report.
    #[must_use]
    pub fn new(server: impl Into<String>, timestamp: u64) -> Self {
        Self {
            server: server.into(),
            timestamp,
            protocol_version: liquide_protocol::PROTOCOL_VERSION.to_string(),
            suites: Vec::new(),
        }
    }

    /// Add a suite result.
    pub fn add_suite(&mut self, result: SuiteResult) {
        self.suites.push(result);
    }

    /// Total passed across all suites.
    #[must_use]
    pub fn total_passed(&self) -> usize {
        self.suites.iter().map(|s| s.passed()).sum()
    }

    /// Total failed across all suites.
    #[must_use]
    pub fn total_failed(&self) -> usize {
        self.suites.iter().map(|s| s.failed()).sum()
    }

    /// Total skipped across all suites.
    #[must_use]
    pub fn total_skipped(&self) -> usize {
        self.suites.iter().map(|s| s.skipped()).sum()
    }

    /// Total test cases across all suites.
    #[must_use]
    pub fn total_cases(&self) -> usize {
        self.suites.iter().map(|s| s.total()).sum()
    }

    /// Whether all non-skipped tests passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.suites.iter().all(|s| s.all_passed())
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::ConformanceError::Serialization(e.to_string()))
    }

    /// Generate a human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Conformance Report — {}\n", self.server));
        out.push_str(&format!("Protocol: {}\n", self.protocol_version));
        out.push_str(&format!(
            "Overall: {} passed, {} failed, {} skipped (of {})\n\n",
            self.total_passed(),
            self.total_failed(),
            self.total_skipped(),
            self.total_cases(),
        ));

        for suite in &self.suites {
            out.push_str(&format!(
                "  {} — {} passed, {} failed, {} skipped\n",
                suite.suite.label(),
                suite.passed(),
                suite.failed(),
                suite.skipped(),
            ));
            for case in &suite.cases {
                out.push_str(&format!("    {case}\n"));
            }
            out.push('\n');
        }

        if self.all_passed() {
            out.push_str("Result: CONFORMANT\n");
        } else {
            out.push_str("Result: NON-CONFORMANT\n");
        }

        out
    }
}
