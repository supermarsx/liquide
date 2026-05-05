//! Conformance report generation.

use serde::{Deserialize, Serialize};

use crate::case::{CaseResult, Outcome, Requirement};
use crate::config::ConformanceMode;
use crate::suite::SuiteName;

/// Aggregate certification status for a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceStatus {
    /// Live evidence proves all mandatory cases passed and no cases failed.
    Conformant,
    /// Live evidence contains failed cases.
    NonConformant,
    /// Evidence is incomplete, offline-only, or has mandatory skipped cases.
    Indeterminate,
}

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

    /// Number of skipped mandatory tests.
    #[must_use]
    pub fn mandatory_skipped(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.outcome == Outcome::Skip && c.requirement == Requirement::Mandatory)
            .count()
    }

    /// Total number of test cases.
    #[must_use]
    pub fn total(&self) -> usize {
        self.cases.len()
    }

    /// Whether this suite has no failures and no skipped mandatory tests.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.failed() == 0 && self.mandatory_skipped() == 0
    }
}

/// Full conformance report across all suites.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// Target server that was tested.
    pub server: String,
    /// Evidence-gathering mode used for this run.
    #[serde(default)]
    pub mode: ConformanceMode,
    /// Whether the target server was contacted during this run.
    #[serde(default)]
    pub server_contacted: bool,
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
        Self::new_for_run(server, timestamp, ConformanceMode::OfflineValidation, false)
    }

    /// Create a new report with explicit evidence metadata.
    #[must_use]
    pub fn new_for_run(
        server: impl Into<String>,
        timestamp: u64,
        mode: ConformanceMode,
        server_contacted: bool,
    ) -> Self {
        Self {
            server: server.into(),
            mode,
            server_contacted,
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

    /// Total mandatory skipped tests across all suites.
    #[must_use]
    pub fn total_mandatory_skipped(&self) -> usize {
        self.suites.iter().map(|s| s.mandatory_skipped()).sum()
    }

    /// Total test cases across all suites.
    #[must_use]
    pub fn total_cases(&self) -> usize {
        self.suites.iter().map(|s| s.total()).sum()
    }

    /// Certification status for this report.
    #[must_use]
    pub fn status(&self) -> ConformanceStatus {
        if self.mode == ConformanceMode::OfflineValidation || !self.server_contacted {
            return ConformanceStatus::Indeterminate;
        }

        if self.total_cases() == 0 {
            return ConformanceStatus::Indeterminate;
        }

        if self.total_failed() > 0 {
            return ConformanceStatus::NonConformant;
        }

        if self.total_mandatory_skipped() > 0 {
            return ConformanceStatus::Indeterminate;
        }

        ConformanceStatus::Conformant
    }

    /// Human-readable explanation for the aggregate status.
    #[must_use]
    pub fn status_reason(&self) -> String {
        if self.mode == ConformanceMode::OfflineValidation {
            return "offline validation did not contact the target server".to_string();
        }

        if !self.server_contacted {
            return "live server conformance did not contact the target server".to_string();
        }

        let failed = self.total_failed();
        if failed > 0 {
            return format!("{failed} conformance check(s) failed");
        }

        let mandatory_skipped = self.total_mandatory_skipped();
        if mandatory_skipped > 0 {
            return format!("{mandatory_skipped} mandatory conformance check(s) were skipped");
        }

        "all mandatory live conformance checks passed".to_string()
    }

    /// Whether this report certifies live server conformance.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.status() == ConformanceStatus::Conformant
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
        out.push_str(&format!("Mode: {}\n", self.mode.label()));
        out.push_str(&format!(
            "Server contact: {}\n",
            if self.server_contacted { "yes" } else { "no" }
        ));
        out.push_str(&format!("Protocol: {}\n", self.protocol_version));
        out.push_str(&format!(
            "Overall: {} passed, {} failed, {} skipped, {} mandatory skipped (of {})\n\n",
            self.total_passed(),
            self.total_failed(),
            self.total_skipped(),
            self.total_mandatory_skipped(),
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

        match self.status() {
            ConformanceStatus::Conformant => out.push_str("Result: CONFORMANT\n"),
            ConformanceStatus::NonConformant => out.push_str("Result: NON-CONFORMANT\n"),
            ConformanceStatus::Indeterminate => out.push_str("Result: INDETERMINATE\n"),
        }
        out.push_str(&format!("Reason: {}\n", self.status_reason()));

        out
    }
}
