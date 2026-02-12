//! Firewall rule viewer types.
//!
//! Models system firewall rules including direction, action, profile,
//! and hit-count tracking (spec section 14.8).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// FirewallDirection
// ---------------------------------------------------------------------------

/// Traffic direction a firewall rule applies to (spec 14.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallDirection {
    Inbound,
    Outbound,
}

impl FirewallDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inbound => "Inbound",
            Self::Outbound => "Outbound",
        }
    }
}

impl fmt::Display for FirewallDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// FirewallAction
// ---------------------------------------------------------------------------

/// Action taken when a firewall rule matches (spec 14.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallAction {
    Allow,
    Block,
    Log,
}

impl FirewallAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "Allow",
            Self::Block => "Block",
            Self::Log => "Log",
        }
    }
}

impl fmt::Display for FirewallAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// FirewallProfile
// ---------------------------------------------------------------------------

/// Network profile a firewall rule is associated with (spec 14.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallProfile {
    Domain,
    Private,
    Public,
}

impl FirewallProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Domain => "Domain",
            Self::Private => "Private",
            Self::Public => "Public",
        }
    }
}

impl fmt::Display for FirewallProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// FirewallRule
// ---------------------------------------------------------------------------

/// A single firewall rule with metadata and usage statistics (spec 14.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    /// Human-readable rule name.
    pub name: String,
    /// Whether the rule is currently enabled.
    pub enabled: bool,
    /// Traffic direction the rule applies to.
    pub direction: FirewallDirection,
    /// Action taken when the rule matches.
    pub action: FirewallAction,
    /// Matching protocol (e.g. "TCP", "UDP", "ICMP", "Any").
    pub protocol: Option<String>,
    /// Local port or port range.
    pub local_port: Option<String>,
    /// Remote port or port range.
    pub remote_port: Option<String>,
    /// Local IP address or subnet.
    pub local_address: Option<String>,
    /// Remote IP address or subnet.
    pub remote_address: Option<String>,
    /// Associated executable path.
    pub program: Option<String>,
    /// Associated Windows service name.
    pub service: Option<String>,
    /// Network profile this rule belongs to.
    pub profile: FirewallProfile,
    /// Human-readable rule description.
    pub description: Option<String>,
    /// Number of times the rule has been triggered since boot.
    pub hit_count: u64,
    /// ISO-8601 timestamp of the last match.
    pub last_hit: Option<String>,
}
