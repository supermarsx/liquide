//! Configuration types for the gateway subsystem.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Top-level gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Human-readable hostname for this gateway instance.
    pub hostname: String,
    /// Directory used for persistent state (certs, logs, etc.).
    pub data_dir: String,
    /// Default log level filter string.
    pub log_level: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            hostname: "gateway-01".to_string(),
            data_dir: "/var/lib/liquide/gateway".to_string(),
            log_level: "info".to_string(),
        }
    }
}

/// Transport protocol for a listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListenTransport {
    /// QUIC (UDP-based, always encrypted).
    Quic,
    /// TLS over TCP.
    TlsTcp,
    /// WebSocket over TLS.
    WebSocketTls,
    /// Plain HTTP (management only, never for client traffic).
    Http,
}

impl fmt::Display for ListenTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quic => write!(f, "QUIC"),
            Self::TlsTcp => write!(f, "TLS/TCP"),
            Self::WebSocketTls => write!(f, "WebSocket/TLS"),
            Self::Http => write!(f, "HTTP"),
        }
    }
}

/// The role a listener serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListenRole {
    /// Accepts remote desktop client connections.
    Client,
    /// Accepts management API requests.
    Management,
}

impl fmt::Display for ListenRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client => write!(f, "client"),
            Self::Management => write!(f, "management"),
        }
    }
}

/// Configuration for a single listener endpoint.
#[derive(Debug, Clone)]
pub struct ListenConfig {
    /// The address and port to bind (e.g. "0.0.0.0:3900").
    pub address: String,
    /// Transport protocol.
    pub transport: ListenTransport,
    /// Role this listener serves.
    pub role: ListenRole,
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:3900".to_string(),
            transport: ListenTransport::TlsTcp,
            role: ListenRole::Client,
        }
    }
}

/// TLS and ACME certificate configuration.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Whether to use ACME (Let's Encrypt) for automatic certificate management.
    pub acme_enabled: bool,
    /// ACME account email.
    pub acme_email: String,
    /// Domain name for ACME certificate requests.
    pub acme_domain: String,
    /// Path to a manually provisioned certificate file.
    pub cert_path: String,
    /// Path to the corresponding private key file.
    pub key_path: String,
    /// Minimum TLS version string (e.g. "1.2", "1.3").
    pub min_version: String,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            acme_enabled: false,
            acme_email: String::new(),
            acme_domain: String::new(),
            cert_path: "/etc/liquide/certs/gateway.crt".to_string(),
            key_path: "/etc/liquide/certs/gateway.key".to_string(),
            min_version: "1.3".to_string(),
        }
    }
}

/// Routing strategy for assigning clients to servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Route to an explicit server ID.
    Direct,
    /// Cycle through healthy servers.
    RoundRobin,
    /// Pick the server with the least load.
    LeastLoad,
    /// Pick the server with the lowest latency to the client.
    LeastLatency,
    /// Route by geographic proximity.
    Geographic,
    /// Route by server tags.
    TagBased,
    /// Bind a client IP to a server for the sticky TTL.
    Sticky,
}

impl fmt::Display for RoutingStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::RoundRobin => write!(f, "round_robin"),
            Self::LeastLoad => write!(f, "least_load"),
            Self::LeastLatency => write!(f, "least_latency"),
            Self::Geographic => write!(f, "geographic"),
            Self::TagBased => write!(f, "tag_based"),
            Self::Sticky => write!(f, "sticky"),
        }
    }
}

/// Routing subsystem configuration.
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Strategy used to assign clients to servers.
    pub strategy: RoutingStrategy,
    /// Whether sticky sessions are enabled.
    pub sticky_sessions: bool,
    /// TTL for sticky bindings in seconds.
    pub sticky_ttl_sec: u64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            strategy: RoutingStrategy::LeastLoad,
            sticky_sessions: false,
            sticky_ttl_sec: 300,
        }
    }
}

/// Relay proxy configuration.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Whether the relay subsystem is enabled.
    pub enabled: bool,
    /// Maximum number of concurrent relay sessions.
    pub max_relay_sessions: u32,
    /// Maximum aggregate bandwidth in Mbps.
    pub max_bandwidth_mbps: u32,
    /// Splice buffer size for data forwarding.
    pub splice_buffer_bytes: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_relay_sessions: 1000,
            max_bandwidth_mbps: 10_000,
            splice_buffer_bytes: 65_536,
        }
    }
}

/// Reverse-connect (server-initiated tunnel) configuration.
#[derive(Debug, Clone)]
pub struct ReverseConnectConfig {
    /// Whether reverse-connect is enabled.
    pub enabled: bool,
    /// Maximum number of servers awaiting a connect-back.
    pub max_pending: u32,
    /// Timeout for a connect-back handshake in seconds.
    pub timeout_sec: u64,
}

impl Default for ReverseConnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_pending: 100,
            timeout_sec: 30,
        }
    }
}

/// Connection and rate limits.
#[derive(Debug, Clone)]
pub struct LimitsConfig {
    /// Maximum concurrent client connections.
    pub max_concurrent_clients: u32,
    /// Maximum concurrent registered servers.
    pub max_concurrent_servers: u32,
    /// Per-IP request rate limit (requests per second).
    pub per_ip_rate_per_sec: u32,
    /// Number of auth failures before an IP is auto-banned.
    pub auth_failure_ban_threshold: u32,
    /// Window in seconds over which auth failures are counted.
    pub auth_failure_window_sec: u64,
    /// Duration of an automatic ban in seconds.
    pub ban_duration_sec: u64,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_concurrent_clients: 10_000,
            max_concurrent_servers: 500,
            per_ip_rate_per_sec: 60,
            auth_failure_ban_threshold: 5,
            auth_failure_window_sec: 300,
            ban_duration_sec: 3600,
        }
    }
}

/// Health-check configuration for backend servers.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Interval between health checks in seconds.
    pub interval_sec: u64,
    /// Number of consecutive failures before marking a server unhealthy.
    pub unhealthy_threshold: u32,
    /// Timeout for a single health-check probe in seconds.
    pub timeout_sec: u64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval_sec: 10,
            unhealthy_threshold: 3,
            timeout_sec: 5,
        }
    }
}

/// Protocol obfuscation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObfuscationMode {
    /// Standard protocol headers.
    Default,
    /// Minimal header information.
    Minimal,
    /// Hide protocol fingerprint entirely.
    Hidden,
    /// User-defined obfuscation plugin.
    Custom,
}

impl fmt::Display for ObfuscationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Minimal => write!(f, "minimal"),
            Self::Hidden => write!(f, "hidden"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// Security and anti-abuse configuration.
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Protocol obfuscation mode.
    pub obfuscation_mode: ObfuscationMode,
    /// Enable honeypot listeners that log scan attempts.
    pub honeypot_enabled: bool,
    /// Enable tarpit mode to slow down abusive connections.
    pub tarpit_enabled: bool,
    /// Maximum throughput for tarpitted connections in bytes per second.
    pub tarpit_throughput_bps: u64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            obfuscation_mode: ObfuscationMode::Default,
            honeypot_enabled: false,
            tarpit_enabled: false,
            tarpit_throughput_bps: 128,
        }
    }
}

/// Management API configuration.
#[derive(Debug, Clone)]
pub struct ManagementApiConfig {
    /// Whether the management API is enabled.
    pub enabled: bool,
    /// Address and port for the management API listener.
    pub listen_addr: String,
    /// API key required for management requests.
    pub api_key: String,
}

impl Default for ManagementApiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_addr: "127.0.0.1:3901".to_string(),
            api_key: String::new(),
        }
    }
}

/// State store type for cluster coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateStoreType {
    /// External Redis cluster.
    Redis,
    /// Embedded Raft consensus.
    EmbeddedRaft,
    /// No shared state; each node is independent.
    Stateless,
}

impl fmt::Display for StateStoreType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Redis => write!(f, "redis"),
            Self::EmbeddedRaft => write!(f, "embedded_raft"),
            Self::Stateless => write!(f, "stateless"),
        }
    }
}

/// Cluster configuration.
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Whether clustering is enabled.
    pub enabled: bool,
    /// State store backend.
    pub state_store: StateStoreType,
    /// Redis connection URL (when using Redis store).
    pub redis_url: String,
    /// Raft peer addresses (when using embedded Raft).
    pub raft_peers: Vec<String>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            state_store: StateStoreType::Stateless,
            redis_url: String::new(),
            raft_peers: Vec::new(),
        }
    }
}
