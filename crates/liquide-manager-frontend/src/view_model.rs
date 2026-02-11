//! View-model types that mirror backend data for UI rendering.
//!
//! These types are independently defined so the frontend crate does not
//! depend on the backend.  They are intended to be deserialized from JSON
//! responses returned by the manager REST API.

use std::fmt;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

/// Dashboard overview view model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DashboardVM {
    /// Total active sessions across all servers.
    pub total_sessions: u32,
    /// Total connected users.
    pub total_users: u32,
    /// Healthy server count.
    pub servers_healthy: u32,
    /// Unhealthy server count.
    pub servers_unhealthy: u32,
    /// Offline server count.
    pub servers_offline: u32,
    /// Online gateway count.
    pub gateways_online: u32,
    /// Offline gateway count.
    pub gateways_offline: u32,
    /// Aggregate inbound bandwidth (bytes/sec).
    pub bandwidth_in: u64,
    /// Aggregate outbound bandwidth (bytes/sec).
    pub bandwidth_out: u64,
    /// Active alerts.
    pub alerts: Vec<AlertVM>,
}

/// A dashboard alert view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertVM {
    /// Alert severity.
    pub severity: AlertSeverityVM,
    /// Short diagnostic message.
    pub message: String,
    /// Epoch-seconds timestamp.
    pub timestamp: u64,
    /// Related server name, if any.
    pub server: Option<String>,
}

impl AlertVM {
    /// Create a new alert view model.
    #[must_use]
    pub fn new(
        severity: AlertSeverityVM,
        message: impl Into<String>,
        timestamp: u64,
        server: Option<String>,
    ) -> Self {
        Self {
            severity,
            message: message.into(),
            timestamp,
            server,
        }
    }
}

/// Alert severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverityVM {
    Info,
    Warning,
    Critical,
}

impl fmt::Display for AlertSeverityVM {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl AlertSeverityVM {
    /// CSS class name for styling.
    #[must_use]
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Info => "alert-info",
            Self::Warning => "alert-warning",
            Self::Critical => "alert-critical",
        }
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Server status for the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatusVM {
    Online,
    Unhealthy,
    Offline,
    Draining,
}

impl fmt::Display for ServerStatusVM {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Online => write!(f, "online"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Offline => write!(f, "offline"),
            Self::Draining => write!(f, "draining"),
        }
    }
}

impl ServerStatusVM {
    /// CSS class name for styling.
    #[must_use]
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Online => "status-online",
            Self::Unhealthy => "status-unhealthy",
            Self::Offline => "status-offline",
            Self::Draining => "status-draining",
        }
    }
}

/// Server view model for list and detail pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerVM {
    /// Display name.
    pub name: String,
    /// Server API address.
    pub address: String,
    /// Current health status.
    pub status: ServerStatusVM,
    /// Active session count on this server.
    pub active_sessions: u32,
    /// CPU utilization percentage.
    pub cpu_percent: f32,
    /// Memory utilization percentage.
    pub memory_percent: f32,
    /// Server uptime in seconds.
    pub uptime_seconds: u64,
}

impl ServerVM {
    /// Create a new server view model.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        address: impl Into<String>,
        status: ServerStatusVM,
    ) -> Self {
        Self {
            name: name.into(),
            address: address.into(),
            status,
            active_sessions: 0,
            cpu_percent: 0.0,
            memory_percent: 0.0,
            uptime_seconds: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Session status for the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatusVM {
    Active,
    Locked,
    Suspended,
    Disconnecting,
}

impl fmt::Display for SessionStatusVM {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Locked => write!(f, "locked"),
            Self::Suspended => write!(f, "suspended"),
            Self::Disconnecting => write!(f, "disconnecting"),
        }
    }
}

impl SessionStatusVM {
    /// CSS class name for styling.
    #[must_use]
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Active => "session-active",
            Self::Locked => "session-locked",
            Self::Suspended => "session-suspended",
            Self::Disconnecting => "session-disconnecting",
        }
    }
}

/// Session view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionVM {
    /// Unique session identifier.
    pub session_id: String,
    /// Username who owns the session.
    pub user: String,
    /// Server hosting the session.
    pub server: String,
    /// Current session status.
    pub status: SessionStatusVM,
    /// Session duration in seconds.
    pub duration_seconds: u64,
    /// Display resolution (e.g. "1920x1080").
    pub resolution: String,
    /// Encoder name (e.g. "h264", "h265").
    pub encoder: String,
    /// Transport protocol (e.g. "quic", "tcp").
    pub transport: String,
    /// Network latency in milliseconds.
    pub latency_ms: f32,
    /// Frames per second.
    pub fps: f32,
    /// Bandwidth in bytes per second.
    pub bandwidth_bps: u64,
}

impl SessionVM {
    /// Create a new session view model with the minimum required fields.
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        user: impl Into<String>,
        server: impl Into<String>,
        status: SessionStatusVM,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            user: user.into(),
            server: server.into(),
            status,
            duration_seconds: 0,
            resolution: "1920x1080".to_string(),
            encoder: "h264".to_string(),
            transport: "quic".to_string(),
            latency_ms: 0.0,
            fps: 0.0,
            bandwidth_bps: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

/// User view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserVM {
    /// Username.
    pub username: String,
    /// Assigned role.
    pub role: String,
    /// Number of active sessions for this user.
    pub active_sessions: u32,
    /// Last login timestamp (epoch-seconds), or `None` if never logged in.
    pub last_login: Option<u64>,
}

impl UserVM {
    /// Create a new user view model.
    #[must_use]
    pub fn new(username: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            role: role.into(),
            active_sessions: 0,
            last_login: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// Policy view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyVM {
    /// Policy name.
    pub name: String,
    /// Current version number.
    pub version: u64,
    /// Whether the policy is currently active.
    pub active: bool,
    /// Last modification timestamp (epoch-seconds).
    pub modified: u64,
}

impl PolicyVM {
    /// Create a new policy view model.
    #[must_use]
    pub fn new(name: impl Into<String>, version: u64) -> Self {
        Self {
            name: name.into(),
            version,
            active: true,
            modified: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Gateway
// ---------------------------------------------------------------------------

/// Gateway view model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayVM {
    /// Display name.
    pub name: String,
    /// Gateway address.
    pub address: String,
    /// Whether the gateway is online.
    pub online: bool,
    /// Number of servers behind this gateway.
    pub servers_count: u32,
    /// Number of active relay connections.
    pub active_relays: u32,
}

impl GatewayVM {
    /// Create a new gateway view model.
    #[must_use]
    pub fn new(name: impl Into<String>, address: impl Into<String>, online: bool) -> Self {
        Self {
            name: name.into(),
            address: address.into(),
            online,
            servers_count: 0,
            active_relays: 0,
        }
    }
}
