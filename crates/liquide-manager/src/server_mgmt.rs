//! Server management operations.

use serde::{Deserialize, Serialize};

/// Health status of a managed server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    Online,
    Unhealthy,
    Offline,
    Draining,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Online => write!(f, "online"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Offline => write!(f, "offline"),
            Self::Draining => write!(f, "draining"),
        }
    }
}

/// Summary view of a managed server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSummary {
    pub name: String,
    pub address: String,
    pub status: ServerStatus,
    pub active_sessions: u32,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub uptime_seconds: u64,
}

/// Detailed server information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDetail {
    pub summary: ServerSummary,
    pub version: String,
    pub encoder_capabilities: Vec<String>,
    pub transport_capabilities: Vec<String>,
    pub sessions: Vec<String>,
}

/// Server management registry.
pub struct ServerRegistry {
    servers: Vec<ServerState>,
}

/// Internal tracked state for a server.
#[derive(Debug, Clone)]
struct ServerState {
    name: String,
    address: String,
    status: ServerStatus,
    active_sessions: u32,
    cpu_percent: f32,
    memory_percent: f32,
    uptime_seconds: u64,
    #[allow(dead_code)]
    version: String,
    last_poll_timestamp: u64,
}

impl ServerRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    /// Register a server by name and address.
    pub fn register(&mut self, name: String, address: String) {
        if !self.servers.iter().any(|s| s.name == name) {
            self.servers.push(ServerState {
                name,
                address,
                status: ServerStatus::Offline,
                active_sessions: 0,
                cpu_percent: 0.0,
                memory_percent: 0.0,
                uptime_seconds: 0,
                version: String::new(),
                last_poll_timestamp: 0,
            });
        }
    }

    /// Update metrics for a server.
    pub fn update_metrics(
        &mut self,
        name: &str,
        status: ServerStatus,
        sessions: u32,
        cpu: f32,
        memory: f32,
        uptime: u64,
        timestamp: u64,
    ) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.name == name) {
            s.status = status;
            s.active_sessions = sessions;
            s.cpu_percent = cpu;
            s.memory_percent = memory;
            s.uptime_seconds = uptime;
            s.last_poll_timestamp = timestamp;
        }
    }

    /// Mark a server as offline.
    pub fn mark_offline(&mut self, name: &str) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.name == name) {
            s.status = ServerStatus::Offline;
        }
    }

    /// Mark a server as draining.
    pub fn mark_draining(&mut self, name: &str) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.name == name) {
            s.status = ServerStatus::Draining;
        }
    }

    /// Get summary of all servers.
    #[must_use]
    pub fn list(&self) -> Vec<ServerSummary> {
        self.servers
            .iter()
            .map(|s| ServerSummary {
                name: s.name.clone(),
                address: s.address.clone(),
                status: s.status,
                active_sessions: s.active_sessions,
                cpu_percent: s.cpu_percent,
                memory_percent: s.memory_percent,
                uptime_seconds: s.uptime_seconds,
            })
            .collect()
    }

    /// Get summary for a specific server.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<ServerSummary> {
        self.servers
            .iter()
            .find(|s| s.name == name)
            .map(|s| ServerSummary {
                name: s.name.clone(),
                address: s.address.clone(),
                status: s.status,
                active_sessions: s.active_sessions,
                cpu_percent: s.cpu_percent,
                memory_percent: s.memory_percent,
                uptime_seconds: s.uptime_seconds,
            })
    }

    /// Count of servers by status.
    #[must_use]
    pub fn count_by_status(&self, status: ServerStatus) -> usize {
        self.servers.iter().filter(|s| s.status == status).count()
    }

    /// Total server count.
    #[must_use]
    pub fn count(&self) -> usize {
        self.servers.len()
    }

    /// Total sessions across all online servers.
    #[must_use]
    pub fn total_sessions(&self) -> u32 {
        self.servers.iter().map(|s| s.active_sessions).sum()
    }
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
