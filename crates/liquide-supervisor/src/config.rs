//! Configuration types for the supervisor daemon.

use serde::{Deserialize, Serialize};

/// Authentication backend for user authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthBackend {
    /// PAM-based authentication.
    Pam,
    /// LDAP directory authentication.
    Ldap,
    /// OAuth2 / OpenID Connect.
    Oidc,
    /// Certificate-based authentication.
    Certificate,
    /// Developer mode: accept all users.
    DevAllow,
}

impl std::fmt::Display for AuthBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pam => write!(f, "PAM"),
            Self::Ldap => write!(f, "LDAP"),
            Self::Oidc => write!(f, "OIDC"),
            Self::Certificate => write!(f, "Certificate"),
            Self::DevAllow => write!(f, "DevAllow"),
        }
    }
}

/// Top-level supervisor daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorConfig {
    /// Address to listen on for incoming connections.
    pub listen_address: String,
    /// Path to the Unix domain socket for IPC control commands.
    pub control_socket_path: String,
    /// Authentication backend to use.
    pub auth_backend: AuthBackend,
    /// Path to the policy rules file.
    pub policy_rules_path: String,
    /// Directory for crash report files.
    pub crash_report_dir: String,
    /// Directory for log files.
    pub log_dir: String,
    /// Whether developer mode is enabled (relaxed security).
    pub dev_mode: bool,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0:3900".to_string(),
            control_socket_path: "/run/liquide/supervisor.sock".to_string(),
            auth_backend: AuthBackend::Pam,
            policy_rules_path: "/etc/liquide/policy.toml".to_string(),
            crash_report_dir: "/var/log/liquide/crashes".to_string(),
            log_dir: "/var/log/liquide".to_string(),
            dev_mode: false,
        }
    }
}

/// Per-session resource defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefaults {
    /// Default CPU cores per session.
    pub cpu_cores: f64,
    /// Default memory in megabytes per session.
    pub memory_mb: u64,
    /// Maximum number of PIDs per session.
    pub max_pids: u32,
    /// I/O bandwidth limit in megabytes per second.
    pub io_bandwidth_mbps: u32,
    /// Network bandwidth limit in megabits per second.
    pub net_bandwidth_mbps: u32,
    /// Number of encoder threads per session.
    pub encoder_threads: u32,
}

impl Default for ResourceDefaults {
    fn default() -> Self {
        Self {
            cpu_cores: 2.0,
            memory_mb: 512,
            max_pids: 256,
            io_bandwidth_mbps: 10,
            net_bandwidth_mbps: 20,
            encoder_threads: 2,
        }
    }
}

/// Admission control configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionConfig {
    /// Whether admission control is enabled.
    pub enabled: bool,
    /// CPU cores reserved for the supervisor and OS.
    pub reserved_cpu_cores: f64,
    /// Memory in megabytes reserved for the supervisor and OS.
    pub reserved_memory_mb: u64,
    /// Maximum number of concurrent sessions (0 = auto-calculate).
    pub max_sessions: u32,
    /// Whether to queue sessions when at capacity.
    pub queue_enabled: bool,
    /// Timeout in seconds for queued sessions.
    pub queue_timeout_sec: u64,
    /// Deny 4K resolution if host has fewer than this many cores.
    pub deny_4k_below_cores: u32,
    /// Force 30fps cap if host has fewer than this many cores.
    pub deny_60fps_below_cores: u32,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            reserved_cpu_cores: 2.0,
            reserved_memory_mb: 1024,
            max_sessions: 0,
            queue_enabled: false,
            queue_timeout_sec: 30,
            deny_4k_below_cores: 8,
            deny_60fps_below_cores: 4,
        }
    }
}

/// Thresholds for automatic downgrade under host pressure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DowngradeThresholds {
    /// CPU percentage at which FPS is reduced.
    pub reduce_fps_cpu_pct: f64,
    /// CPU percentage at which tile-only mode is forced.
    pub tile_only_cpu_pct: f64,
    /// CPU percentage at which quality is reduced.
    pub reduce_quality_cpu_pct: f64,
    /// CPU percentage at which least-active sessions are suspended.
    pub suspend_cpu_pct: f64,
    /// CPU percentage below which recovery is allowed (hysteresis).
    pub recovery_hysteresis_pct: f64,
    /// Seconds that CPU must stay below recovery threshold before upgrading.
    pub recovery_hold_sec: u64,
}

impl Default for DowngradeThresholds {
    fn default() -> Self {
        Self {
            reduce_fps_cpu_pct: 85.0,
            tile_only_cpu_pct: 90.0,
            reduce_quality_cpu_pct: 95.0,
            suspend_cpu_pct: 95.0,
            recovery_hysteresis_pct: 5.0,
            recovery_hold_sec: 30,
        }
    }
}
