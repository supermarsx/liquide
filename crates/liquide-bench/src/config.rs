//! Benchmark configuration types.

use serde::{Deserialize, Serialize};

use crate::BenchError;

/// Which benchmark suite(s) to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuiteSelection {
    /// Run all benchmark suites.
    All,
    /// Run only the compositor benchmark suite.
    Compositor,
    /// Run only the encoder benchmark suite.
    Encoder,
    /// Run only the protocol benchmark suite.
    Protocol,
    /// Quick CI smoke test (reduced iterations).
    CiQuick,
    /// Full CI benchmark run.
    CiFull,
}

impl SuiteSelection {
    /// Parse a suite name from a string.
    pub fn from_name(name: &str) -> crate::Result<Self> {
        match name.to_lowercase().as_str() {
            "all" => Ok(Self::All),
            "compositor" => Ok(Self::Compositor),
            "encoder" => Ok(Self::Encoder),
            "protocol" => Ok(Self::Protocol),
            "ci-quick" | "ci_quick" | "ciquick" => Ok(Self::CiQuick),
            "ci-full" | "ci_full" | "cifull" => Ok(Self::CiFull),
            _ => Err(BenchError::UnknownSuite {
                name: name.to_string(),
            }),
        }
    }

    /// Whether this selection includes the compositor suite.
    #[must_use]
    pub fn includes_compositor(&self) -> bool {
        matches!(self, Self::All | Self::Compositor | Self::CiQuick | Self::CiFull)
    }

    /// Whether this selection includes the encoder suite.
    #[must_use]
    pub fn includes_encoder(&self) -> bool {
        matches!(self, Self::All | Self::Encoder | Self::CiQuick | Self::CiFull)
    }

    /// Whether this selection includes the protocol suite.
    #[must_use]
    pub fn includes_protocol(&self) -> bool {
        matches!(self, Self::All | Self::Protocol | Self::CiQuick | Self::CiFull)
    }

    /// Label for display purposes.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Compositor => "compositor",
            Self::Encoder => "encoder",
            Self::Protocol => "protocol",
            Self::CiQuick => "ci-quick",
            Self::CiFull => "ci-full",
        }
    }
}

impl std::fmt::Display for SuiteSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Top-level benchmark configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchConfig {
    /// Which suite(s) to run.
    pub suite: SuiteSelection,
    /// Network profile name (e.g. "lan", "wan-good").
    pub network_profile: String,
    /// Optional path to write the JSON report.
    pub output_path: Option<String>,
    /// Duration in seconds for sustained benchmarks.
    pub duration_secs: u64,
    /// Warmup period in seconds (excluded from measurements).
    pub warmup_secs: u64,
    /// Number of iterations for each micro-benchmark.
    pub iterations: u32,
    /// Enable verbose logging.
    pub verbose: bool,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            suite: SuiteSelection::All,
            network_profile: "lan".to_string(),
            output_path: None,
            duration_secs: 30,
            warmup_secs: 5,
            iterations: 100,
            verbose: false,
        }
    }
}
