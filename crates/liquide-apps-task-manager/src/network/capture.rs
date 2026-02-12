//! Lightweight packet capture types.
//!
//! Provides configuration and session tracking for the built-in
//! packet capture facility (spec section 14.12).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// CaptureFormat
// ---------------------------------------------------------------------------

/// Output file format for packet captures (spec 14.12 – Save Format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureFormat {
    Pcap,
    PcapNg,
}

impl CaptureFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pcap => "PCAP",
            Self::PcapNg => "PCAPNG",
        }
    }
}

impl fmt::Display for CaptureFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// CaptureConfig
// ---------------------------------------------------------------------------

/// Configuration for a packet capture session (spec 14.12).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Network interface to capture on.
    pub interface: String,
    /// BPF-style filter expression.
    pub filter_expression: Option<String>,
    /// Maximum number of packets to capture.
    pub max_packets: Option<u64>,
    /// Maximum capture file size in bytes.
    pub max_size_bytes: Option<u64>,
    /// Maximum capture duration in seconds.
    pub duration_secs: Option<u64>,
    /// Output file format.
    pub format: CaptureFormat,
    /// Whether to enable promiscuous mode on the interface.
    pub promiscuous: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            interface: String::new(),
            filter_expression: None,
            max_packets: None,
            max_size_bytes: None,
            duration_secs: None,
            format: CaptureFormat::Pcap,
            promiscuous: false,
        }
    }
}

// ---------------------------------------------------------------------------
// CaptureSession
// ---------------------------------------------------------------------------

/// An active or completed packet capture session (spec 14.12).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSession {
    /// Unique session identifier.
    pub id: String,
    /// Configuration used for this capture.
    pub config: CaptureConfig,
    /// ISO-8601 timestamp when the capture started.
    pub start_time: String,
    /// Number of packets captured so far.
    pub packets_captured: u64,
    /// Total bytes captured so far.
    pub bytes_captured: u64,
    /// Whether the capture is currently running.
    pub running: bool,
    /// Path to the output capture file.
    pub output_path: Option<String>,
}
