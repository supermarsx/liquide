//! Bandwidth estimation and budgeting for adaptive encoding.
//!
//! The `BandwidthEstimator` maintains a sliding window of recent frame sizes
//! and round-trip times to estimate available bandwidth. The `BandwidthBudget`
//! converts this estimate into a per-frame byte budget that the encoder uses
//! to decide whether to degrade quality or switch compression methods.

use std::collections::VecDeque;

/// Current pressure state for per-frame bandwidth budgeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetPressure {
    /// No useful estimate exists yet, so callers should avoid degrading.
    Warmup,
    /// Recent traffic is within the current per-frame budget.
    Nominal,
    /// Recent traffic exceeded the current per-frame budget.
    Pressured,
}

impl BudgetPressure {
    /// Whether this state should activate budget-aware fallback paths.
    #[must_use]
    pub fn is_pressured(self) -> bool {
        matches!(self, Self::Pressured)
    }
}

/// Sliding-window bandwidth estimator.
///
/// Tracks frame sizes and optional RTT measurements over a configurable
/// window to produce a smoothed bandwidth estimate in bytes per second.
pub struct BandwidthEstimator {
    /// Recent frame sizes in bytes.
    frame_sizes: VecDeque<u64>,
    /// Recent RTT samples in microseconds.
    rtt_samples: VecDeque<u64>,
    /// Maximum number of samples in each sliding window.
    window_size: usize,
    /// Frame interval in microseconds (e.g., 16667 for 60 fps).
    frame_interval_us: u64,
    /// Exponential moving average smoothing factor (0.0–1.0).
    /// Higher values weight recent samples more heavily.
    alpha: f64,
    /// Current smoothed bandwidth estimate (bytes per second).
    estimated_bps: f64,
    /// Current smoothed RTT estimate (microseconds).
    estimated_rtt_us: f64,
}

impl BandwidthEstimator {
    /// Create a new estimator with the given window size and frame rate.
    #[must_use]
    pub fn new(window_size: usize, target_fps: u32) -> Self {
        let frame_interval_us = if target_fps > 0 {
            1_000_000 / target_fps as u64
        } else {
            16_667
        };
        Self {
            frame_sizes: VecDeque::with_capacity(window_size),
            rtt_samples: VecDeque::with_capacity(window_size),
            window_size,
            frame_interval_us,
            alpha: 0.3,
            estimated_bps: 0.0,
            estimated_rtt_us: 0.0,
        }
    }

    /// Record a frame's compressed size in bytes.
    pub fn record_frame(&mut self, compressed_bytes: u64) {
        if self.frame_sizes.len() >= self.window_size {
            self.frame_sizes.pop_front();
        }
        self.frame_sizes.push_back(compressed_bytes);

        // Update bandwidth EMA: bytes_per_frame * frames_per_second
        let bytes_per_frame = compressed_bytes as f64;
        let fps = 1_000_000.0 / self.frame_interval_us as f64;
        let instantaneous_bps = bytes_per_frame * fps;

        if self.estimated_bps == 0.0 {
            self.estimated_bps = instantaneous_bps;
        } else {
            self.estimated_bps =
                self.alpha * instantaneous_bps + (1.0 - self.alpha) * self.estimated_bps;
        }
    }

    /// Record a round-trip time measurement in microseconds.
    pub fn record_rtt(&mut self, rtt_us: u64) {
        if self.rtt_samples.len() >= self.window_size {
            self.rtt_samples.pop_front();
        }
        self.rtt_samples.push_back(rtt_us);

        let rtt = rtt_us as f64;
        if self.estimated_rtt_us == 0.0 {
            self.estimated_rtt_us = rtt;
        } else {
            self.estimated_rtt_us = self.alpha * rtt + (1.0 - self.alpha) * self.estimated_rtt_us;
        }
    }

    /// Get the current smoothed bandwidth estimate in bytes per second.
    #[must_use]
    pub fn estimated_bandwidth_bps(&self) -> f64 {
        self.estimated_bps
    }

    /// Get the current smoothed RTT estimate in microseconds.
    #[must_use]
    pub fn estimated_rtt_us(&self) -> f64 {
        self.estimated_rtt_us
    }

    /// Frame interval implied by the estimator's target frame rate.
    #[must_use]
    pub fn frame_interval_us(&self) -> u64 {
        self.frame_interval_us
    }

    /// Whether the estimator has at least one recorded frame sample.
    #[must_use]
    pub fn has_samples(&self) -> bool {
        !self.frame_sizes.is_empty()
    }

    /// Average frame size over the sliding window.
    #[must_use]
    pub fn average_frame_size(&self) -> f64 {
        if self.frame_sizes.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.frame_sizes.iter().sum();
        sum as f64 / self.frame_sizes.len() as f64
    }

    /// Peak frame size in the sliding window.
    #[must_use]
    pub fn peak_frame_size(&self) -> u64 {
        self.frame_sizes.iter().copied().max().unwrap_or(0)
    }

    /// Number of recorded frame samples.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.frame_sizes.len()
    }

    /// Seed the estimator with a repeated frame-size sample.
    pub fn warm_up(&mut self, compressed_bytes: u64, sample_count: usize) {
        if sample_count == 0 {
            return;
        }

        for _ in 0..sample_count {
            self.record_frame(compressed_bytes);
        }
    }

    /// Derive a per-frame budget from the estimator's current state.
    #[must_use]
    pub fn frame_budget(&self, safety_margin: f64) -> BandwidthBudget {
        BandwidthBudget::from_estimator(self, safety_margin)
    }

    /// Set the EMA smoothing factor.
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha.clamp(0.01, 1.0);
    }

    /// Reset the estimator, clearing all recorded history and estimates.
    pub fn reset(&mut self) {
        self.frame_sizes.clear();
        self.rtt_samples.clear();
        self.estimated_bps = 0.0;
        self.estimated_rtt_us = 0.0;
    }
}

/// Per-frame byte budget derived from bandwidth estimates.
pub struct BandwidthBudget {
    /// Target bytes per frame at the estimated bandwidth.
    budget_bytes: u64,
    /// Safety margin (0.0–1.0) — fraction of budget to reserve.
    safety_margin: f64,
    /// Current pressure state derived from observed frame sizes.
    pressure: BudgetPressure,
}

impl BandwidthBudget {
    /// Compute a frame budget from the estimator's current state.
    #[must_use]
    pub fn from_estimator(estimator: &BandwidthEstimator, safety_margin: f64) -> Self {
        let margin = safety_margin.clamp(0.0, 0.5);
        if !estimator.has_samples() || estimator.estimated_bandwidth_bps() <= 0.0 {
            return Self {
                budget_bytes: u64::MAX,
                safety_margin: margin,
                pressure: BudgetPressure::Warmup,
            };
        }

        let bps = estimator.estimated_bandwidth_bps();
        let frame_interval_s = estimator.frame_interval_us() as f64 / 1_000_000.0;
        let raw_budget = bps * frame_interval_s;
        let budget_bytes = (raw_budget * (1.0 - margin)).max(0.0) as u64;

        Self {
            budget_bytes,
            safety_margin: margin,
            pressure: BudgetPressure::Nominal,
        }
    }

    /// Create a budget with finite bytes.
    #[must_use]
    pub fn new(budget_bytes: u64, safety_margin: f64) -> Self {
        Self {
            budget_bytes,
            safety_margin: safety_margin.clamp(0.0, 0.5),
            pressure: BudgetPressure::Nominal,
        }
    }

    /// Create an unlimited budget (no degradation).
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            budget_bytes: u64::MAX,
            safety_margin: 0.0,
            pressure: BudgetPressure::Nominal,
        }
    }

    /// Refresh the budget bytes from a newer estimator snapshot.
    pub fn refresh_from_estimator(&mut self, estimator: &BandwidthEstimator) {
        let next = Self::from_estimator(estimator, self.safety_margin);
        self.budget_bytes = next.budget_bytes;
        self.pressure = next.pressure;
    }

    /// Target bytes per frame.
    #[must_use]
    pub fn budget_bytes(&self) -> u64 {
        self.budget_bytes
    }

    /// The configured safety margin.
    #[must_use]
    pub fn safety_margin(&self) -> f64 {
        self.safety_margin
    }

    /// Whether this budget disables pressure-based degradation entirely.
    #[must_use]
    pub fn is_unlimited(&self) -> bool {
        self.budget_bytes == u64::MAX
    }

    /// Current pressure state for this budget.
    #[must_use]
    pub fn pressure(&self) -> BudgetPressure {
        self.pressure
    }

    /// Whether the budget currently recommends pressure-aware degradation.
    #[must_use]
    pub fn under_pressure(&self) -> bool {
        self.pressure.is_pressured()
    }

    /// Compute the pressure state implied by an observed frame size.
    #[must_use]
    pub fn pressure_for(&self, batch_compressed_bytes: u64) -> BudgetPressure {
        if self.is_unlimited() {
            return BudgetPressure::Nominal;
        }

        if batch_compressed_bytes > self.budget_bytes {
            BudgetPressure::Pressured
        } else {
            BudgetPressure::Nominal
        }
    }

    /// Update the current pressure state from an observed frame size.
    pub fn observe(&mut self, batch_compressed_bytes: u64) -> BudgetPressure {
        let pressure = self.pressure_for(batch_compressed_bytes);
        self.pressure = pressure;
        pressure
    }

    /// Check if a batch size exceeds the budget and degradation is needed.
    #[must_use]
    pub fn should_degrade(&self, batch_compressed_bytes: u64) -> bool {
        self.pressure_for(batch_compressed_bytes).is_pressured()
    }

    /// Fraction of the budget used by this batch (> 1.0 means over budget).
    #[must_use]
    pub fn utilization(&self, batch_compressed_bytes: u64) -> f64 {
        if self.budget_bytes == 0 || self.is_unlimited() {
            return 0.0;
        }
        batch_compressed_bytes as f64 / self.budget_bytes as f64
    }
}
