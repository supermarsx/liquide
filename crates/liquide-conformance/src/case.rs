//! Test case definition and categorisation.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::suite::SuiteName;

/// How important a test case is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Requirement {
    /// Must pass for conformance.
    Mandatory,
    /// Nice to have; failure is noted but not blocking.
    Optional,
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mandatory => write!(f, "mandatory"),
            Self::Optional => write!(f, "optional"),
        }
    }
}

/// Category within a suite for grouping related tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestCategory {
    /// Protocol wire format.
    WireFormat,
    /// State machine transitions.
    StateMachine,
    /// Data integrity and validation.
    DataIntegrity,
    /// Error handling and edge cases.
    ErrorHandling,
    /// Timing and ordering.
    Ordering,
    /// Security constraints.
    Security,
}

impl fmt::Display for TestCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WireFormat => write!(f, "wire-format"),
            Self::StateMachine => write!(f, "state-machine"),
            Self::DataIntegrity => write!(f, "data-integrity"),
            Self::ErrorHandling => write!(f, "error-handling"),
            Self::Ordering => write!(f, "ordering"),
            Self::Security => write!(f, "security"),
        }
    }
}

/// Outcome of running a single test case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Test passed.
    Pass,
    /// Test failed.
    Fail,
    /// Test was skipped (e.g. missing credentials).
    Skip,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::Fail => write!(f, "FAIL"),
            Self::Skip => write!(f, "SKIP"),
        }
    }
}

/// A single conformance test case definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Unique identifier (e.g. `HS-001`).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Which suite this belongs to.
    pub suite: SuiteName,
    /// Sub-category within the suite.
    pub category: TestCategory,
    /// Whether the test is mandatory or optional.
    pub requirement: Requirement,
    /// Detailed description of what the test validates.
    pub description: String,
    /// Spec section reference (e.g. `§7.2`).
    pub spec_ref: String,
}

impl TestCase {
    /// Create a new mandatory test case.
    #[must_use]
    pub fn mandatory(
        id: impl Into<String>,
        name: impl Into<String>,
        suite: SuiteName,
        category: TestCategory,
        description: impl Into<String>,
        spec_ref: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            suite,
            category,
            requirement: Requirement::Mandatory,
            description: description.into(),
            spec_ref: spec_ref.into(),
        }
    }

    /// Create a new optional test case.
    #[must_use]
    pub fn optional(
        id: impl Into<String>,
        name: impl Into<String>,
        suite: SuiteName,
        category: TestCategory,
        description: impl Into<String>,
        spec_ref: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            suite,
            category,
            requirement: Requirement::Optional,
            description: description.into(),
            spec_ref: spec_ref.into(),
        }
    }

    /// Whether this test is mandatory.
    #[must_use]
    pub fn is_mandatory(&self) -> bool {
        self.requirement == Requirement::Mandatory
    }
}

impl fmt::Display for TestCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} ({}, {})",
            self.id, self.name, self.requirement, self.suite
        )
    }
}

/// Result of executing a single test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    /// Test case identifier.
    pub case_id: String,
    /// Test case name.
    pub case_name: String,
    /// Suite this belongs to.
    pub suite: SuiteName,
    /// Whether passed, failed, or skipped.
    pub outcome: Outcome,
    /// Duration in microseconds.
    pub duration_us: u64,
    /// Failure reason (empty on pass/skip).
    pub message: String,
}

impl CaseResult {
    /// Create a passing result.
    #[must_use]
    pub fn pass(case: &TestCase, duration_us: u64) -> Self {
        Self {
            case_id: case.id.clone(),
            case_name: case.name.clone(),
            suite: case.suite,
            outcome: Outcome::Pass,
            duration_us,
            message: String::new(),
        }
    }

    /// Create a failing result.
    #[must_use]
    pub fn fail(case: &TestCase, duration_us: u64, message: impl Into<String>) -> Self {
        Self {
            case_id: case.id.clone(),
            case_name: case.name.clone(),
            suite: case.suite,
            outcome: Outcome::Fail,
            duration_us,
            message: message.into(),
        }
    }

    /// Create a skipped result.
    #[must_use]
    pub fn skip(case: &TestCase, reason: impl Into<String>) -> Self {
        Self {
            case_id: case.id.clone(),
            case_name: case.name.clone(),
            suite: case.suite,
            outcome: Outcome::Skip,
            duration_us: 0,
            message: reason.into(),
        }
    }
}

impl fmt::Display for CaseResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "[{}] {} — {}", self.outcome, self.case_id, self.case_name)
        } else {
            write!(
                f,
                "[{}] {} — {}: {}",
                self.outcome, self.case_id, self.case_name, self.message
            )
        }
    }
}
