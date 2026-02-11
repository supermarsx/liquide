//! Dashboard data aggregation.

use serde::{Deserialize, Serialize};

/// Aggregate dashboard snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DashboardData {
    /// Total active sessions across all servers.
    pub total_sessions: u32,
    /// Total connected users.
    pub total_users: u32,
    /// Servers by health status.
    pub servers_healthy: u32,
    pub servers_unhealthy: u32,
    pub servers_offline: u32,
    /// Gateway count (if configured).
    pub gateways_online: u32,
    pub gateways_offline: u32,
    /// Aggregate bandwidth (bytes/sec inbound + outbound).
    pub bandwidth_in_bps: u64,
    pub bandwidth_out_bps: u64,
    /// Active alerts.
    pub alerts: Vec<DashboardAlert>,
}

/// A dashboard alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardAlert {
    /// Alert severity.
    pub severity: AlertSeverity,
    /// Short message.
    pub message: String,
    /// Timestamp (epoch seconds).
    pub timestamp: u64,
    /// Related server name, if any.
    pub server: Option<String>,
}

/// Alert severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Dashboard data builder.
pub struct DashboardBuilder {
    data: DashboardData,
}

impl DashboardBuilder {
    /// Create a new empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: DashboardData::default(),
        }
    }

    /// Add server stats.
    pub fn add_server(&mut self, healthy: bool, sessions: u32, users: u32, bw_in: u64, bw_out: u64) {
        if healthy {
            self.data.servers_healthy += 1;
        } else {
            self.data.servers_unhealthy += 1;
        }
        self.data.total_sessions += sessions;
        self.data.total_users += users;
        self.data.bandwidth_in_bps += bw_in;
        self.data.bandwidth_out_bps += bw_out;
    }

    /// Mark a server as offline.
    pub fn add_offline_server(&mut self) {
        self.data.servers_offline += 1;
    }

    /// Add a gateway.
    pub fn add_gateway(&mut self, online: bool) {
        if online {
            self.data.gateways_online += 1;
        } else {
            self.data.gateways_offline += 1;
        }
    }

    /// Push an alert.
    pub fn add_alert(&mut self, severity: AlertSeverity, message: String, timestamp: u64, server: Option<String>) {
        self.data.alerts.push(DashboardAlert {
            severity,
            message,
            timestamp,
            server,
        });
    }

    /// Consume and return the dashboard data.
    #[must_use]
    pub fn build(self) -> DashboardData {
        self.data
    }
}

impl Default for DashboardBuilder {
    fn default() -> Self {
        Self::new()
    }
}
