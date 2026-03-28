//! Per-application rate limiting for notification delivery.
//!
//! Uses a sliding-window algorithm: each application tracks the timestamps of
//! its recent notifications, and new ones are rejected if the window is full.

use std::collections::HashMap;
use std::collections::VecDeque;

/// Per-application sliding-window rate limiter.
pub struct RateLimiter {
    /// Maximum notifications allowed per 1-second window.
    max_per_second: u32,
    /// Per-app timestamp queues (timestamps in milliseconds).
    windows: HashMap<String, VecDeque<u64>>,
}

impl RateLimiter {
    /// Creates a new rate limiter with the given limit.
    pub fn new(max_per_second: u32) -> Self {
        Self {
            max_per_second,
            windows: HashMap::new(),
        }
    }

    /// Updates the maximum notifications per second.
    pub fn set_limit(&mut self, max_per_second: u32) {
        self.max_per_second = max_per_second;
    }

    /// Returns the current limit.
    pub fn limit(&self) -> u32 {
        self.max_per_second
    }

    /// Checks whether `app_name` is allowed to send a notification at time `now_ms`.
    ///
    /// Returns `true` if allowed (and records the timestamp), `false` if the
    /// rate limit has been exceeded.
    pub fn check(&mut self, app_name: &str, now_ms: u64) -> bool {
        let window = self.windows.entry(app_name.to_string()).or_default();

        // Evict timestamps older than 1 second.
        let cutoff = now_ms.saturating_sub(1000);
        while let Some(&oldest) = window.front() {
            if oldest < cutoff {
                window.pop_front();
            } else {
                break;
            }
        }

        if window.len() >= self.max_per_second as usize {
            return false;
        }

        window.push_back(now_ms);
        true
    }

    /// Resets the rate limiter state for all applications.
    pub fn reset(&mut self) {
        self.windows.clear();
    }

    /// Resets the rate limiter state for a specific application.
    pub fn reset_app(&mut self, app_name: &str) {
        self.windows.remove(app_name);
    }

    /// Returns the number of notifications recorded in the current window for an app.
    pub fn current_count(&self, app_name: &str) -> usize {
        self.windows.get(app_name).map_or(0, |w| w.len())
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(5)
    }
}
