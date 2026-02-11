//! Connection broker and relay server for the LiquiDE remote desktop protocol.
//!
//! Provides TLS termination, authentication, routing, relay proxying,
//! rate limiting, health checking, and cluster coordination.

pub mod config;
pub mod server;
pub mod routing;
pub mod connection;
pub mod auth;
pub mod relay;
pub mod reverse;
pub mod listener;
pub mod ratelimit;
pub mod health;
pub mod management;
pub mod cluster;
pub mod audit;
pub mod runtime;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the gateway subsystem.
#[derive(Debug, Error)]
pub enum GatewayError {
    /// The requested server was not found in the registry.
    #[error("server not found: {server_id}")]
    ServerNotFound { server_id: String },

    /// The target server is not healthy enough to accept connections.
    #[error("server unhealthy: {server_id}")]
    ServerUnhealthy { server_id: String },

    /// The requested session does not exist.
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },

    /// Client authentication failed.
    #[error("authentication failed ({method}): {reason}")]
    AuthenticationFailed { method: String, reason: String },

    /// Route computation failed.
    #[error("routing failed: {reason}")]
    RoutingFailed { reason: String },

    /// The relay subsystem has reached its session capacity.
    #[error("relay capacity exceeded (max {max_sessions})")]
    RelayCapacityExceeded { max_sessions: u32 },

    /// The client exceeded the per-IP rate limit.
    #[error("rate limit exceeded for {ip} (window {window_seconds}s)")]
    RateLimitExceeded { ip: String, window_seconds: u64 },

    /// The client IP is currently banned.
    #[error("IP banned: {ip} until {until}")]
    IpBanned { ip: String, until: String },

    /// A TLS-related error occurred.
    #[error("TLS error: {detail}")]
    TlsError { detail: String },

    /// A configuration error was detected.
    #[error("config error: {detail}")]
    ConfigError { detail: String },

    /// The listener failed to bind to the requested address.
    #[error("listener bind failed on {addr}: {reason}")]
    ListenerBindFailed { addr: String, reason: String },

    /// A cluster coordination error occurred.
    #[error("cluster error: {detail}")]
    ClusterError { detail: String },

    /// A reverse-connect handshake timed out.
    #[error("reverse connect timeout for server {server_id} after {timeout_sec}s")]
    ReverseConnectTimeout { server_id: String, timeout_sec: u64 },

    /// An internal error not covered by specific variants.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for gateway operations.
pub type Result<T> = std::result::Result<T, GatewayError>;

// Re-exports
pub use config::{
    GatewayConfig, ListenConfig, ListenTransport, ListenRole, TlsConfig,
    RoutingConfig, RelayConfig, ReverseConnectConfig, LimitsConfig,
    HealthCheckConfig, SecurityConfig, ObfuscationMode,
    ManagementApiConfig, ClusterConfig,
};
pub use server::{ServerHealth, ServerCapabilities, ServerLoad, RegisteredServer, ServerRegistry};
pub use routing::{RoutingStrategy, RouteDecision, Router};
pub use connection::{ConnectionMode, ConnectionState, ClientConnection, ConnectionTracker};
pub use auth::{GatewayAuthMethod, AuthResult, AuthChallenge, AuthHandler};
pub use relay::{RelaySession, RelayManager};
pub use reverse::{ReverseConnectionState, ReverseConnection, ReverseConnectionManager};
pub use listener::{ListenerState, TransportListener, ListenerManager};
pub use ratelimit::{RateLimiterEntry, IpBan, TarpitSession, TarpitMode, RateLimiter};
pub use health::{HealthStatus, HealthChecker};
pub use management::{ApiEndpoint, ManagementApi};
pub use config::StateStoreType;
pub use cluster::{ClusterNode, ClusterState};
pub use audit::{AuditLevel, GatewayAuditEvent};
pub use runtime::{GatewayRuntime, GatewayStatus};
