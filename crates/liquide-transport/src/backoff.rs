//! Exponential back-off with jitter for reconnection attempts.

use std::time::Duration;

/// Exponential back-off configuration.
#[derive(Debug, Clone)]
pub struct Backoff {
    /// Minimum delay between attempts.
    min: Duration,
    /// Maximum delay (cap).
    max: Duration,
    /// Multiplicative factor per attempt.
    factor: f64,
    /// Current attempt number (0-indexed).
    attempt: u32,
}

impl Backoff {
    /// Create a new back-off with the given bounds.
    #[must_use]
    pub fn new(min: Duration, max: Duration) -> Self {
        Self {
            min,
            max,
            factor: 2.0,
            attempt: 0,
        }
    }

    /// Set the multiplicative factor (default 2.0).
    #[must_use]
    pub fn with_factor(mut self, factor: f64) -> Self {
        self.factor = factor;
        self
    }

    /// Return the delay for the current attempt and advance the counter.
    pub fn next_delay(&mut self) -> Duration {
        let base = self.min.as_secs_f64() * self.factor.powi(self.attempt as i32);
        let capped = base.min(self.max.as_secs_f64());
        // Add ±25 % jitter using a simple deterministic hash of the attempt.
        let jitter_frac = Self::jitter_fraction(self.attempt);
        let jittered = capped * (0.75 + 0.5 * jitter_frac);
        let jittered = jittered.max(self.min.as_secs_f64());
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_secs_f64(jittered)
    }

    /// Peek at the next delay without advancing the counter.
    #[must_use]
    pub fn peek_delay(&self) -> Duration {
        let base = self.min.as_secs_f64() * self.factor.powi(self.attempt as i32);
        let capped = base.min(self.max.as_secs_f64());
        let jitter_frac = Self::jitter_fraction(self.attempt);
        let jittered = capped * (0.75 + 0.5 * jitter_frac);
        let jittered = jittered.max(self.min.as_secs_f64());
        Duration::from_secs_f64(jittered)
    }

    /// Reset the back-off counter (e.g. after a successful connection).
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Current attempt number.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Deterministic jitter in [0, 1) derived from the attempt number.
    fn jitter_fraction(attempt: u32) -> f64 {
        // Simple hash: multiply by a large prime, take fractional part.
        let h = (attempt as u64).wrapping_mul(2_654_435_761);
        (h % 1_000_000) as f64 / 1_000_000.0
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(Duration::from_millis(100), Duration::from_secs(30))
    }
}
