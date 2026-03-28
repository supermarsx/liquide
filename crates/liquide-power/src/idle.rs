//! Idle detection and state machine.
//!
//! Tracks how long the system has been idle (no user input) and transitions
//! through progressive power-saving states: Active -> Idle -> DimDisplay ->
//! ScreenOff -> Suspend. Each transition has a configurable timeout.
//!
//! The shell calls [`IdleTracker::reset`] on every input event and
//! [`IdleTracker::tick`] on every frame/poll to advance the state machine.

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Idle state
// ---------------------------------------------------------------------------

/// Progressive idle states, ordered from most active to deepest sleep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdleState {
    /// User is actively interacting.
    Active,
    /// No input for a short period; system is idle but screen is on.
    Idle,
    /// Display has been dimmed.
    DimDisplay,
    /// Display has been turned off.
    ScreenOff,
    /// System is about to suspend or has suspended.
    Suspend,
}

impl std::fmt::Display for IdleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::DimDisplay => "dim-display",
            Self::ScreenOff => "screen-off",
            Self::Suspend => "suspend",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Idle policy (timeouts)
// ---------------------------------------------------------------------------

/// Configurable timeouts for each idle transition. A timeout of `Duration::ZERO`
/// means the transition is skipped (jumps to the next state). A timeout of
/// `Duration::MAX` means the transition never happens automatically.
#[derive(Debug, Clone, PartialEq)]
pub struct IdlePolicy {
    /// Time from Active -> Idle.
    pub idle_timeout: Duration,
    /// Time from Idle -> DimDisplay.
    pub dim_timeout: Duration,
    /// Time from DimDisplay -> ScreenOff.
    pub screen_off_timeout: Duration,
    /// Time from ScreenOff -> Suspend.
    pub suspend_timeout: Duration,
}

impl IdlePolicy {
    /// Default desktop policy (5 min idle, 30 sec dim, 2 min screen off,
    /// 10 min suspend).
    pub fn desktop() -> Self {
        Self {
            idle_timeout: Duration::from_secs(300),
            dim_timeout: Duration::from_secs(30),
            screen_off_timeout: Duration::from_secs(120),
            suspend_timeout: Duration::from_secs(600),
        }
    }

    /// Aggressive battery-saving policy.
    pub fn battery_saver() -> Self {
        Self {
            idle_timeout: Duration::from_secs(60),
            dim_timeout: Duration::from_secs(15),
            screen_off_timeout: Duration::from_secs(30),
            suspend_timeout: Duration::from_secs(120),
        }
    }

    /// Never-sleep policy (presentation mode).
    pub fn presentation() -> Self {
        Self {
            idle_timeout: Duration::MAX,
            dim_timeout: Duration::MAX,
            screen_off_timeout: Duration::MAX,
            suspend_timeout: Duration::MAX,
        }
    }

    /// Returns the cumulative timeout from Active to the given state.
    pub fn time_to_state(&self, state: IdleState) -> Duration {
        match state {
            IdleState::Active => Duration::ZERO,
            IdleState::Idle => self.idle_timeout,
            IdleState::DimDisplay => self.idle_timeout.saturating_add(self.dim_timeout),
            IdleState::ScreenOff => self
                .idle_timeout
                .saturating_add(self.dim_timeout)
                .saturating_add(self.screen_off_timeout),
            IdleState::Suspend => self
                .idle_timeout
                .saturating_add(self.dim_timeout)
                .saturating_add(self.screen_off_timeout)
                .saturating_add(self.suspend_timeout),
        }
    }
}

impl Default for IdlePolicy {
    fn default() -> Self {
        Self::desktop()
    }
}

// ---------------------------------------------------------------------------
// Idle tracker
// ---------------------------------------------------------------------------

/// Callback type for idle state transitions.
pub type IdleTransitionCallback = Box<dyn Fn(IdleState, IdleState) + Send + Sync>;

/// Tracks user idle time and transitions through [`IdleState`]s.
pub struct IdleTracker {
    policy: IdlePolicy,
    state: IdleState,
    /// When the user was last active.
    last_input: Instant,
    /// When the current state was entered.
    state_entered: Instant,
    /// Callbacks notified on state transitions.
    callbacks: Vec<IdleTransitionCallback>,
}

impl IdleTracker {
    /// Create a new tracker with the given policy, starting in `Active` state.
    pub fn new(policy: IdlePolicy) -> Self {
        let now = Instant::now();
        Self {
            policy,
            state: IdleState::Active,
            last_input: now,
            state_entered: now,
            callbacks: Vec::new(),
        }
    }

    /// Current idle state.
    pub fn state(&self) -> IdleState {
        self.state
    }

    /// How long since the last user input.
    pub fn idle_duration(&self) -> Duration {
        self.last_input.elapsed()
    }

    /// How long the tracker has been in the current state.
    pub fn time_in_state(&self) -> Duration {
        self.state_entered.elapsed()
    }

    /// Reset idle timer (call on every user input event). Returns the
    /// previous state if a transition back to `Active` occurred.
    pub fn reset(&mut self) -> Option<IdleState> {
        let now = Instant::now();
        self.last_input = now;
        let prev = self.state;
        if prev != IdleState::Active {
            self.state = IdleState::Active;
            self.state_entered = now;
            self.notify(prev, IdleState::Active);
            Some(prev)
        } else {
            None
        }
    }

    /// Advance the state machine. Call periodically (e.g., each frame or
    /// every second). Returns any state transitions that occurred.
    pub fn tick(&mut self) -> Vec<(IdleState, IdleState)> {
        let idle_time = self.last_input.elapsed();
        let in_state = self.state_entered.elapsed();
        let mut transitions = Vec::new();

        loop {
            let timeout = self.timeout_for_current();
            // Duration::MAX means "never transition".
            if timeout == Duration::MAX {
                break;
            }
            let threshold = if self.state == IdleState::Active {
                // From active, compare against total idle time.
                idle_time >= timeout
            } else {
                // From other states, compare time-in-state.
                // But also check total idle time against cumulative threshold.
                let cumulative = self.policy.time_to_state(self.next_state());
                idle_time >= cumulative && in_state >= timeout
            };

            if threshold {
                let prev = self.state;
                let next = self.next_state();
                if next == prev {
                    break; // No further states.
                }
                self.state = next;
                self.state_entered = Instant::now();
                self.notify(prev, next);
                transitions.push((prev, next));
                if next == IdleState::Suspend {
                    break; // Final state.
                }
            } else {
                break;
            }
        }
        transitions
    }

    /// Register a callback for state transitions.
    pub fn on_transition(&mut self, cb: IdleTransitionCallback) {
        self.callbacks.push(cb);
    }

    /// Update the idle policy. Does not reset the current state.
    pub fn set_policy(&mut self, policy: IdlePolicy) {
        self.policy = policy;
    }

    /// Returns a reference to the current policy.
    pub fn policy(&self) -> &IdlePolicy {
        &self.policy
    }

    // -- Internal helpers --

    fn timeout_for_current(&self) -> Duration {
        match self.state {
            IdleState::Active => self.policy.idle_timeout,
            IdleState::Idle => self.policy.dim_timeout,
            IdleState::DimDisplay => self.policy.screen_off_timeout,
            IdleState::ScreenOff => self.policy.suspend_timeout,
            IdleState::Suspend => Duration::MAX, // terminal
        }
    }

    fn next_state(&self) -> IdleState {
        match self.state {
            IdleState::Active => IdleState::Idle,
            IdleState::Idle => IdleState::DimDisplay,
            IdleState::DimDisplay => IdleState::ScreenOff,
            IdleState::ScreenOff => IdleState::Suspend,
            IdleState::Suspend => IdleState::Suspend,
        }
    }

    fn notify(&self, from: IdleState, to: IdleState) {
        for cb in &self.callbacks {
            cb(from, to);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn initial_state_is_active() {
        let tracker = IdleTracker::new(IdlePolicy::desktop());
        assert_eq!(tracker.state(), IdleState::Active);
    }

    #[test]
    fn idle_duration_increases() {
        let tracker = IdleTracker::new(IdlePolicy::desktop());
        std::thread::sleep(Duration::from_millis(10));
        assert!(tracker.idle_duration() >= Duration::from_millis(10));
    }

    #[test]
    fn reset_returns_to_active() {
        // Use zero-timeout policy so we can transition immediately.
        let mut tracker = IdleTracker::new(IdlePolicy {
            idle_timeout: Duration::ZERO,
            dim_timeout: Duration::MAX,
            screen_off_timeout: Duration::MAX,
            suspend_timeout: Duration::MAX,
        });
        // Let it go idle.
        tracker.tick();
        assert_eq!(tracker.state(), IdleState::Idle);

        // Reset.
        let prev = tracker.reset();
        assert_eq!(prev, Some(IdleState::Idle));
        assert_eq!(tracker.state(), IdleState::Active);
    }

    #[test]
    fn reset_from_active_returns_none() {
        let mut tracker = IdleTracker::new(IdlePolicy::desktop());
        assert_eq!(tracker.reset(), None);
    }

    #[test]
    fn zero_timeout_transitions_immediately() {
        let mut tracker = IdleTracker::new(IdlePolicy {
            idle_timeout: Duration::ZERO,
            dim_timeout: Duration::ZERO,
            screen_off_timeout: Duration::ZERO,
            suspend_timeout: Duration::ZERO,
        });
        let transitions = tracker.tick();
        // Should have walked through all states.
        assert!(!transitions.is_empty());
        assert_eq!(tracker.state(), IdleState::Suspend);
    }

    #[test]
    fn max_timeout_never_transitions() {
        let mut tracker = IdleTracker::new(IdlePolicy::presentation());
        let transitions = tracker.tick();
        assert!(transitions.is_empty());
        assert_eq!(tracker.state(), IdleState::Active);
    }

    #[test]
    fn callback_fires_on_transition() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter2 = counter.clone();

        let mut tracker = IdleTracker::new(IdlePolicy {
            idle_timeout: Duration::ZERO,
            dim_timeout: Duration::MAX,
            screen_off_timeout: Duration::MAX,
            suspend_timeout: Duration::MAX,
        });
        tracker.on_transition(Box::new(move |_from, _to| {
            counter2.fetch_add(1, Ordering::SeqCst);
        }));
        tracker.tick();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn set_policy_takes_effect() {
        let mut tracker = IdleTracker::new(IdlePolicy::desktop());
        tracker.set_policy(IdlePolicy::battery_saver());
        assert_eq!(tracker.policy().idle_timeout, Duration::from_secs(60));
    }

    #[test]
    fn desktop_policy_defaults() {
        let p = IdlePolicy::desktop();
        assert_eq!(p.idle_timeout, Duration::from_secs(300));
        assert_eq!(p.dim_timeout, Duration::from_secs(30));
        assert_eq!(p.screen_off_timeout, Duration::from_secs(120));
        assert_eq!(p.suspend_timeout, Duration::from_secs(600));
    }

    #[test]
    fn battery_saver_policy_defaults() {
        let p = IdlePolicy::battery_saver();
        assert_eq!(p.idle_timeout, Duration::from_secs(60));
        assert_eq!(p.dim_timeout, Duration::from_secs(15));
    }

    #[test]
    fn time_to_state() {
        let p = IdlePolicy::desktop();
        assert_eq!(p.time_to_state(IdleState::Active), Duration::ZERO);
        assert_eq!(p.time_to_state(IdleState::Idle), Duration::from_secs(300));
        assert_eq!(
            p.time_to_state(IdleState::DimDisplay),
            Duration::from_secs(330)
        );
    }

    #[test]
    fn idle_state_display() {
        assert_eq!(IdleState::Active.to_string(), "active");
        assert_eq!(IdleState::Idle.to_string(), "idle");
        assert_eq!(IdleState::DimDisplay.to_string(), "dim-display");
        assert_eq!(IdleState::ScreenOff.to_string(), "screen-off");
        assert_eq!(IdleState::Suspend.to_string(), "suspend");
    }

    #[test]
    fn idle_state_ordering() {
        assert!(IdleState::Active < IdleState::Idle);
        assert!(IdleState::Idle < IdleState::DimDisplay);
        assert!(IdleState::DimDisplay < IdleState::ScreenOff);
        assert!(IdleState::ScreenOff < IdleState::Suspend);
    }

    #[test]
    fn time_in_state_increases() {
        let tracker = IdleTracker::new(IdlePolicy::desktop());
        std::thread::sleep(Duration::from_millis(10));
        assert!(tracker.time_in_state() >= Duration::from_millis(10));
    }
}
