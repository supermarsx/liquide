//! Focus management — tracking focused window and focus history.

use serde::{Deserialize, Serialize};

use liquide_window_groups::{
    CurrentFocus, FocusDecision, FocusGuard, FocusPolicy as GroupFocusPolicy, FocusReason,
    FocusRequest,
};

use crate::window::WindowId;

/// Focus policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusPolicy {
    ClickToFocus,
    FocusFollowsMouse,
}

/// Manages window focus and focus history.
///
/// In addition to tracking the focused window and history, the manager
/// embeds the canonical `liquide-window-groups` [`FocusGuard`], so
/// non-user-initiated focus requests (notably newly-created windows) are
/// evaluated against the active focus-stealing-prevention policy instead of
/// unconditionally stealing focus.
pub struct FocusManager {
    focused: Option<WindowId>,
    history: Vec<WindowId>,
    policy: FocusPolicy,
    /// Canonical focus-stealing-prevention guard (`liquide-window-groups`).
    guard: FocusGuard,
    /// App id of the currently-focused window, for same-app focus decisions.
    current_app: Option<String>,
    /// Timestamp (us) of the last accepted (user-driven) focus change.
    last_activity_us: u64,
}

impl FocusManager {
    /// Create a new focus manager.
    #[must_use]
    pub fn new(policy: FocusPolicy) -> Self {
        Self {
            focused: None,
            history: Vec::new(),
            policy,
            guard: FocusGuard::new(GroupFocusPolicy::default()),
            current_app: None,
            last_activity_us: 0,
        }
    }

    /// Get the currently focused window.
    #[must_use]
    pub fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    /// Set focus to a window.
    ///
    /// This is the unconditional, user-driven activation path: it always
    /// succeeds (mirroring `FocusReason::UserActivation` in the canonical
    /// policy). Programmatic / new-window focus requests that must respect the
    /// focus-stealing policy should go through [`Self::request_focus`].
    pub fn set_focus(&mut self, id: WindowId) {
        self.history.retain(|w| *w != id);
        if let Some(prev) = self.focused {
            if prev != id {
                self.history.push(prev);
            }
        }
        self.focused = Some(id);
    }

    /// Update the bookkeeping used by the focus-stealing guard after a focus
    /// change has been accepted: record the focused window's app id and the
    /// time of the (user) activity. Called by the shell on `set_focus`.
    pub fn note_focus_context(&mut self, app_id: Option<String>, timestamp_us: u64) {
        self.current_app = app_id;
        self.last_activity_us = timestamp_us;
    }

    /// Evaluate a non-user-initiated focus request (e.g. a newly-created
    /// window asking for initial focus) against the canonical
    /// focus-stealing-prevention policy. Returns the [`FocusDecision`] without
    /// mutating the focused window.
    pub fn evaluate_focus_request(
        &mut self,
        window_id: WindowId,
        app_id: Option<String>,
        reason: FocusReason,
        timestamp_us: u64,
    ) -> FocusDecision {
        let request = FocusRequest::new(window_id.0, app_id, reason, timestamp_us);
        let current = self
            .focused
            .map(|w| CurrentFocus::new(w.0, self.current_app.clone(), self.last_activity_us));
        self.guard.evaluate(&request, current.as_ref())
    }

    /// Request focus for a window subject to the focus-stealing policy.
    ///
    /// Returns `true` and moves focus to `window_id` only if the canonical
    /// policy [`FocusDecision::Allow`]s the steal; otherwise focus is left
    /// unchanged (the caller may flash the taskbar entry on `DenyFlash`).
    pub fn request_focus(
        &mut self,
        window_id: WindowId,
        app_id: Option<String>,
        reason: FocusReason,
        timestamp_us: u64,
    ) -> bool {
        match self.evaluate_focus_request(window_id, app_id.clone(), reason, timestamp_us) {
            FocusDecision::Allow => {
                self.set_focus(window_id);
                self.note_focus_context(app_id, timestamp_us);
                true
            }
            FocusDecision::DenyFlash | FocusDecision::DenySilent => false,
        }
    }

    /// Set the canonical focus-stealing-prevention policy.
    pub fn set_steal_policy(&mut self, policy: GroupFocusPolicy) {
        self.guard.policy = policy;
    }

    /// Get the canonical focus-stealing-prevention policy.
    #[must_use]
    pub fn steal_policy(&self) -> GroupFocusPolicy {
        self.guard.policy
    }

    /// Number of focus steal attempts denied by the guard since last reset.
    #[must_use]
    pub fn denied_steal_count(&self) -> u64 {
        self.guard.denied_count()
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
            // The focused-window app context no longer applies; the next
            // accepted focus change will refresh it.
            self.current_app = None;
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
