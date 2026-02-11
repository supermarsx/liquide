//! Gateway management operations.

use serde::{Deserialize, Serialize};

/// Gateway health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayStatus {
    Online,
    Degraded,
    Offline,
}

impl std::fmt::Display for GatewayStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Online => write!(f, "online"),
            Self::Degraded => write!(f, "degraded"),
            Self::Offline => write!(f, "offline"),
        }
    }
}

/// Gateway summary for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySummary {
    pub name: String,
    pub address: String,
    pub status: GatewayStatus,
    pub connected_servers: u32,
    pub active_relays: u32,
    pub bandwidth_bps: u64,
}

/// Gateway state tracker.
pub struct GatewayRegistry {
    gateways: Vec<GatewayState>,
}

#[derive(Debug, Clone)]
struct GatewayState {
    name: String,
    address: String,
    status: GatewayStatus,
    connected_servers: u32,
    active_relays: u32,
    bandwidth_bps: u64,
}

impl GatewayRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            gateways: Vec::new(),
        }
    }

    /// Register a gateway.
    pub fn register(&mut self, name: String, address: String) {
        if !self.gateways.iter().any(|g| g.name == name) {
            self.gateways.push(GatewayState {
                name,
                address,
                status: GatewayStatus::Offline,
                connected_servers: 0,
                active_relays: 0,
                bandwidth_bps: 0,
            });
        }
    }

    /// Update gateway metrics.
    pub fn update(
        &mut self,
        name: &str,
        status: GatewayStatus,
        servers: u32,
        relays: u32,
        bandwidth: u64,
    ) {
        if let Some(g) = self.gateways.iter_mut().find(|g| g.name == name) {
            g.status = status;
            g.connected_servers = servers;
            g.active_relays = relays;
            g.bandwidth_bps = bandwidth;
        }
    }

    /// Mark a gateway offline.
    pub fn mark_offline(&mut self, name: &str) {
        if let Some(g) = self.gateways.iter_mut().find(|g| g.name == name) {
            g.status = GatewayStatus::Offline;
        }
    }

    /// List all gateways.
    #[must_use]
    pub fn list(&self) -> Vec<GatewaySummary> {
        self.gateways
            .iter()
            .map(|g| GatewaySummary {
                name: g.name.clone(),
                address: g.address.clone(),
                status: g.status,
                connected_servers: g.connected_servers,
                active_relays: g.active_relays,
                bandwidth_bps: g.bandwidth_bps,
            })
            .collect()
    }

    /// Get a single gateway.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<GatewaySummary> {
        self.gateways
            .iter()
            .find(|g| g.name == name)
            .map(|g| GatewaySummary {
                name: g.name.clone(),
                address: g.address.clone(),
                status: g.status,
                connected_servers: g.connected_servers,
                active_relays: g.active_relays,
                bandwidth_bps: g.bandwidth_bps,
            })
    }

    /// Total gateway count.
    #[must_use]
    pub fn count(&self) -> usize {
        self.gateways.len()
    }

    /// Count of online gateways.
    #[must_use]
    pub fn online_count(&self) -> usize {
        self.gateways
            .iter()
            .filter(|g| g.status == GatewayStatus::Online)
            .count()
    }
}

impl Default for GatewayRegistry {
    fn default() -> Self {
        Self::new()
    }
}
