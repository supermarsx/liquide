//! Configuration types for session management.

use crate::resume::TokenScope;
use crate::sandbox::JailType;

/// Top-level session configuration.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Whether automatic resume is enabled after disconnect.
    pub auto_resume: bool,
    /// Seconds before a disconnected session is terminated.
    pub disconnect_timeout_sec: u64,
    /// Seconds of inactivity before the session locks.
    pub idle_lock_sec: u64,
    /// Seconds of inactivity before the session suspends (0 = disabled).
    pub idle_suspend_sec: u64,
    /// Maximum session duration in seconds.
    pub max_duration_sec: u64,
    /// Maximum concurrent sessions per user.
    pub max_per_user: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_resume: true,
            disconnect_timeout_sec: 3600,
            idle_lock_sec: 300,
            idle_suspend_sec: 0,
            max_duration_sec: 86400,
            max_per_user: 3,
        }
    }
}

/// Multi-client connection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiClientMode {
    /// New connection steals the session from the current client.
    Steal,
    /// Multiple clients can view simultaneously.
    Mirror,
    /// Additional connections are denied.
    Deny,
    /// Additional connections are view-only.
    ViewOnly,
}

impl std::fmt::Display for MultiClientMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Steal => write!(f, "Steal"),
            Self::Mirror => write!(f, "Mirror"),
            Self::Deny => write!(f, "Deny"),
            Self::ViewOnly => write!(f, "ViewOnly"),
        }
    }
}

/// Configuration for multi-client session handling.
#[derive(Debug, Clone)]
pub struct MultiClientConfig {
    /// How to handle additional client connections.
    pub mode: MultiClientMode,
    /// Maximum clients in mirror mode.
    pub mirror_max_clients: u32,
    /// Whether to show remote cursors in mirror mode.
    pub mirror_show_remote_cursor: bool,
}

impl Default for MultiClientConfig {
    fn default() -> Self {
        Self {
            mode: MultiClientMode::Steal,
            mirror_max_clients: 4,
            mirror_show_remote_cursor: true,
        }
    }
}

/// Configuration for the session supervisor.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Heartbeat interval in seconds.
    pub heartbeat_interval_sec: u64,
    /// Number of consecutive missed heartbeats before timeout.
    pub heartbeat_timeout_count: u32,
    /// Maximum restart attempts within the restart window.
    pub max_restarts: u32,
    /// Window in seconds for counting restart attempts.
    pub restart_window_sec: u64,
    /// Base backoff delay in milliseconds for restarts.
    pub restart_backoff_base_ms: u64,
    /// Directory for crash report files.
    pub crash_report_dir: String,
    /// Whether core dumps are enabled.
    pub coredump_enabled: bool,
    /// Number of log lines to capture in crash reports.
    pub crash_log_lines: u32,
    /// Number of restarts before entering safe mode.
    pub safe_mode_after_restart: u32,
    /// Whether to quarantine plugins that cause crashes.
    pub plugin_quarantine_enabled: bool,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_sec: 5,
            heartbeat_timeout_count: 3,
            max_restarts: 5,
            restart_window_sec: 600,
            restart_backoff_base_ms: 1000,
            crash_report_dir: "/var/lib/liquide/crash".to_string(),
            coredump_enabled: true,
            crash_log_lines: 100,
            safe_mode_after_restart: 3,
            plugin_quarantine_enabled: true,
        }
    }
}

/// Resource limits for a session.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum CPU cores allocated.
    pub cpu_cores: f64,
    /// Maximum memory in megabytes.
    pub memory_mb: u64,
    /// Maximum I/O bandwidth in megabits per second.
    pub io_bandwidth_mbps: u64,
    /// Maximum number of processes.
    pub max_pids: u32,
    /// Maximum network bandwidth in megabits per second.
    pub network_bandwidth_mbps: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_cores: 2.0,
            memory_mb: 512,
            io_bandwidth_mbps: 10,
            max_pids: 256,
            network_bandwidth_mbps: 20,
        }
    }
}

/// Configuration for session resume tokens.
#[derive(Debug, Clone)]
pub struct ResumeConfig {
    /// Whether session resume is enabled.
    pub enabled: bool,
    /// Lifetime of resume tokens in hours.
    pub token_lifetime_hours: u64,
    /// Whether tokens are rotated on each use.
    pub token_rotation: bool,
    /// Scope of the resume token.
    pub token_scope: TokenScope,
    /// Maximum minutes a session can be disconnected before resume is rejected.
    pub max_disconnected_minutes: u64,
    /// Whether MFA is required when resuming.
    pub require_mfa_on_resume: bool,
    /// Hours after which MFA is required on resume.
    pub require_mfa_after_hours: u64,
}

impl Default for ResumeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            token_lifetime_hours: 168,
            token_rotation: true,
            token_scope: TokenScope::SameServer,
            max_disconnected_minutes: 60,
            require_mfa_on_resume: false,
            require_mfa_after_hours: 24,
        }
    }
}

/// Network mode for the sandbox jail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JailNetwork {
    /// Use the host network namespace.
    Host,
    /// Use an isolated network namespace.
    Isolated,
    /// No network access.
    None,
}

impl std::fmt::Display for JailNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host => write!(f, "Host"),
            Self::Isolated => write!(f, "Isolated"),
            Self::None => write!(f, "None"),
        }
    }
}

/// Configuration for session sandboxing.
#[derive(Debug, Clone)]
pub struct JailConfig {
    /// Type of jail to apply.
    pub jail_type: JailType,
    /// Paths allowed inside the jail.
    pub allowed_paths: Vec<String>,
    /// System calls denied in the jail.
    pub denied_syscalls: Vec<String>,
    /// Network mode for the jail.
    pub network: JailNetwork,
    /// Maximum number of processes inside the jail.
    pub max_processes: u32,
    /// Maximum memory in megabytes.
    pub max_memory_mb: u64,
    /// Maximum disk usage in megabytes.
    pub max_disk_mb: u64,
}

impl Default for JailConfig {
    fn default() -> Self {
        Self {
            jail_type: JailType::None,
            allowed_paths: vec![
                "/usr".to_string(),
                "/lib".to_string(),
                "/etc/liquide".to_string(),
            ],
            denied_syscalls: vec![
                "ptrace".to_string(),
                "mount".to_string(),
                "reboot".to_string(),
            ],
            network: JailNetwork::Host,
            max_processes: 200,
            max_memory_mb: 4096,
            max_disk_mb: 10240,
        }
    }
}
