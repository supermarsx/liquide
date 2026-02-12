//! Network traffic monitoring and analysis types.
//!
//! Covers connections, DNS queries, protocol analysis, firewall rules,
//! bandwidth monitoring, interface details, topology mapping, packet
//! capture, and network diagnostics (spec section 14).

use serde::{Deserialize, Serialize};
use std::fmt;

pub mod bandwidth;
pub mod capture;
pub mod connection;
pub mod diagnostics;
pub mod dns;
pub mod firewall;
pub mod interface;
pub mod protocol;
pub mod topology;

// ---------------------------------------------------------------------------
// NetworkView
// ---------------------------------------------------------------------------

/// Sidebar navigation views within the Network Traffic tab (spec 14.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkView {
    Connections,
    DnsQueries,
    Protocols,
    Firewall,
    Bandwidth,
    Interfaces,
    Topology,
    Capture,
    Diagnostics,
    Overview,
}

impl NetworkView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connections => "Connections",
            Self::DnsQueries => "DNS Queries",
            Self::Protocols => "Protocols",
            Self::Firewall => "Firewall",
            Self::Bandwidth => "Bandwidth",
            Self::Interfaces => "Interfaces",
            Self::Topology => "Topology",
            Self::Capture => "Capture",
            Self::Diagnostics => "Diagnostics",
            Self::Overview => "Overview",
        }
    }
}

impl fmt::Display for NetworkView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// NetworkOverview
// ---------------------------------------------------------------------------

/// Aggregate network activity summary for the Overview dashboard (spec 14.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkOverview {
    /// Number of currently open connections.
    pub active_connections: u32,
    /// Combined inbound throughput in bits per second.
    pub total_bandwidth_in_bps: u64,
    /// Combined outbound throughput in bits per second.
    pub total_bandwidth_out_bps: u64,
    /// DNS queries resolved per second.
    pub dns_queries_per_sec: u32,
    /// Connections blocked by firewall since boot.
    pub blocked_connections: u32,
    /// Number of network interfaces present.
    pub interface_count: u32,
    /// Whether a VPN tunnel is currently active.
    pub vpn_active: bool,
}

// ---------------------------------------------------------------------------
// VpnInfo
// ---------------------------------------------------------------------------

/// Information about an active VPN tunnel (spec 14.3 – VPN Status widget).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnInfo {
    /// Human-readable VPN connection name.
    pub name: String,
    /// Tunnelling protocol (e.g. WireGuard, OpenVPN, IKEv2).
    pub protocol: String,
    /// Remote VPN server address.
    pub server_address: String,
    /// Whether the tunnel is currently connected.
    pub connected: bool,
    /// Total bytes sent through the tunnel.
    pub bytes_sent: u64,
    /// Total bytes received through the tunnel.
    pub bytes_received: u64,
    /// ISO-8601 timestamp when the tunnel was established.
    pub connected_since: Option<String>,
}
