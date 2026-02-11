//! Shadow session state and management.

use serde::{Deserialize, Serialize};

use crate::mode::AssistanceMode;

/// State of a shadow session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowSessionState {
    /// Waiting for consent.
    Pending,
    /// Session is active.
    Active,
    /// An escalation is in progress.
    Escalating,
    /// Session has ended.
    Ended,
}

/// A shadow session that tracks a remote assistance connection.
pub struct ShadowSession {
    id: String,
    target_session_id: String,
    state: ShadowSessionState,
    mode: AssistanceMode,
    observers: Vec<String>,
    created_at: u64,
    ended_at: Option<u64>,
    recording_enabled: bool,
}

impl ShadowSession {
    /// Create a new shadow session.
    #[must_use]
    pub fn new(id: String, target_session_id: String, mode: AssistanceMode) -> Self {
        Self {
            id,
            target_session_id,
            state: ShadowSessionState::Pending,
            mode,
            observers: Vec::new(),
            created_at: 0,
            ended_at: None,
            recording_enabled: true,
        }
    }

    /// The session identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The target session being shadowed.
    #[must_use]
    pub fn target_session_id(&self) -> &str {
        &self.target_session_id
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> ShadowSessionState {
        self.state
    }

    /// Current assistance mode.
    #[must_use]
    pub fn mode(&self) -> AssistanceMode {
        self.mode
    }

    /// List of observer identifiers.
    #[must_use]
    pub fn observers(&self) -> &[String] {
        &self.observers
    }

    /// When the session was created.
    #[must_use]
    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    /// When the session ended, if it has.
    #[must_use]
    pub fn ended_at(&self) -> Option<u64> {
        self.ended_at
    }

    /// Whether recording is enabled.
    #[must_use]
    pub fn recording_enabled(&self) -> bool {
        self.recording_enabled
    }

    /// Add an observer to the session and activate it.
    pub fn add_observer(&mut self, observer_id: String) {
        if !self.observers.contains(&observer_id) {
            self.observers.push(observer_id);
        }
        if self.state == ShadowSessionState::Pending {
            self.state = ShadowSessionState::Active;
        }
    }

    /// Remove an observer from the session.
    pub fn remove_observer(&mut self, observer_id: &str) {
        self.observers.retain(|id| id != observer_id);
    }

    /// End the session.
    pub fn end(&mut self, timestamp: u64) {
        self.state = ShadowSessionState::Ended;
        self.ended_at = Some(timestamp);
    }

    /// Begin an escalation.
    pub fn escalate(&mut self, new_mode: AssistanceMode) {
        self.state = ShadowSessionState::Escalating;
        self.mode = new_mode;
    }

    /// Whether the session is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == ShadowSessionState::Active || self.state == ShadowSessionState::Escalating
    }
}

/// Serializable summary of a shadow session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session identifier.
    pub id: String,
    /// Target session being shadowed.
    pub target_session_id: String,
    /// Human-readable state name.
    pub state: String,
    /// Current mode.
    pub mode: AssistanceMode,
    /// Number of active observers.
    pub observer_count: usize,
    /// Creation timestamp.
    pub created_at: u64,
}

impl SessionInfo {
    /// Create a `SessionInfo` from a `ShadowSession`.
    #[must_use]
    pub fn from_session(session: &ShadowSession) -> Self {
        let state_name = match session.state() {
            ShadowSessionState::Pending => "Pending",
            ShadowSessionState::Active => "Active",
            ShadowSessionState::Escalating => "Escalating",
            ShadowSessionState::Ended => "Ended",
        };
        Self {
            id: session.id().to_string(),
            target_session_id: session.target_session_id().to_string(),
            state: state_name.to_string(),
            mode: session.mode(),
            observer_count: session.observers().len(),
            created_at: session.created_at(),
        }
    }
}
