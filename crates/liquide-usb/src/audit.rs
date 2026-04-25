//! USB audit event types and logging.

use serde::{Deserialize, Serialize};

/// Severity level for audit events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLevel {
    Info,
    Warn,
    Debug,
}

/// Audit events produced by the USB subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UsbAuditEvent {
    /// A device was forwarded to the session.
    DeviceForwarded {
        user: String,
        device_name: String,
        vid_pid: String,
        class: String,
        session_id: String,
    },
    /// A device was disconnected.
    DeviceDisconnected {
        user: String,
        device_name: String,
        vid_pid: String,
        reason: String,
    },
    /// A device was blocked by policy.
    DeviceBlocked {
        user: String,
        device_name: String,
        vid_pid: String,
        class: String,
        block_reason: String,
    },
    /// A security key forward was attempted.
    SecurityKeyForwardAttempt {
        user: String,
        device_name: String,
        vid_pid: String,
        allowed: bool,
    },
    /// A policy violation occurred.
    PolicyViolation {
        user: String,
        device_name: String,
        vid_pid: String,
        policy_rule: String,
    },
}

impl UsbAuditEvent {
    /// Get the audit level for this event.
    #[must_use]
    pub fn level(&self) -> AuditLevel {
        match self {
            Self::DeviceForwarded { .. } => AuditLevel::Info,
            Self::DeviceDisconnected { .. } => AuditLevel::Info,
            Self::DeviceBlocked { .. } => AuditLevel::Warn,
            Self::SecurityKeyForwardAttempt { .. } => AuditLevel::Warn,
            Self::PolicyViolation { .. } => AuditLevel::Warn,
        }
    }

    /// Get a short name for the event type.
    #[must_use]
    pub fn event_name(&self) -> &str {
        match self {
            Self::DeviceForwarded { .. } => "device_forwarded",
            Self::DeviceDisconnected { .. } => "device_disconnected",
            Self::DeviceBlocked { .. } => "device_blocked",
            Self::SecurityKeyForwardAttempt { .. } => "security_key_forward_attempt",
            Self::PolicyViolation { .. } => "policy_violation",
        }
    }
}
