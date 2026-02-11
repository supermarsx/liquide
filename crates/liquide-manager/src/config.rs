//! Manager configuration types.

use serde::{Deserialize, Serialize};

/// Top-level manager configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerConfig {
    /// Listen address for the HTTP API.
    pub listen_addr: String,
    /// TLS configuration.
    pub tls: TlsConfig,
    /// Authentication configuration.
    pub auth: AuthConfig,
    /// Managed servers.
    pub servers: Vec<ServerEntry>,
    /// Managed gateways.
    pub gateways: Vec<GatewayEntry>,
    /// Metrics configuration.
    pub metrics: MetricsConfig,
    /// UI settings.
    pub ui: UiConfig,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:8443".to_string(),
            tls: TlsConfig::default(),
            auth: AuthConfig::default(),
            servers: Vec::new(),
            gateways: Vec::new(),
            metrics: MetricsConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

/// TLS settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Whether TLS is enabled.
    pub enabled: bool,
    /// Path to TLS certificate.
    pub cert_path: String,
    /// Path to TLS private key.
    pub key_path: String,
    /// Auto-generate a self-signed cert on first run.
    pub auto_generate_self_signed: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cert_path: "/etc/liquid-manager/cert.pem".to_string(),
            key_path: "/etc/liquid-manager/key.pem".to_string(),
            auto_generate_self_signed: true,
        }
    }
}

/// Authentication settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Authentication mode.
    pub mode: AuthMode,
    /// Session timeout in minutes.
    pub session_timeout_min: u32,
    /// Maximum login attempts before lockout.
    pub max_login_attempts: u32,
    /// Lockout duration in minutes.
    pub lockout_duration_min: u32,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            mode: AuthMode::Local,
            session_timeout_min: 30,
            max_login_attempts: 5,
            lockout_duration_min: 15,
        }
    }
}

/// Authentication mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    /// Local accounts stored in configuration.
    Local,
    /// OpenID Connect provider.
    Oidc,
    /// Mutual TLS client certificates.
    Mtls,
}

impl std::fmt::Display for AuthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Oidc => write!(f, "oidc"),
            Self::Mtls => write!(f, "mtls"),
        }
    }
}

/// An entry for a managed server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    /// Display name.
    pub name: String,
    /// Server API address.
    pub address: String,
    /// API key for authentication.
    pub api_key: String,
}

/// An entry for a managed gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayEntry {
    /// Display name.
    pub name: String,
    /// Gateway address.
    pub address: String,
    /// API key for authentication.
    pub api_key: String,
}

/// Metrics collection settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// How many hours to keep in-memory metrics.
    pub retention_hours: u32,
    /// How often to poll servers (seconds).
    pub polling_interval_sec: u32,
    /// Optional external TSDB URL for Prometheus remote-write.
    pub external_tsdb_url: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            retention_hours: 24,
            polling_interval_sec: 5,
            external_tsdb_url: String::new(),
        }
    }
}

/// UI display settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Theme name.
    pub theme: String,
    /// Items per page in list views.
    pub items_per_page: u32,
    /// Auto-refresh interval in seconds.
    pub auto_refresh_sec: u32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "liquid-glass".to_string(),
            items_per_page: 25,
            auto_refresh_sec: 5,
        }
    }
}

/// Admin role controlling access levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AdminRole {
    /// Read-only access.
    Viewer,
    /// Viewer + session control.
    Operator,
    /// Operator + policy/config editing.
    Admin,
    /// Full access including user management.
    SuperAdmin,
}

impl std::fmt::Display for AdminRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Viewer => write!(f, "viewer"),
            Self::Operator => write!(f, "operator"),
            Self::Admin => write!(f, "admin"),
            Self::SuperAdmin => write!(f, "super-admin"),
        }
    }
}

impl AdminRole {
    /// Whether this role has at least the given level.
    #[must_use]
    pub fn has_permission(&self, required: AdminRole) -> bool {
        *self >= required
    }
}
