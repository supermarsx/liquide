//! Stealth session management.

use crate::config::StealthConfig;
use crate::observer::{Observer, ObserverRole};
use crate::{AssistanceError, Result};

/// A stealth monitoring session.
#[derive(Debug)]
pub struct StealthSession {
    observer_id: String,
    target_session_id: String,
    started_at: u64,
    last_audit_at: u64,
    config: StealthConfig,
}

impl StealthSession {
    /// Create a new stealth session.  The observer must have the `SecurityAdmin` role.
    #[must_use]
    pub fn new(observer: &Observer, target_session_id: String, config: StealthConfig) -> Result<Self> {
        if observer.role != ObserverRole::SecurityAdmin {
            return Err(AssistanceError::StealthRoleRequired {
                required_role: config.required_role.clone(),
            });
        }
        Ok(Self {
            observer_id: observer.id.clone(),
            target_session_id,
            started_at: 0,
            last_audit_at: 0,
            config,
        })
    }

    /// Whether an audit event should be emitted at the given time.
    #[must_use]
    pub fn should_emit_audit(&self, now: u64) -> bool {
        now >= self.last_audit_at + self.config.audit_interval_seconds
    }

    /// Whether the stealth session has exceeded its maximum duration.
    #[must_use]
    pub fn is_expired(&self, now: u64) -> bool {
        let max_seconds = self.config.max_duration_minutes * 60;
        now >= self.started_at + max_seconds
    }

    /// Record that an audit event was emitted.
    pub fn record_audit(&mut self, now: u64) {
        self.last_audit_at = now;
    }

    /// Duration in seconds since the session started.
    #[must_use]
    pub fn duration_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.started_at)
    }

    /// The observer that initiated this stealth session.
    #[must_use]
    pub fn observer_id(&self) -> &str {
        &self.observer_id
    }

    /// The target session being monitored.
    #[must_use]
    pub fn target_session_id(&self) -> &str {
        &self.target_session_id
    }

    /// When the session was started.
    #[must_use]
    pub fn started_at(&self) -> u64 {
        self.started_at
    }

    /// Set the start timestamp.
    pub fn set_started_at(&mut self, ts: u64) {
        self.started_at = ts;
        self.last_audit_at = ts;
    }
}
