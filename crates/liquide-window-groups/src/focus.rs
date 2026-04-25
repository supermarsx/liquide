//! Focus stealing prevention.
//!
//! Implements focus policies inspired by GNOME/KWin that control when a window
//! is allowed to steal focus from the currently focused window. Three levels
//! are provided: Strict (never allow, flash taskbar), Moderate (allow from
//! same app or within a time threshold), and Lenient (always allow).

/// Focus stealing prevention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPolicy {
    /// Never allow focus stealing. The requesting window's taskbar entry
    /// should be flashed/highlighted instead.
    Strict,
    /// Allow focus stealing from the same application, or if the request
    /// arrives within the time threshold of user activity.
    Moderate,
    /// Always allow focus stealing.
    Lenient,
}

impl Default for FocusPolicy {
    fn default() -> Self {
        Self::Moderate
    }
}

/// The reason a window is requesting focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusReason {
    /// User explicitly activated the window (e.g., clicked on it).
    UserActivation,
    /// A new window was just created and wants initial focus.
    NewWindow,
    /// An urgent notification or dialog requires attention.
    Urgency,
    /// The window completed a background task and wants to notify.
    TaskCompletion,
    /// Programmatic request with no specific reason.
    Programmatic,
}

/// A request from a window to receive input focus.
#[derive(Debug, Clone)]
pub struct FocusRequest {
    /// The window requesting focus.
    pub requestor_window_id: u64,
    /// Application ID of the requestor.
    pub requestor_app_id: Option<String>,
    /// Why the window is requesting focus.
    pub reason: FocusReason,
    /// Timestamp (microseconds since epoch) when the request was made.
    pub timestamp_us: u64,
}

impl FocusRequest {
    /// Create a new focus request.
    pub fn new(
        window_id: u64,
        app_id: Option<String>,
        reason: FocusReason,
        timestamp_us: u64,
    ) -> Self {
        Self {
            requestor_window_id: window_id,
            requestor_app_id: app_id,
            reason,
            timestamp_us,
        }
    }
}

/// Information about the currently focused window.
#[derive(Debug, Clone)]
pub struct CurrentFocus {
    /// The currently focused window's ID.
    pub window_id: u64,
    /// Application ID of the focused window.
    pub app_id: Option<String>,
    /// Timestamp (microseconds) of the last user interaction with this window.
    pub last_user_activity_us: u64,
}

impl CurrentFocus {
    /// Create a new CurrentFocus.
    pub fn new(window_id: u64, app_id: Option<String>, last_user_activity_us: u64) -> Self {
        Self {
            window_id,
            app_id,
            last_user_activity_us,
        }
    }
}

/// Result of evaluating a focus steal request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDecision {
    /// Allow the requestor to take focus.
    Allow,
    /// Deny focus, but flash/highlight the requestor's taskbar entry.
    DenyFlash,
    /// Deny focus silently (e.g., the request is from a window being restored).
    DenySilent,
}

/// Default time threshold in microseconds (3 seconds) for Moderate policy.
pub const MODERATE_THRESHOLD_US: u64 = 3_000_000;

/// Evaluate whether a focus steal request should be allowed.
///
/// # Arguments
/// * `request` - The focus request being evaluated.
/// * `current` - Information about the currently focused window. If `None`,
///   there is no focused window and the request is always allowed.
/// * `policy` - The active focus policy.
///
/// # Returns
/// A `FocusDecision` indicating whether to allow or deny the request.
pub fn should_allow_focus_steal(
    request: &FocusRequest,
    current: Option<&CurrentFocus>,
    policy: FocusPolicy,
) -> FocusDecision {
    // If no window currently has focus, always allow.
    let current = match current {
        Some(c) => c,
        None => return FocusDecision::Allow,
    };

    // If the requestor is the currently focused window, always allow.
    if request.requestor_window_id == current.window_id {
        return FocusDecision::Allow;
    }

    // User activation always succeeds regardless of policy.
    if request.reason == FocusReason::UserActivation {
        return FocusDecision::Allow;
    }

    match policy {
        FocusPolicy::Lenient => FocusDecision::Allow,

        FocusPolicy::Strict => {
            // Strict: never allow programmatic focus stealing.
            // Urgency gets a flash instead of silent deny.
            match request.reason {
                FocusReason::Urgency => FocusDecision::DenyFlash,
                FocusReason::NewWindow => FocusDecision::DenyFlash,
                _ => FocusDecision::DenySilent,
            }
        }

        FocusPolicy::Moderate => {
            // Allow from the same application.
            if let (Some(req_app), Some(cur_app)) = (&request.requestor_app_id, &current.app_id) {
                if req_app == cur_app {
                    return FocusDecision::Allow;
                }
            }

            // Allow new windows if the request is recent (within threshold).
            if request.reason == FocusReason::NewWindow {
                let elapsed = request
                    .timestamp_us
                    .saturating_sub(current.last_user_activity_us);
                if elapsed <= MODERATE_THRESHOLD_US {
                    return FocusDecision::Allow;
                }
                return FocusDecision::DenyFlash;
            }

            // Urgency: always flash.
            if request.reason == FocusReason::Urgency {
                return FocusDecision::DenyFlash;
            }

            // Task completion: allow if recent user activity.
            if request.reason == FocusReason::TaskCompletion {
                let elapsed = request
                    .timestamp_us
                    .saturating_sub(current.last_user_activity_us);
                if elapsed <= MODERATE_THRESHOLD_US {
                    return FocusDecision::Allow;
                }
                return FocusDecision::DenyFlash;
            }

            // Programmatic: deny with flash.
            FocusDecision::DenyFlash
        }
    }
}

/// A focus guard that tracks focus policy and recent requests.
#[derive(Debug)]
pub struct FocusGuard {
    /// Current focus policy.
    pub policy: FocusPolicy,
    /// Custom time threshold for Moderate policy (microseconds).
    /// If `None`, uses `MODERATE_THRESHOLD_US`.
    pub threshold_us: Option<u64>,
    /// Number of focus steal attempts denied since last reset.
    denied_count: u64,
    /// Number of focus steal attempts allowed since last reset.
    allowed_count: u64,
}

impl Default for FocusGuard {
    fn default() -> Self {
        Self::new(FocusPolicy::default())
    }
}

impl FocusGuard {
    /// Create a new focus guard with the given policy.
    pub fn new(policy: FocusPolicy) -> Self {
        Self {
            policy,
            threshold_us: None,
            denied_count: 0,
            allowed_count: 0,
        }
    }

    /// Evaluate a focus request and return the decision.
    pub fn evaluate(
        &mut self,
        request: &FocusRequest,
        current: Option<&CurrentFocus>,
    ) -> FocusDecision {
        let decision = should_allow_focus_steal(request, current, self.policy);
        match decision {
            FocusDecision::Allow => self.allowed_count += 1,
            FocusDecision::DenyFlash | FocusDecision::DenySilent => self.denied_count += 1,
        }
        decision
    }

    /// Returns the number of denied focus steal attempts.
    pub fn denied_count(&self) -> u64 {
        self.denied_count
    }

    /// Returns the number of allowed focus steal attempts.
    pub fn allowed_count(&self) -> u64 {
        self.allowed_count
    }

    /// Reset the counters.
    pub fn reset_counters(&mut self) {
        self.denied_count = 0;
        self.allowed_count = 0;
    }
}
