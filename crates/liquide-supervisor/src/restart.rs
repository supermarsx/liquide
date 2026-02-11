//! Restart policy for crashed sessions.

/// Decision on how to handle a crashed session restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestartDecision {
    /// Restart the session immediately.
    RestartNow,
    /// Restart the session after a delay.
    RestartAfterDelay {
        /// Delay in milliseconds before restarting.
        delay_ms: u64,
    },
    /// Do not restart; enter failed state.
    EnterFailed {
        /// Reason the session cannot be restarted.
        reason: String,
    },
}

/// Restart policy configuration.
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    /// Maximum restart attempts within the window.
    pub max_restarts: u32,
    /// Window in seconds for counting restart attempts.
    pub window_sec: u64,
    /// Base backoff delay in milliseconds.
    pub backoff_base_ms: u64,
    /// Number of restarts after which safe mode is entered.
    pub safe_mode_threshold: u32,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 5,
            window_sec: 600,
            backoff_base_ms: 1000,
            safe_mode_threshold: 3,
        }
    }
}

impl RestartPolicy {
    /// Create a new restart policy.
    #[must_use]
    pub fn new(
        max_restarts: u32,
        window_sec: u64,
        backoff_base_ms: u64,
        safe_mode_threshold: u32,
    ) -> Self {
        Self {
            max_restarts,
            window_sec,
            backoff_base_ms,
            safe_mode_threshold,
        }
    }

    /// Evaluate whether a session should be restarted based on its restart count.
    #[must_use]
    pub fn evaluate(&self, restart_count: u32) -> RestartDecision {
        if restart_count >= self.max_restarts {
            return RestartDecision::EnterFailed {
                reason: format!(
                    "exceeded maximum restarts ({} of {})",
                    restart_count, self.max_restarts
                ),
            };
        }

        let delay = self.compute_delay(restart_count);
        if delay == 0 {
            RestartDecision::RestartNow
        } else {
            RestartDecision::RestartAfterDelay { delay_ms: delay }
        }
    }

    /// Compute the backoff delay for a given restart count.
    ///
    /// Uses exponential backoff: `base_ms * 2^(restart_count - 1)`.
    /// First restart has zero delay.
    #[must_use]
    pub fn compute_delay(&self, restart_count: u32) -> u64 {
        if restart_count == 0 {
            return 0;
        }
        self.backoff_base_ms * 2u64.saturating_pow(restart_count.saturating_sub(1))
    }

    /// Whether safe mode should be entered after this many restarts.
    #[must_use]
    pub fn should_enter_safe_mode(&self, restart_count: u32) -> bool {
        restart_count >= self.safe_mode_threshold
    }
}
