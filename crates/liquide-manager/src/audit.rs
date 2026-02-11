//! Audit event types for management actions.

use serde::{Deserialize, Serialize};

/// Audit severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLevel {
    Info,
    Warning,
    Critical,
}

/// Management audit events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ManagerAuditEvent {
    AdminLogin {
        username: String,
        ip: String,
    },
    AdminLogout {
        username: String,
    },
    LoginFailed {
        username: String,
        ip: String,
        reason: String,
    },
    SessionDisconnected {
        session_id: String,
        admin: String,
    },
    SessionLocked {
        session_id: String,
        admin: String,
    },
    SessionUnlocked {
        session_id: String,
        admin: String,
    },
    PolicyUpdated {
        admin: String,
        version: u64,
    },
    PolicyRolledBack {
        admin: String,
        from_version: u64,
        to_version: u64,
    },
    ServerConfigPushed {
        server: String,
        admin: String,
    },
    ServerRestarted {
        server: String,
        admin: String,
    },
    ServerDrained {
        server: String,
        admin: String,
    },
    PluginInstalled {
        plugin_id: String,
        admin: String,
    },
    PluginRemoved {
        plugin_id: String,
        admin: String,
    },
    UserRoleChanged {
        username: String,
        old_role: String,
        new_role: String,
        admin: String,
    },
    CrashReportViewed {
        crash_id: String,
        admin: String,
    },
}

impl ManagerAuditEvent {
    /// Severity level for this event.
    #[must_use]
    pub fn level(&self) -> AuditLevel {
        match self {
            Self::LoginFailed { .. } => AuditLevel::Warning,
            Self::PolicyRolledBack { .. }
            | Self::ServerRestarted { .. }
            | Self::ServerDrained { .. } => AuditLevel::Warning,
            Self::AdminLogin { .. }
            | Self::AdminLogout { .. }
            | Self::SessionDisconnected { .. }
            | Self::SessionLocked { .. }
            | Self::SessionUnlocked { .. }
            | Self::PolicyUpdated { .. }
            | Self::ServerConfigPushed { .. }
            | Self::PluginInstalled { .. }
            | Self::PluginRemoved { .. }
            | Self::UserRoleChanged { .. }
            | Self::CrashReportViewed { .. } => AuditLevel::Info,
        }
    }

    /// Machine-readable event name.
    #[must_use]
    pub fn event_name(&self) -> &str {
        match self {
            Self::AdminLogin { .. } => "admin_login",
            Self::AdminLogout { .. } => "admin_logout",
            Self::LoginFailed { .. } => "login_failed",
            Self::SessionDisconnected { .. } => "session_disconnected",
            Self::SessionLocked { .. } => "session_locked",
            Self::SessionUnlocked { .. } => "session_unlocked",
            Self::PolicyUpdated { .. } => "policy_updated",
            Self::PolicyRolledBack { .. } => "policy_rolled_back",
            Self::ServerConfigPushed { .. } => "server_config_pushed",
            Self::ServerRestarted { .. } => "server_restarted",
            Self::ServerDrained { .. } => "server_drained",
            Self::PluginInstalled { .. } => "plugin_installed",
            Self::PluginRemoved { .. } => "plugin_removed",
            Self::UserRoleChanged { .. } => "user_role_changed",
            Self::CrashReportViewed { .. } => "crash_report_viewed",
        }
    }
}
