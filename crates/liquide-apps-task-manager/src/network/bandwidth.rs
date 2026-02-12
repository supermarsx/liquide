//! Bandwidth monitoring and traffic shaping types.
//!
//! Covers per-interface bandwidth metrics, QoS priority rules,
//! and data usage quotas (spec section 14.9).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// QosPriority
// ---------------------------------------------------------------------------

/// Traffic shaping priority class (spec 14.9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QosPriority {
    Critical,
    High,
    Medium,
    Low,
    Background,
}

impl QosPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::Background => "Background",
        }
    }
}

impl fmt::Display for QosPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// InterfaceBandwidth
// ---------------------------------------------------------------------------

/// Per-interface bandwidth metrics (spec 14.9.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceBandwidth {
    /// Network adapter name.
    pub interface_name: String,
    /// Negotiated link capacity in bits per second.
    pub capacity_bps: u64,
    /// Current inbound throughput in bits per second.
    pub current_in_bps: u64,
    /// Current outbound throughput in bits per second.
    pub current_out_bps: u64,
    /// Peak inbound throughput in bits per second (session).
    pub peak_in_bps: u64,
    /// Peak outbound throughput in bits per second (session).
    pub peak_out_bps: u64,
    /// Current utilization as a percentage of capacity.
    pub utilization_percent: f64,
}

// ---------------------------------------------------------------------------
// QosRule
// ---------------------------------------------------------------------------

/// A QoS traffic-shaping rule (spec 14.9.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QosRule {
    /// Human-readable rule name.
    pub name: String,
    /// Priority class assigned to matching traffic.
    pub priority: QosPriority,
    /// Criteria expression for matching traffic (e.g. process name, port).
    pub match_criteria: String,
    /// Optional maximum bandwidth in bits per second.
    pub bandwidth_limit_bps: Option<u64>,
    /// Whether the rule is currently active.
    pub active: bool,
}

// ---------------------------------------------------------------------------
// BandwidthQuota
// ---------------------------------------------------------------------------

/// Data usage quota configuration (spec 14.9.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthQuota {
    /// Human-readable quota name.
    pub name: String,
    /// Total allowed bytes for the quota period.
    pub quota_bytes: u64,
    /// Bytes consumed so far in the current period.
    pub used_bytes: u64,
    /// Quota period (e.g. "daily", "weekly", "monthly").
    pub period: String,
    /// Optional interface this quota is scoped to.
    pub interface: Option<String>,
}
