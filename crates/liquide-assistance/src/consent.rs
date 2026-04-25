//! Consent flow for assistance sessions.

use crate::Result;
use crate::message::ConsentPromptMsg;
use crate::mode::{AssistanceMode, Restriction};

/// State of the consent flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentState {
    /// Not yet prompted.
    Pending,
    /// Prompt has been sent, waiting for response.
    AwaitingResponse {
        /// When the prompt was sent.
        prompted_at: u64,
        /// Timeout in seconds.
        timeout_secs: u64,
    },
    /// Owner approved with optional restrictions.
    Approved {
        /// Restrictions applied by the owner.
        restrictions: Vec<Restriction>,
    },
    /// Owner denied the request.
    Denied,
    /// The prompt timed out.
    TimedOut,
}

/// Manages the consent flow for a single assistance request.
pub struct ConsentFlow {
    state: ConsentState,
    observer_id: String,
    observer_name: String,
    observer_role: String,
    requested_mode: AssistanceMode,
    reason: String,
    timeout_secs: u64,
}

impl ConsentFlow {
    /// Create a new consent flow.
    #[must_use]
    pub fn new(
        observer_id: String,
        observer_name: String,
        observer_role: String,
        requested_mode: AssistanceMode,
        reason: String,
        timeout_secs: u64,
    ) -> Self {
        Self {
            state: ConsentState::Pending,
            observer_id,
            observer_name,
            observer_role,
            requested_mode,
            reason,
            timeout_secs,
        }
    }

    /// Send the consent prompt.  Transitions from `Pending` to `AwaitingResponse`.
    pub fn prompt(&mut self, now: u64) -> Result<ConsentPromptMsg> {
        if self.state != ConsentState::Pending {
            return Err(crate::AssistanceError::Internal(
                "consent flow not in Pending state".to_string(),
            ));
        }
        self.state = ConsentState::AwaitingResponse {
            prompted_at: now,
            timeout_secs: self.timeout_secs,
        };
        Ok(ConsentPromptMsg {
            observer_name: self.observer_name.clone(),
            observer_role: self.observer_role.clone(),
            mode: self.requested_mode,
            reason: self.reason.clone(),
            timeout_seconds: self.timeout_secs,
        })
    }

    /// Record the owner's response.
    pub fn respond(
        &mut self,
        accepted: bool,
        restrictions: Vec<Restriction>,
    ) -> Result<ConsentState> {
        match &self.state {
            ConsentState::AwaitingResponse { .. } => {}
            _ => {
                return Err(crate::AssistanceError::Internal(
                    "consent flow not awaiting response".to_string(),
                ));
            }
        }
        if accepted {
            self.state = ConsentState::Approved { restrictions };
        } else {
            self.state = ConsentState::Denied;
        }
        Ok(self.state.clone())
    }

    /// Check if the prompt has timed out.  Returns `true` if it transitioned to `TimedOut`.
    pub fn check_timeout(&mut self, now: u64) -> bool {
        if let ConsentState::AwaitingResponse {
            prompted_at,
            timeout_secs,
        } = self.state
        {
            if now >= prompted_at + timeout_secs {
                self.state = ConsentState::TimedOut;
                return true;
            }
        }
        false
    }

    /// Current state of the consent flow.
    #[must_use]
    pub fn state(&self) -> &ConsentState {
        &self.state
    }

    /// The observer who initiated this consent flow.
    #[must_use]
    pub fn observer_id(&self) -> &str {
        &self.observer_id
    }
}
