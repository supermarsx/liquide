//! Session state machine with validated transitions.

use std::fmt;
use std::time::Instant;

use crate::{SessionError, Result};

/// States a session can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Session has been created but not yet authenticated.
    Created,
    /// Session is in the authentication phase.
    Authenticating,
    /// Session is fully running and active.
    Running,
    /// Session is locked (e.g. due to idle timeout or user action).
    Locked,
    /// Client has disconnected; session persists server-side.
    Disconnected,
    /// Session is suspended to save resources.
    Suspended,
    /// Session crashed and may be restarted.
    Crashed,
    /// Session has entered a permanent failure state.
    Failed,
    /// Session has been cleanly terminated.
    Terminated,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "Created"),
            Self::Authenticating => write!(f, "Authenticating"),
            Self::Running => write!(f, "Running"),
            Self::Locked => write!(f, "Locked"),
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Suspended => write!(f, "Suspended"),
            Self::Crashed => write!(f, "Crashed"),
            Self::Failed => write!(f, "Failed"),
            Self::Terminated => write!(f, "Terminated"),
        }
    }
}

impl SessionState {
    /// Returns all states that are valid transition targets from this state.
    #[must_use]
    pub fn valid_transitions(&self) -> &[SessionState] {
        match self {
            Self::Created => &[SessionState::Authenticating, SessionState::Terminated],
            Self::Authenticating => &[
                SessionState::Running,
                SessionState::Failed,
                SessionState::Terminated,
            ],
            Self::Running => &[
                SessionState::Locked,
                SessionState::Disconnected,
                SessionState::Suspended,
                SessionState::Crashed,
                SessionState::Terminated,
            ],
            Self::Locked => &[
                SessionState::Running,
                SessionState::Disconnected,
                SessionState::Terminated,
            ],
            Self::Disconnected => &[
                SessionState::Running,
                SessionState::Suspended,
                SessionState::Terminated,
            ],
            Self::Suspended => &[
                SessionState::Running,
                SessionState::Terminated,
            ],
            Self::Crashed => &[
                SessionState::Running,
                SessionState::Failed,
                SessionState::Terminated,
            ],
            Self::Failed => &[SessionState::Terminated],
            Self::Terminated => &[],
        }
    }
}

/// State machine tracking the lifecycle of a single session.
pub struct SessionStateMachine {
    session_id: String,
    state: SessionState,
    created_at: Instant,
    last_transition: Instant,
    transition_count: u64,
    safe_mode: bool,
}

impl SessionStateMachine {
    /// Create a new state machine for the given session.
    #[must_use]
    pub fn new(session_id: String) -> Self {
        let now = Instant::now();
        Self {
            session_id,
            state: SessionState::Created,
            created_at: now,
            last_transition: now,
            transition_count: 0,
            safe_mode: false,
        }
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// The session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Whether the session is in safe mode.
    #[must_use]
    pub fn is_safe_mode(&self) -> bool {
        self.safe_mode
    }

    /// Set safe mode on or off.
    pub fn set_safe_mode(&mut self, enabled: bool) {
        self.safe_mode = enabled;
    }

    /// The number of state transitions that have occurred.
    #[must_use]
    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    /// Returns the set of valid target states from the current state.
    #[must_use]
    pub fn valid_transitions(&self) -> &[SessionState] {
        self.state.valid_transitions()
    }

    /// Attempt to transition to a new state.
    ///
    /// Returns `Ok(())` if the transition is valid, or an error if not.
    pub fn transition_to(&mut self, target: SessionState) -> Result<()> {
        let valid = self.state.valid_transitions();
        if !valid.contains(&target) {
            return Err(SessionError::InvalidStateTransition {
                from: self.state.to_string(),
                to: target.to_string(),
            });
        }
        self.state = target;
        self.last_transition = Instant::now();
        self.transition_count += 1;
        Ok(())
    }

    /// Seconds since the session was created.
    #[must_use]
    pub fn uptime_seconds(&self) -> u64 {
        self.created_at.elapsed().as_secs()
    }

    /// Seconds since the last state transition.
    #[must_use]
    pub fn seconds_since_last_transition(&self) -> u64 {
        self.last_transition.elapsed().as_secs()
    }
}
