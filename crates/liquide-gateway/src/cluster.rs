//! Cluster coordination and shared-state management.

use std::collections::HashMap;

use crate::config::StateStoreType;

/// A node in the gateway cluster.
#[derive(Debug, Clone)]
pub struct ClusterNode {
    /// Unique node identifier.
    pub node_id: String,
    /// Network address of the node.
    pub address: String,
    /// Whether this node is the current leader.
    pub is_leader: bool,
    /// Epoch timestamp of the last heartbeat from this node.
    pub last_seen: u64,
}

/// Shared cluster state. Coordinates session routing across gateway nodes.
pub struct ClusterState {
    nodes: Vec<ClusterNode>,
    local_node_id: String,
    store_type: StateStoreType,
    session_routing: HashMap<String, String>,
}

impl ClusterState {
    /// Create a new cluster state for the local node.
    #[must_use]
    pub fn new(local_node_id: String, store_type: StateStoreType) -> Self {
        Self {
            nodes: Vec::new(),
            local_node_id,
            store_type,
            session_routing: HashMap::new(),
        }
    }

    /// Add a peer node to the cluster.
    pub fn add_node(&mut self, node: ClusterNode) {
        // Replace existing node with same ID, or insert new.
        if let Some(existing) = self.nodes.iter_mut().find(|n| n.node_id == node.node_id) {
            *existing = node;
        } else {
            self.nodes.push(node);
        }
    }

    /// Remove a peer node.
    pub fn remove_node(&mut self, node_id: &str) {
        self.nodes.retain(|n| n.node_id != node_id);
    }

    /// Get the current leader node, if one is elected.
    #[must_use]
    pub fn leader(&self) -> Option<&ClusterNode> {
        self.nodes.iter().find(|n| n.is_leader)
    }

    /// Whether the cluster is healthy (at least one leader is present
    /// or the store is stateless).
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        if self.store_type == StateStoreType::Stateless {
            return true;
        }
        self.leader().is_some()
    }

    /// Look up which gateway node owns a session.
    #[must_use]
    pub fn lookup_session_server(&self, session_id: &str) -> Option<&str> {
        self.session_routing.get(session_id).map(|s| s.as_str())
    }

    /// Register a session -> server mapping in shared state.
    pub fn register_session_route(&mut self, session_id: String, server_id: String) {
        self.session_routing.insert(session_id, server_id);
    }

    /// Local node identifier.
    #[must_use]
    pub fn local_node_id(&self) -> &str {
        &self.local_node_id
    }

    /// State store type.
    #[must_use]
    pub fn store_type(&self) -> StateStoreType {
        self.store_type
    }

    /// All known nodes.
    #[must_use]
    pub fn nodes(&self) -> &[ClusterNode] {
        &self.nodes
    }
}
