//! Benchmark harness for measuring LiquiDE subsystem performance.
//!
//! Provides workload simulation, measurement collection, SLO validation,
//! and regression detection for the compositor, encoder, and protocol layers.

pub mod compare;
pub mod config;
pub mod harness;
pub mod measurement;
pub mod network;
pub mod report;
pub mod runner;
pub mod slo;
pub mod workload;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the benchmark harness.
#[derive(Debug, Error)]
pub enum BenchError {
    /// An unknown suite name was specified.
    #[error("unknown suite: {name}")]
    UnknownSuite { name: String },

    /// An unknown workload profile was specified.
    #[error("unknown workload: {name}")]
    UnknownWorkload { name: String },

    /// An unknown network profile was specified.
    #[error("unknown network profile: {name}")]
    UnknownNetwork { name: String },

    /// No samples were recorded for the given metric.
    #[error("no samples recorded for metric: {name}")]
    NoSamples { name: String },

    /// The benchmark run failed.
    #[error("benchmark failed: {0}")]
    Failed(String),

    /// An I/O error occurred.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Convenience result type for benchmark operations.
pub type Result<T> = std::result::Result<T, BenchError>;

pub use config::BenchConfig;
pub use runner::BenchRunner;
