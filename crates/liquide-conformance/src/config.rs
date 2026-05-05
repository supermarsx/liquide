//! Configuration for the conformance runner.

use serde::{Deserialize, Serialize};

use crate::suite::SuiteName;

/// How the runner obtains evidence for the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConformanceMode {
    /// Run local protocol validators without contacting the target server.
    OfflineValidation,
    /// Exercise a target server over the protocol.
    LiveServer,
}

impl Default for ConformanceMode {
    fn default() -> Self {
        Self::OfflineValidation
    }
}

impl ConformanceMode {
    /// Human-readable label for reports and logs.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OfflineValidation => "offline protocol validation",
            Self::LiveServer => "live server conformance",
        }
    }
}

/// Conformance runner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceConfig {
    /// Evidence-gathering mode.
    #[serde(default)]
    pub mode: ConformanceMode,
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
            mode: ConformanceMode::OfflineValidation,
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
