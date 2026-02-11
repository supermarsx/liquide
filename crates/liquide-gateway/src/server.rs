//! Backend server registry, health, load, and capabilities.

use std::collections::HashMap;
use std::fmt;

use crate::config::ListenTransport;

/// Health status of a registered server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerHealth {
    /// Server is fully operational.
    Healthy,
    /// Server is operational but experiencing degraded performance.
    Degraded,
    /// Server is not accepting new connections.
    Unhealthy,
    /// Health status has not been determined yet.
    Unknown,
}

impl fmt::Display for ServerHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Advertised capabilities of a backend server.
#[derive(Debug, Clone)]
pub struct ServerCapabilities {
    /// Maximum sessions this server can handle.
    pub max_sessions: u32,
    /// Transport protocols the server supports.
    pub supported_transports: Vec<ListenTransport>,
    /// Hardware encoders available on this server.
    pub supported_encoders: Vec<String>,
    /// Whether the server has a GPU available.
    pub gpu_available: bool,
    /// Arbitrary key-value tags for tag-based routing.
    pub tags: HashMap<String, String>,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            max_sessions: 50,
            supported_transports: vec![ListenTransport::TlsTcp],
            supported_encoders: Vec::new(),
            gpu_available: false,
            tags: HashMap::new(),
        }
    }
}

/// Current load metrics reported by a server.
#[derive(Debug, Clone)]
pub struct ServerLoad {
    /// Number of active desktop sessions.
    pub active_sessions: u32,
    /// CPU usage percentage (0-100).
    pub cpu_percent: f32,
    /// Memory usage percentage (0-100).
    pub memory_percent: f32,
    /// Network bandwidth utilisation percentage (0-100).
    pub bandwidth_percent: f32,
}

impl Default for ServerLoad {
    fn default() -> Self {
        Self {
            active_sessions: 0,
            cpu_percent: 0.0,
            memory_percent: 0.0,
            bandwidth_percent: 0.0,
        }
    }
}

impl ServerLoad {
    /// Compute a composite load score in the range 0.0 ..= 1.0.
    ///
    /// Weights: sessions capacity 40 %, CPU 30 %, memory 20 %, bandwidth 10 %.
    #[must_use]
    pub fn score(&self, capacity: u32) -> f32 {
        let session_ratio = if capacity > 0 {
            self.active_sessions as f32 / capacity as f32
        } else {
            1.0
        };
        0.4 * session_ratio
            + 0.3 * (self.cpu_percent / 100.0)
            + 0.2 * (self.memory_percent / 100.0)
            + 0.1 * (self.bandwidth_percent / 100.0)
    }
}

/// A backend server registered with the gateway.
pub struct RegisteredServer {
    server_id: String,
    address: String,
    capabilities: ServerCapabilities,
    load: ServerLoad,
    health: ServerHealth,
    registered_at: u64,
    last_heartbeat: u64,
    keepalive_active: bool,
}

impl RegisteredServer {
    /// Create a new registered server entry.
    #[must_use]
    pub fn new(
        server_id: String,
        address: String,
        capabilities: ServerCapabilities,
        registered_at: u64,
    ) -> Self {
        Self {
            server_id,
            address,
            capabilities,
            load: ServerLoad::default(),
            health: ServerHealth::Unknown,
            registered_at,
            last_heartbeat: registered_at,
            keepalive_active: true,
        }
    }

    /// Server identifier.
    #[must_use]
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Network address.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Server capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// Current load metrics.
    #[must_use]
    pub fn load(&self) -> &ServerLoad {
        &self.load
    }

    /// Current health status.
    #[must_use]
    pub fn health(&self) -> ServerHealth {
        self.health
    }

    /// Epoch timestamp at which the server was registered.
    #[must_use]
    pub fn registered_at(&self) -> u64 {
        self.registered_at
    }

    /// Epoch timestamp of the last heartbeat.
    #[must_use]
    pub fn last_heartbeat(&self) -> u64 {
        self.last_heartbeat
    }

    /// Whether the keep-alive channel is active.
    #[must_use]
    pub fn keepalive_active(&self) -> bool {
        self.keepalive_active
    }

    /// Update load metrics from a heartbeat.
    pub fn update_load(&mut self, load: ServerLoad) {
        self.load = load;
    }

    /// Update the health status.
    pub fn update_health(&mut self, health: ServerHealth) {
        self.health = health;
    }

    /// Record a heartbeat at the given epoch timestamp.
    pub fn record_heartbeat(&mut self, timestamp: u64) {
        self.last_heartbeat = timestamp;
        self.keepalive_active = true;
    }
}

/// Registry of all backend servers known to this gateway.
pub struct ServerRegistry {
    servers: HashMap<String, RegisteredServer>,
    next_id: u64,
}

impl ServerRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a new server. Returns the assigned server ID.
    pub fn register(
        &mut self,
        address: String,
        capabilities: ServerCapabilities,
        timestamp: u64,
    ) -> String {
        let server_id = format!("srv-{}", self.next_id);
        self.next_id += 1;

        let server = RegisteredServer::new(
            server_id.clone(),
            address,
            capabilities,
            timestamp,
        );
        self.servers.insert(server_id.clone(), server);
        server_id
    }

    /// Remove a server from the registry.
    pub fn deregister(&mut self, server_id: &str) -> Option<RegisteredServer> {
        self.servers.remove(server_id)
    }

    /// Get a reference to a server by ID.
    #[must_use]
    pub fn get(&self, server_id: &str) -> Option<&RegisteredServer> {
        self.servers.get(server_id)
    }

    /// Get a mutable reference to a server by ID.
    pub fn get_mut(&mut self, server_id: &str) -> Option<&mut RegisteredServer> {
        self.servers.get_mut(server_id)
    }

    /// Return a list of server IDs that are considered healthy (Healthy or Degraded).
    #[must_use]
    pub fn healthy_servers(&self) -> Vec<String> {
        self.servers
            .values()
            .filter(|s| matches!(s.health, ServerHealth::Healthy | ServerHealth::Degraded))
            .map(|s| s.server_id.clone())
            .collect()
    }

    /// Update the health status of a specific server.
    pub fn update_health(&mut self, server_id: &str, health: ServerHealth) {
        if let Some(server) = self.servers.get_mut(server_id) {
            server.update_health(health);
        }
    }

    /// Iterate over all registered servers.
    #[must_use]
    pub fn all_servers(&self) -> &HashMap<String, RegisteredServer> {
        &self.servers
    }

    /// Number of registered servers.
    #[must_use]
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
