/// Idle detection for the lock screen.
///
/// Tracks user activity and emits events when the user has been idle
/// for configurable durations (dim, blank, lock).

/// Configuration for idle timeouts.
#[derive(Debug, Clone)]
pub struct IdleConfig {
    /// Milliseconds of inactivity before dimming the screen.
    pub dim_timeout_ms: u64,
    /// Milliseconds of inactivity before blanking the screen.
    pub blank_timeout_ms: u64,
    /// Milliseconds of inactivity before locking the session.
    pub lock_timeout_ms: u64,
    /// Whether to lock automatically when blanking.
    pub lock_on_blank: bool,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            dim_timeout_ms: 180_000,  // 3 minutes
            blank_timeout_ms: 300_000, // 5 minutes
            lock_timeout_ms: 600_000,  // 10 minutes
            lock_on_blank: false,
        }
    }
}

/// Current idle state progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleState {
    /// User is actively using the system.
    Active,
    /// Screen is dimming (user idle for dim_timeout_ms).
    Dimming,
    /// Screen is blanked (user idle for blank_timeout_ms).
    Blanked,
    /// Session is locked (user idle for lock_timeout_ms).
    Locked,
}

/// Events emitted when idle state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEvent {
    /// Start dimming the display.
    DimScreen,
    /// Blank the display entirely.
    BlankScreen,
    /// Lock the session.
    LockScreen,
    /// User returned from idle — wake up.
    Wake,
}

/// Tracks user activity and determines idle state transitions.
pub struct IdleDetector {
    config: IdleConfig,
    last_activity_ms: u64,
    state: IdleState,
}

impl IdleDetector {
    /// Create a new idle detector with the given timeout configuration.
    pub fn new(config: IdleConfig) -> Self {
        Self {
            config,
            last_activity_ms: 0,
            state: IdleState::Active,
        }
    }

    /// Create a detector with a simple single timeout (all thresholds equal).
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self::new(IdleConfig {
            dim_timeout_ms: timeout_ms,
            blank_timeout_ms: timeout_ms,
            lock_timeout_ms: timeout_ms,
            lock_on_blank: false,
        })
    }

    /// Report user activity, resetting the idle timer.
    pub fn report_activity(&mut self, now_ms: u64) -> Option<IdleEvent> {
        self.last_activity_ms = now_ms;
        let was_idle = self.state != IdleState::Active;
        self.state = IdleState::Active;
        if was_idle {
            Some(IdleEvent::Wake)
        } else {
            None
        }
    }

    /// Advance the idle detector by the current timestamp.
    /// Returns an event if a state transition occurred.
    pub fn tick(&mut self, now_ms: u64) -> Option<IdleEvent> {
        let idle_ms = now_ms.saturating_sub(self.last_activity_ms);

        match self.state {
            IdleState::Active => {
                if idle_ms >= self.config.lock_timeout_ms {
                    self.state = IdleState::Locked;
                    Some(IdleEvent::LockScreen)
                } else if idle_ms >= self.config.blank_timeout_ms {
                    self.state = IdleState::Blanked;
                    if self.config.lock_on_blank {
                        self.state = IdleState::Locked;
                        Some(IdleEvent::LockScreen)
                    } else {
                        Some(IdleEvent::BlankScreen)
                    }
                } else if idle_ms >= self.config.dim_timeout_ms {
                    self.state = IdleState::Dimming;
                    Some(IdleEvent::DimScreen)
                } else {
                    None
                }
            }
            IdleState::Dimming => {
                if idle_ms >= self.config.lock_timeout_ms {
                    self.state = IdleState::Locked;
                    Some(IdleEvent::LockScreen)
                } else if idle_ms >= self.config.blank_timeout_ms {
                    self.state = IdleState::Blanked;
                    if self.config.lock_on_blank {
                        self.state = IdleState::Locked;
                        Some(IdleEvent::LockScreen)
                    } else {
                        Some(IdleEvent::BlankScreen)
                    }
                } else {
                    None
                }
            }
            IdleState::Blanked => {
                if idle_ms >= self.config.lock_timeout_ms {
                    self.state = IdleState::Locked;
                    Some(IdleEvent::LockScreen)
                } else {
                    None
                }
            }
            IdleState::Locked => None,
        }
    }

    /// Update the idle configuration.
    pub fn set_config(&mut self, config: IdleConfig) {
        self.config = config;
    }

    /// Set the dim timeout.
    pub fn set_timeout(&mut self, ms: u64) {
        self.config.dim_timeout_ms = ms;
    }

    /// Whether the user is currently idle (not Active).
    pub fn is_idle(&self) -> bool {
        self.state != IdleState::Active
    }

    /// How long the user has been idle, in milliseconds.
    pub fn idle_duration_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.last_activity_ms)
    }

    /// Current idle state.
    pub fn state(&self) -> IdleState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> IdleConfig {
        IdleConfig {
            dim_timeout_ms: 1000,
            blank_timeout_ms: 2000,
            lock_timeout_ms: 3000,
            lock_on_blank: false,
        }
    }

    #[test]
    fn initial_state_is_active() {
        let det = IdleDetector::new(default_config());
        assert_eq!(det.state(), IdleState::Active);
        assert!(!det.is_idle());
    }

    #[test]
    fn no_event_before_timeout() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        assert_eq!(det.tick(500), None);
        assert_eq!(det.state(), IdleState::Active);
    }

    #[test]
    fn dim_after_dim_timeout() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        assert_eq!(det.tick(1000), Some(IdleEvent::DimScreen));
        assert_eq!(det.state(), IdleState::Dimming);
        assert!(det.is_idle());
    }

    #[test]
    fn blank_after_blank_timeout() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(1000); // dim
        assert_eq!(det.tick(2000), Some(IdleEvent::BlankScreen));
        assert_eq!(det.state(), IdleState::Blanked);
    }

    #[test]
    fn lock_after_lock_timeout() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(1000); // dim
        det.tick(2000); // blank
        assert_eq!(det.tick(3000), Some(IdleEvent::LockScreen));
        assert_eq!(det.state(), IdleState::Locked);
    }

    #[test]
    fn activity_resets_timer() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(999); // almost dim
        det.report_activity(999); // reset
        assert_eq!(det.tick(1500), None); // only 501ms idle
        assert_eq!(det.state(), IdleState::Active);
    }

    #[test]
    fn wake_from_dimming() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(1000); // dim
        assert_eq!(det.state(), IdleState::Dimming);
        let ev = det.report_activity(1500);
        assert_eq!(ev, Some(IdleEvent::Wake));
        assert_eq!(det.state(), IdleState::Active);
    }

    #[test]
    fn wake_from_blanked() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(1000);
        det.tick(2000);
        assert_eq!(det.state(), IdleState::Blanked);
        let ev = det.report_activity(2500);
        assert_eq!(ev, Some(IdleEvent::Wake));
        assert_eq!(det.state(), IdleState::Active);
    }

    #[test]
    fn wake_from_locked() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(1000);
        det.tick(2000);
        det.tick(3000);
        assert_eq!(det.state(), IdleState::Locked);
        let ev = det.report_activity(4000);
        assert_eq!(ev, Some(IdleEvent::Wake));
        assert_eq!(det.state(), IdleState::Active);
    }

    #[test]
    fn no_wake_when_already_active() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        let ev = det.report_activity(100);
        assert_eq!(ev, None);
    }

    #[test]
    fn lock_on_blank_skips_blank_state() {
        let mut det = IdleDetector::new(IdleConfig {
            dim_timeout_ms: 1000,
            blank_timeout_ms: 2000,
            lock_timeout_ms: 3000,
            lock_on_blank: true,
        });
        det.report_activity(0);
        det.tick(1000); // dim
        let ev = det.tick(2000); // blank -> lock because lock_on_blank
        assert_eq!(ev, Some(IdleEvent::LockScreen));
        assert_eq!(det.state(), IdleState::Locked);
    }

    #[test]
    fn idle_duration_increases() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(100);
        assert_eq!(det.idle_duration_ms(100), 0);
        assert_eq!(det.idle_duration_ms(600), 500);
        assert_eq!(det.idle_duration_ms(1100), 1000);
    }

    #[test]
    fn set_timeout_changes_dim() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.set_timeout(500);
        assert_eq!(det.tick(500), Some(IdleEvent::DimScreen));
    }

    #[test]
    fn with_timeout_sets_all_equal() {
        let mut det = IdleDetector::with_timeout(5000);
        det.report_activity(0);
        assert_eq!(det.tick(4999), None);
        // At 5000 all three thresholds are hit; since lock >= blank >= dim,
        // and we check lock first in Active, lock_timeout_ms == 5000 triggers.
        assert_eq!(det.tick(5000), Some(IdleEvent::LockScreen));
    }

    #[test]
    fn locked_state_emits_nothing_on_further_ticks() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(1000);
        det.tick(2000);
        det.tick(3000); // locked
        assert_eq!(det.tick(4000), None);
        assert_eq!(det.tick(100_000), None);
    }

    #[test]
    fn dim_blank_lock_full_progression() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);

        assert_eq!(det.tick(999), None);
        assert_eq!(det.tick(1000), Some(IdleEvent::DimScreen));
        assert_eq!(det.state(), IdleState::Dimming);

        assert_eq!(det.tick(1999), None);
        assert_eq!(det.tick(2000), Some(IdleEvent::BlankScreen));
        assert_eq!(det.state(), IdleState::Blanked);

        assert_eq!(det.tick(2999), None);
        assert_eq!(det.tick(3000), Some(IdleEvent::LockScreen));
        assert_eq!(det.state(), IdleState::Locked);
    }

    #[test]
    fn activity_after_lock_then_re_idle() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.tick(1000);
        det.tick(2000);
        det.tick(3000); // locked

        det.report_activity(4000); // wake
        assert_eq!(det.state(), IdleState::Active);

        // Should dim again after another dim timeout
        assert_eq!(det.tick(5000), Some(IdleEvent::DimScreen));
    }

    #[test]
    fn skip_directly_to_lock_if_timeouts_equal() {
        let mut det = IdleDetector::new(IdleConfig {
            dim_timeout_ms: 500,
            blank_timeout_ms: 500,
            lock_timeout_ms: 500,
            lock_on_blank: false,
        });
        det.report_activity(0);
        // All three thresholds crossed at once — lock wins (checked first)
        assert_eq!(det.tick(500), Some(IdleEvent::LockScreen));
    }

    #[test]
    fn default_config_values() {
        let cfg = IdleConfig::default();
        assert_eq!(cfg.dim_timeout_ms, 180_000);
        assert_eq!(cfg.blank_timeout_ms, 300_000);
        assert_eq!(cfg.lock_timeout_ms, 600_000);
        assert!(!cfg.lock_on_blank);
    }

    #[test]
    fn set_config_replaces_all() {
        let mut det = IdleDetector::new(default_config());
        det.report_activity(0);
        det.set_config(IdleConfig {
            dim_timeout_ms: 100,
            blank_timeout_ms: 200,
            lock_timeout_ms: 300,
            lock_on_blank: true,
        });
        assert_eq!(det.tick(100), Some(IdleEvent::DimScreen));
    }
}
