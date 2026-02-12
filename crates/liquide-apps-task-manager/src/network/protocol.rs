//! Application-layer protocol analysis types.
//!
//! Models protocol detection results and per-protocol traffic statistics
//! (spec section 14.7).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// AppProtocol
// ---------------------------------------------------------------------------

/// Detected application-layer protocol (spec 14.7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppProtocol {
    Http,
    Https,
    Ftp,
    Ssh,
    Smtp,
    Pop3,
    Imap,
    Dns,
    Dhcp,
    Ntp,
    Snmp,
    Rdp,
    Vnc,
    Mqtt,
    WebSocket,
    Grpc,
    Quic,
    Other,
}

impl AppProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Https => "HTTPS",
            Self::Ftp => "FTP",
            Self::Ssh => "SSH",
            Self::Smtp => "SMTP",
            Self::Pop3 => "POP3",
            Self::Imap => "IMAP",
            Self::Dns => "DNS",
            Self::Dhcp => "DHCP",
            Self::Ntp => "NTP",
            Self::Snmp => "SNMP",
            Self::Rdp => "RDP",
            Self::Vnc => "VNC",
            Self::Mqtt => "MQTT",
            Self::WebSocket => "WebSocket",
            Self::Grpc => "gRPC",
            Self::Quic => "QUIC",
            Self::Other => "Other",
        }
    }
}

impl fmt::Display for AppProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ProtocolStats
// ---------------------------------------------------------------------------

/// Per-protocol traffic statistics (spec 14.7.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStats {
    /// The application-layer protocol.
    pub protocol: AppProtocol,
    /// Number of active connections using this protocol.
    pub connection_count: u32,
    /// Total bytes sent via this protocol.
    pub bytes_sent: u64,
    /// Total bytes received via this protocol.
    pub bytes_received: u64,
    /// Total packets sent via this protocol.
    pub packets_sent: u64,
    /// Total packets received via this protocol.
    pub packets_received: u64,
    /// Average round-trip latency in milliseconds.
    pub avg_latency_ms: Option<f64>,
    /// Protocol-level error rate as a fraction (0.0 to 1.0).
    pub error_rate: f64,
    /// Number of currently active sessions.
    pub active_sessions: u32,
    /// Percentage of total bandwidth consumed by this protocol.
    pub bandwidth_percent: f64,
    /// ISO-8601 timestamp of the most recent activity.
    pub last_activity: Option<String>,
}
