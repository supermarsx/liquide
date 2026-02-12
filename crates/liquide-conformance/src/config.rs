//! Configuration for the conformance runner.

use serde::{Deserialize, Serialize};

use crate::suite::SuiteName;

/// Conformance runner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceConfig {
    /// Target server address (`host:port`).
    pub server: String,
    /// Which suite(s) to run.
    pub suite: SuiteName,
    /// Optional username for authentication tests.
    pub username: Option<String>,
    /// Optional password for authentication tests.
    pub password: Option<String>,
    /// Per-test timeout in milliseconds.
    pub timeout_ms: u64,
    /// Whether to emit verbose output.
    pub verbose: bool,
    /// Output path for the JSON report.
    pub output: Option<String>,
}

impl Default for ConformanceConfig {
    fn default() -> Self {
        Self {
            server: "localhost:3389".to_string(),
            suite: SuiteName::All,
            username: None,
            password: None,
            timeout_ms: 5000,
            verbose: false,
            output: None,
        }
    }
}
