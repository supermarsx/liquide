//! Network health and diagnostics types.
//!
//! Defines diagnostic tests (ping, traceroute, speed test, etc.) and
//! their result types (spec section 14.13).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// NetworkTest
// ---------------------------------------------------------------------------

/// A network diagnostic test to execute (spec 14.13).
///
/// This enum is **not** `Copy` because several variants carry heap-allocated
/// data (`String`, `Vec<u16>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkTest {
    /// ICMP echo to the given host.
    Ping(String),
    /// Traceroute to the given host.
    TracerouteTarget(String),
    /// DNS lookup for the given domain.
    DnsLookup(String),
    /// TCP port scan against a host and set of ports.
    PortScan { host: String, ports: Vec<u16> },
    /// Internet speed test against a well-known server.
    SpeedTest,
    /// Bandwidth test to the given target.
    BandwidthTest(String),
    /// Latency measurement to the given target.
    LatencyTest(String),
    /// Packet-loss measurement to the given target.
    PacketLoss(String),
    /// Path MTU discovery to the given target.
    MtuDiscovery(String),
    /// Scan nearby Wi-Fi networks and channels.
    WifiSurvey,
    /// ARP scan of the local network segment.
    ArpScan,
    /// Overall connection quality assessment to the given target.
    ConnectionQuality(String),
}

impl NetworkTest {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ping(_) => "Ping",
            Self::TracerouteTarget(_) => "Traceroute",
            Self::DnsLookup(_) => "DNS Lookup",
            Self::PortScan { .. } => "Port Scan",
            Self::SpeedTest => "Speed Test",
            Self::BandwidthTest(_) => "Bandwidth Test",
            Self::LatencyTest(_) => "Latency Test",
            Self::PacketLoss(_) => "Packet Loss",
            Self::MtuDiscovery(_) => "MTU Discovery",
            Self::WifiSurvey => "Wi-Fi Survey",
            Self::ArpScan => "ARP Scan",
            Self::ConnectionQuality(_) => "Connection Quality",
        }
    }
}

impl fmt::Display for NetworkTest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// TracerouteHop
// ---------------------------------------------------------------------------

/// A single hop in a traceroute result (spec 14.13 – Traceroute).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracerouteHop {
    /// Hop number (1-based TTL).
    pub hop_number: u8,
    /// IP address of the responding router, if any.
    pub address: Option<String>,
    /// Reverse DNS hostname of the responding router.
    pub hostname: Option<String>,
    /// Round-trip times for each probe (typically 3).
    pub rtt_ms: Vec<f64>,
    /// Percentage of probes that received no response.
    pub loss_percent: f64,
}

// ---------------------------------------------------------------------------
// SpeedTestResult
// ---------------------------------------------------------------------------

/// Result of an internet speed test (spec 14.13 – Speed Test).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedTestResult {
    /// Download speed in megabits per second.
    pub download_mbps: f64,
    /// Upload speed in megabits per second.
    pub upload_mbps: f64,
    /// Latency to the test server in milliseconds.
    pub latency_ms: f64,
    /// Jitter (latency variance) in milliseconds.
    pub jitter_ms: f64,
    /// Name or address of the speed-test server used.
    pub server: String,
    /// ISO-8601 timestamp when the test completed.
    pub timestamp: String,
}
