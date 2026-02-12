//! Network topology map types.
//!
//! Models discovered network nodes and the edges (links) between them
//! for the interactive topology visualization (spec section 14.11).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// NodeType
// ---------------------------------------------------------------------------

/// Type of a discovered network node (spec 14.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Router,
    Switch,
    Host,
    Firewall,
    Unknown,
}

impl NodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Router => "Router",
            Self::Switch => "Switch",
            Self::Host => "Host",
            Self::Firewall => "Firewall",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// TopologyNode
// ---------------------------------------------------------------------------

/// A node in the network topology map (spec 14.11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    /// Unique identifier for this node.
    pub id: String,
    /// Human-readable node name.
    pub name: String,
    /// Type of network device.
    pub node_type: NodeType,
    /// IP address of the node, if known.
    pub ip_address: Option<String>,
    /// MAC address of the node, if known.
    pub mac_address: Option<String>,
    /// Whether this node represents the local machine.
    pub is_local: bool,
}

// ---------------------------------------------------------------------------
// TopologyEdge
// ---------------------------------------------------------------------------

/// A link between two nodes in the network topology (spec 14.11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    /// ID of the source node.
    pub source_id: String,
    /// ID of the target node.
    pub target_id: String,
    /// Link bandwidth in bits per second, if known.
    pub bandwidth_bps: Option<u64>,
    /// Link latency in milliseconds, if known.
    pub latency_ms: Option<f64>,
    /// Optional descriptive label for the link.
    pub label: Option<String>,
}
