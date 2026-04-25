//! Observer roles and identity.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::mode::AssistanceMode;

/// Role of an observer in an assistance session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObserverRole {
    /// Help desk technician.
    HelpDesk,
    /// System administrator.
    Admin,
    /// Security administrator (can use stealth mode).
    SecurityAdmin,
    /// Peer user.
    Peer,
}

impl fmt::Display for ObserverRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelpDesk => write!(f, "HelpDesk"),
            Self::Admin => write!(f, "Admin"),
            Self::SecurityAdmin => write!(f, "SecurityAdmin"),
            Self::Peer => write!(f, "Peer"),
        }
    }
}

/// An observer participating in an assistance session.
#[derive(Debug, Clone)]
pub struct Observer {
    /// Unique identifier of the observer.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Role of the observer.
    pub role: ObserverRole,
    /// Current assistance mode.
    pub mode: AssistanceMode,
    /// When the observer connected (unix seconds).
    pub connected_at: u64,
    /// Whether this observer currently has input control.
    pub has_input_control: bool,
}

impl Observer {
    /// Create a new observer.
    #[must_use]
    pub fn new(id: String, name: String, role: ObserverRole, mode: AssistanceMode) -> Self {
        Self {
            id,
            name,
            role,
            mode,
            connected_at: 0,
            has_input_control: false,
        }
    }

    /// Whether this observer can use stealth mode.
    #[must_use]
    pub fn can_stealth(&self) -> bool {
        self.role == ObserverRole::SecurityAdmin
    }

    /// Whether this observer can escalate to the given mode.
    #[must_use]
    pub fn can_escalate_to(&self, mode: AssistanceMode) -> bool {
        match mode {
            AssistanceMode::ViewOnly => true,
            AssistanceMode::Interactive => {
                matches!(
                    self.role,
                    ObserverRole::HelpDesk | ObserverRole::Admin | ObserverRole::SecurityAdmin
                )
            }
            AssistanceMode::Exclusive => {
                matches!(self.role, ObserverRole::Admin | ObserverRole::SecurityAdmin)
            }
            AssistanceMode::Stealth => self.role == ObserverRole::SecurityAdmin,
        }
    }
}
