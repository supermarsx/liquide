//! Management API endpoint definitions.

use crate::config::ManagementApiConfig;

/// Constant-time byte comparison to prevent timing side-channel attacks.
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

/// Known management API endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiEndpoint {
    /// General gateway status.
    Status,
    /// List all registered servers.
    ListServers,
    /// Get details for a single server.
    GetServer,
    /// Remove a server from the registry.
    DeleteServer,
    /// List all client sessions.
    ListSessions,
    /// Get details for a single session.
    GetSession,
    /// Forcibly terminate a session.
    DeleteSession,
    /// List authenticated users.
    ListUsers,
    /// Prometheus-compatible metrics.
    Metrics,
    /// Get current configuration.
    GetConfig,
    /// Hot-reload configuration.
    UpdateConfig,
}

impl ApiEndpoint {
    /// URL path for this endpoint.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Status => "/api/v1/status",
            Self::ListServers => "/api/v1/servers",
            Self::GetServer => "/api/v1/servers/:id",
            Self::DeleteServer => "/api/v1/servers/:id",
            Self::ListSessions => "/api/v1/sessions",
            Self::GetSession => "/api/v1/sessions/:id",
            Self::DeleteSession => "/api/v1/sessions/:id",
            Self::ListUsers => "/api/v1/users",
            Self::Metrics => "/api/v1/metrics",
            Self::GetConfig => "/api/v1/config",
            Self::UpdateConfig => "/api/v1/config",
        }
    }

    /// HTTP method for this endpoint.
    #[must_use]
    pub fn method(&self) -> &str {
        match self {
            Self::Status
            | Self::ListServers
            | Self::GetServer
            | Self::ListSessions
            | Self::GetSession
            | Self::ListUsers
            | Self::Metrics
            | Self::GetConfig => "GET",
            Self::DeleteServer | Self::DeleteSession => "DELETE",
            Self::UpdateConfig => "PUT",
        }
    }
}

/// Management API handler.
pub struct ManagementApi {
    config: ManagementApiConfig,
}

impl ManagementApi {
    /// Create a new management API handler.
    #[must_use]
    pub fn new(config: ManagementApiConfig) -> Self {
        Self { config }
    }

    /// Whether the management API is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Validate an API key against the configured key.
    #[must_use]
    pub fn validate_api_key(&self, key: &str) -> bool {
        if self.config.api_key.is_empty() {
            return false;
        }
        constant_time_eq(key.as_bytes(), self.config.api_key.as_bytes())
    }

    /// Handle a management API request.
    ///
    /// In production this would dispatch to specific handlers and return
    /// serialized JSON responses. The stub returns a status string.
    #[must_use]
    pub fn handle_request(&self, endpoint: ApiEndpoint, api_key: &str) -> String {
        if !self.is_enabled() {
            return "management API disabled".to_string();
        }
        if !self.validate_api_key(api_key) {
            return "unauthorized".to_string();
        }
        format!("{} {} -> ok", endpoint.method(), endpoint.path())
    }

    /// The configured listen address.
    #[must_use]
    pub fn listen_addr(&self) -> &str {
        &self.config.listen_addr
    }
}
