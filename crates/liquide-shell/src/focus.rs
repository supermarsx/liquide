//! Focus management — tracking focused window and focus history.

use serde::{Deserialize, Serialize};

use crate::window::WindowId;

/// Focus policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusPolicy {
    ClickToFocus,
    FocusFollowsMouse,
}

/// Manages window focus and focus history.
pub struct FocusManager {
    focused: Option<WindowId>,
    history: Vec<WindowId>,
    policy: FocusPolicy,
}

impl FocusManager {
    /// Create a new focus manager.
    #[must_use]
    pub fn new(policy: FocusPolicy) -> Self {
        Self {
            focused: None,
            history: Vec::new(),
            policy,
        }
    }

    /// Get the currently focused window.
    #[must_use]
    pub fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    /// Set focus to a window.
    pub fn set_focus(&mut self, id: WindowId) {
        self.history.retain(|w| *w != id);
        if let Some(prev) = self.focused {
            if prev != id {
                self.history.push(prev);
            }
        }
        self.focused = Some(id);
    }

    /// Clear focus.
    pub fn clear_focus(&mut self) {
        if let Some(prev) = self.focused.take() {
            self.history.push(prev);
        }
    }

    /// Focus the next window in history (cycle forward).
    pub fn focus_next(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next = self.history.remove(0);
        if let Some(current) = self.focused {
            self.history.push(current);
        }
        self.focused = Some(next);
    }

    /// Focus the previous window in history (cycle backward).
    pub fn focus_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let prev = self.history.pop().unwrap();
        if let Some(current) = self.focused {
            self.history.insert(0, current);
        }
        self.focused = Some(prev);
    }

    /// Get the focus history.
    #[must_use]
    pub fn history(&self) -> &[WindowId] {
        &self.history
    }

    /// Remove a window from focus tracking (e.g. when closed).
    pub fn remove_window(&mut self, id: WindowId) {
        self.history.retain(|w| *w != id);
        if self.focused == Some(id) {
            self.focused = self.history.pop();
        }
    }

    /// Get the focus policy.
    #[must_use]
    pub fn policy(&self) -> FocusPolicy {
        self.policy
    }

    /// Set the focus policy.
    pub fn set_policy(&mut self, policy: FocusPolicy) {
        self.policy = policy;
    }
}

impl std::fmt::Display for FocusPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClickToFocus => write!(f, "ClickToFocus"),
            Self::FocusFollowsMouse => write!(f, "FocusFollowsMouse"),
        }
    }
}
