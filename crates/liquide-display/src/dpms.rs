//! Display Power Management Signaling (DPMS).
//!
//! Tracks idle time and transitions displays through power states:
//! On -> Standby -> Suspend -> Off.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// DPMS state machine
// ---------------------------------------------------------------------------

/// Display power state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DpmsState {
    /// Display is fully on.
    On,
    /// Display is in low-power standby (quick resume).
    Standby,
    /// Display is suspended (slower resume).
    Suspend,
    /// Display is off (deepest power save).
    Off,
}

impl Default for DpmsState {
    fn default() -> Self {
        DpmsState::On
    }
}

impl DpmsState {
    /// Return the "depth" of this state (higher = deeper sleep).
    pub fn depth(&self) -> u8 {
        match self {
            DpmsState::On => 0,
            DpmsState::Standby => 1,
            DpmsState::Suspend => 2,
            DpmsState::Off => 3,
        }
    }

    /// Whether the display is in a power-saving state.
    pub fn is_power_save(&self) -> bool {
        !matches!(self, DpmsState::On)
    }
}

// ---------------------------------------------------------------------------
// DPMS policy
// ---------------------------------------------------------------------------

/// Timeout policy for DPMS transitions.
///
/// Each timeout is in seconds from last user input. Set to 0 to disable
/// a particular transition (skip straight to the next).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpmsPolicy {
    /// Seconds of idle before entering Standby (0 = skip).
    pub standby_timeout_secs: u32,
    /// Seconds of idle before entering Suspend (0 = skip).
    pub suspend_timeout_secs: u32,
    /// Seconds of idle before entering Off (0 = skip).
    pub off_timeout_secs: u32,
    /// Whether DPMS is enabled at all.
    pub enabled: bool,
}

impl Default for DpmsPolicy {
    fn default() -> Self {
        Self {
            standby_timeout_secs: 300, // 5 minutes
            suspend_timeout_secs: 600, // 10 minutes
            off_timeout_secs: 900,     // 15 minutes
            enabled: true,
        }
    }
}

impl DpmsPolicy {
    /// Create a policy with a single timeout (goes straight to Off).
    pub fn single_timeout(off_secs: u32) -> Self {
        Self {
            standby_timeout_secs: 0,
            suspend_timeout_secs: 0,
            off_timeout_secs: off_secs,
            enabled: true,
        }
    }

    /// Create a disabled policy (display always on).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Determine the target DPMS state for a given idle duration (in seconds).
    pub fn target_state(&self, idle_secs: u32) -> DpmsState {
        if !self.enabled {
            return DpmsState::On;
        }
        // Check from deepest to shallowest.
        if self.off_timeout_secs > 0 && idle_secs >= self.off_timeout_secs {
            return DpmsState::Off;
        }
        if self.suspend_timeout_secs > 0 && idle_secs >= self.suspend_timeout_secs {
            return DpmsState::Suspend;
        }
        if self.standby_timeout_secs > 0 && idle_secs >= self.standby_timeout_secs {
            return DpmsState::Standby;
        }
        DpmsState::On
    }
}

// ---------------------------------------------------------------------------
// DPMS controller
// ---------------------------------------------------------------------------

/// Tracks display power state and manages transitions based on idle time.
#[derive(Debug, Clone)]
pub struct DpmsController {
    /// Current power state.
    state: DpmsState,
    /// Active policy.
    policy: DpmsPolicy,
    /// Accumulated idle time in seconds (reset on user input).
    idle_secs: u32,
    /// Whether a wake event is pending (set on input, cleared after processing).
    wake_pending: bool,
}

impl DpmsController {
    /// Create a new controller with the given policy.
    pub fn new(policy: DpmsPolicy) -> Self {
        Self {
            state: DpmsState::On,
            policy,
            idle_secs: 0,
            wake_pending: false,
        }
    }

    /// Get the current DPMS state.
    pub fn state(&self) -> DpmsState {
        self.state
    }

    /// Get the current idle time in seconds.
    pub fn idle_secs(&self) -> u32 {
        self.idle_secs
    }

    /// Get a reference to the active policy.
    pub fn policy(&self) -> &DpmsPolicy {
        &self.policy
    }

    /// Update the policy.
    pub fn set_policy(&mut self, policy: DpmsPolicy) {
        self.policy = policy;
    }

    /// Notify the controller of user input (mouse move, key press, etc.).
    ///
    /// This resets the idle timer and schedules a wake-up if the display
    /// is in a power-save state.
    pub fn notify_input(&mut self) {
        self.idle_secs = 0;
        if self.state.is_power_save() {
            self.wake_pending = true;
        }
    }

    /// Advance the idle timer by `delta_secs` and return the new DPMS state
    /// if it changed. Returns `None` if the state didn't change.
    ///
    /// Call this periodically (e.g., once per second from a timer).
    pub fn tick(&mut self, delta_secs: u32) -> Option<DpmsState> {
        // Process pending wake.
        if self.wake_pending {
            self.wake_pending = false;
            if self.state != DpmsState::On {
                self.state = DpmsState::On;
                return Some(DpmsState::On);
            }
        }

        self.idle_secs = self.idle_secs.saturating_add(delta_secs);

        let target = self.policy.target_state(self.idle_secs);
        if target != self.state {
            self.state = target;
            Some(target)
        } else {
            None
        }
    }

    /// Force the display to a specific state, bypassing the policy.
    pub fn force_state(&mut self, state: DpmsState) {
        self.state = state;
        if state == DpmsState::On {
            self.idle_secs = 0;
        }
    }

    /// Check whether a wake event was triggered since last `tick()`.
    pub fn has_wake_pending(&self) -> bool {
        self.wake_pending
    }
}
