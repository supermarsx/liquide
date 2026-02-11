//! Configuration types for the assistance framework.

/// Top-level assistance configuration.
#[derive(Debug, Clone)]
pub struct AssistanceConfig {
    /// Whether remote assistance is enabled.
    pub enabled: bool,
    /// Maximum number of concurrent observers across all sessions.
    pub max_concurrent_observers: u32,
    /// Default invitation expiry in seconds.
    pub invitation_expiry_seconds: u64,
    /// Timeout for owner consent prompts in seconds.
    pub consent_timeout_seconds: u64,
}

impl Default for AssistanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_observers: 5,
            invitation_expiry_seconds: 300,
            consent_timeout_seconds: 60,
        }
    }
}

/// Per-mode enable/disable configuration.
#[derive(Debug, Clone)]
pub struct ModeConfig {
    /// Whether view-only mode is allowed.
    pub view_only: bool,
    /// Whether interactive mode is allowed.
    pub interactive: bool,
    /// Whether exclusive mode is allowed.
    pub exclusive: bool,
    /// Whether stealth mode is allowed.
    pub stealth: bool,
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            view_only: true,
            interactive: true,
            exclusive: true,
            stealth: false,
        }
    }
}

/// Stealth-mode specific configuration.
#[derive(Debug, Clone)]
pub struct StealthConfig {
    /// Whether stealth mode is enabled.
    pub enabled: bool,
    /// Role required to initiate stealth sessions.
    pub required_role: String,
    /// Interval between audit events in seconds.
    pub audit_interval_seconds: u64,
    /// Maximum stealth session duration in minutes.
    pub max_duration_minutes: u64,
    /// Legal notice displayed when stealth is activated.
    pub legal_notice: String,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            required_role: "security_admin".to_string(),
            audit_interval_seconds: 1,
            max_duration_minutes: 60,
            legal_notice: String::new(),
        }
    }
}

/// Permissions configuration for assistance requests.
#[derive(Debug, Clone)]
pub struct PermissionsConfig {
    /// Whether helpdesk role can request assistance.
    pub helpdesk_can_request: bool,
    /// Whether admin role can force assistance without consent.
    pub admin_can_force: bool,
    /// Whether the user can invite an observer.
    pub user_can_invite: bool,
}

impl Default for PermissionsConfig {
    fn default() -> Self {
        Self {
            helpdesk_can_request: true,
            admin_can_force: false,
            user_can_invite: true,
        }
    }
}

/// Recording configuration for assistance sessions.
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    /// Whether to automatically record assistance sessions.
    pub auto_record: bool,
    /// Whether to include chat messages in the recording.
    pub include_chat: bool,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            auto_record: true,
            include_chat: true,
        }
    }
}
