//! Mobile session policy enforcement.

use serde::{Deserialize, Serialize};

/// Policy rules governing what the mobile client is allowed to do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobilePolicy {
    /// Whether the mobile client is allowed to connect at all.
    pub enabled: bool,
    /// Whether clipboard sharing is permitted.
    pub clipboard_enabled: bool,
    /// Whether file transfer is permitted.
    pub file_transfer_enabled: bool,
    /// Maximum session duration in hours (0 = unlimited).
    pub max_session_duration_hours: u32,
    /// Whether biometric authentication is required.
    pub require_biometric: bool,
    /// Whether the device must be managed (MDM enrolled).
    pub require_managed_device: bool,
    /// Whether push notifications are allowed.
    pub push_notifications: bool,
    /// Whether connections over metered (cellular) networks are allowed.
    pub metered_connection_allowed: bool,
}

impl Default for MobilePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            clipboard_enabled: true,
            file_transfer_enabled: true,
            max_session_duration_hours: 0,
            require_biometric: false,
            require_managed_device: false,
            push_notifications: true,
            metered_connection_allowed: true,
        }
    }
}

/// Enforces mobile policy rules against runtime conditions.
pub struct PolicyEnforcer {
    policy: MobilePolicy,
}

impl PolicyEnforcer {
    /// Create a new enforcer with the given policy.
    #[must_use]
    pub fn new(policy: MobilePolicy) -> Self {
        Self { policy }
    }

    /// Reference to the underlying policy.
    #[must_use]
    pub fn policy(&self) -> &MobilePolicy {
        &self.policy
    }

    /// Update the enforced policy.
    pub fn set_policy(&mut self, policy: MobilePolicy) {
        self.policy = policy;
    }

    /// Whether the client is allowed to connect.
    #[must_use]
    pub fn can_connect(&self) -> bool {
        self.policy.enabled
    }

    /// Whether clipboard sharing is allowed.
    #[must_use]
    pub fn can_use_clipboard(&self) -> bool {
        self.policy.clipboard_enabled
    }

    /// Whether file transfer is allowed.
    #[must_use]
    pub fn can_transfer_files(&self) -> bool {
        self.policy.file_transfer_enabled
    }

    /// Remaining session time in seconds, or `None` if there is no limit.
    ///
    /// `started_at` and `now` are epoch seconds.
    #[must_use]
    pub fn remaining_session_time(&self, started_at: u64, now: u64) -> Option<u64> {
        if self.policy.max_session_duration_hours == 0 {
            return None;
        }
        let max_seconds = u64::from(self.policy.max_session_duration_hours) * 3600;
        let elapsed = now.saturating_sub(started_at);
        if elapsed >= max_seconds {
            Some(0)
        } else {
            Some(max_seconds - elapsed)
        }
    }

    /// Whether the session has exceeded its maximum allowed duration.
    ///
    /// `started_at` and `now` are epoch seconds.
    #[must_use]
    pub fn is_session_expired(&self, started_at: u64, now: u64) -> bool {
        if self.policy.max_session_duration_hours == 0 {
            return false;
        }
        let max_seconds = u64::from(self.policy.max_session_duration_hours) * 3600;
        let elapsed = now.saturating_sub(started_at);
        elapsed >= max_seconds
    }
}
