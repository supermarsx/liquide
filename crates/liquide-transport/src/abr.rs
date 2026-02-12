//! Adaptive Bitrate (ABR) control loop.
//!
//! Runs on a 100 ms tick.  Monitors transport metrics (RTT, loss, cwnd
//! occupancy, jitter) and dynamically adjusts quality parameters: video
//! FPS cap, quality index, keyframe interval, and per-channel bandwidth
//! budgets.

use std::collections::VecDeque;
use std::time::Duration;

// ---------------------------------------------------------------------------
// ABR Metrics
// ---------------------------------------------------------------------------

/// Input metrics for an ABR tick.
#[derive(Debug, Clone, Copy)]
pub struct AbrMetrics {
    /// Smoothed round-trip time.
    pub srtt: Duration,
    /// Current loss rate (0.0–1.0).
    pub loss_rate: f64,
    /// Congestion window occupancy (bytes_in_flight / cwnd).
    pub cwnd_occupancy: f64,
    /// CPU utilization (0.0–1.0), if available.
    pub cpu_util: f64,
    /// Decoder latency.
    pub decode_latency: Duration,
    /// Send queue depth in bytes.
    pub queue_depth: u64,
    /// Jitter estimate.
    pub jitter: Duration,
}

impl Default for AbrMetrics {
    fn default() -> Self {
        Self {
            srtt: Duration::from_millis(50),
            loss_rate: 0.0,
            cwnd_occupancy: 0.0,
            cpu_util: 0.0,
            decode_latency: Duration::ZERO,
            queue_depth: 0,
            jitter: Duration::ZERO,
        }
    }
}

// ---------------------------------------------------------------------------
// ABR Decision
// ---------------------------------------------------------------------------

/// Output of an ABR tick: quality parameters.
#[derive(Debug, Clone, Copy)]
pub struct AbrDecision {
    /// Video FPS cap (1–60).
    pub video_fps_cap: u32,
    /// Quality index (0 = best, 51 = worst; H.264/H.265 CRF scale).
    pub quality_index: u32,
    /// Keyframe interval in seconds (2–10).
    pub keyframe_interval_secs: u32,
    /// Tile compression level (1 = least, 6 = most).
    pub tile_compression_level: u32,
    /// Estimated available bandwidth budget in bytes/sec.
    pub bandwidth_budget: u64,
}

impl Default for AbrDecision {
    fn default() -> Self {
        Self {
            video_fps_cap: 60,
            quality_index: 20,
            keyframe_interval_secs: 5,
            tile_compression_level: 3,
            bandwidth_budget: 10_000_000, // 10 MB/s default
        }
    }
}

// ---------------------------------------------------------------------------
// ABR Config
// ---------------------------------------------------------------------------

/// Configuration for the ABR controller.
#[derive(Debug, Clone)]
pub struct AbrConfig {
    /// Tick interval.
    pub tick_interval: Duration,
    /// Number of ticks of stability before upgrading quality.
    pub upgrade_stability_ticks: usize,
    /// Loss rate threshold for quality downgrade.
    pub loss_downgrade_threshold: f64,
    /// RTT threshold for quality downgrade.
    pub rtt_downgrade_threshold: Duration,
    /// Cwnd occupancy threshold for quality downgrade.
    pub cwnd_downgrade_threshold: f64,
    /// History size for metric averaging.
    pub history_size: usize,
}

impl Default for AbrConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(100),
            upgrade_stability_ticks: 5,
            loss_downgrade_threshold: 0.02,
            rtt_downgrade_threshold: Duration::from_millis(200),
            cwnd_downgrade_threshold: 0.90,
            history_size: 20,
        }
    }
}

// ---------------------------------------------------------------------------
// ABR Callback
// ---------------------------------------------------------------------------

/// Callback interface for encoder integration.
pub trait AbrCallback: Send + Sync {
    /// Called when the target bitrate changes.
    fn on_bitrate_change(&self, budget_bytes_per_sec: u64);

    /// Called when the FPS cap changes.
    fn on_fps_change(&self, fps: u32);

    /// Called when the quality index changes.
    fn on_quality_change(&self, quality_index: u32);
}

// ---------------------------------------------------------------------------
// ABR Controller
// ---------------------------------------------------------------------------

/// Adaptive bitrate controller with quality ladder logic.
#[derive(Debug)]
pub struct AbrController {
    config: AbrConfig,
    decision: AbrDecision,
    history: VecDeque<AbrMetrics>,
    /// Number of consecutive stable ticks.
    stable_ticks: usize,
}

impl AbrController {
    /// Create with the given config.
    #[must_use]
    pub fn new(config: AbrConfig) -> Self {
        Self {
            config,
            decision: AbrDecision::default(),
            history: VecDeque::new(),
            stable_ticks: 0,
        }
    }

    /// Create with default config.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(AbrConfig::default())
    }

    /// Current decision.
    #[must_use]
    pub fn decision(&self) -> &AbrDecision {
        &self.decision
    }

    /// Number of consecutive stable ticks.
    #[must_use]
    pub fn stable_ticks(&self) -> usize {
        self.stable_ticks
    }

    /// Process one tick with the given metrics.  Returns the updated decision.
    pub fn tick(&mut self, metrics: AbrMetrics) -> AbrDecision {
        // Record history
        self.history.push_back(metrics);
        if self.history.len() > self.config.history_size {
            self.history.pop_front();
        }

        let degraded = self.is_degraded(&metrics);

        if degraded {
            self.stable_ticks = 0;
            self.downgrade();
        } else {
            self.stable_ticks += 1;
            if self.stable_ticks >= self.config.upgrade_stability_ticks {
                self.upgrade();
                self.stable_ticks = 0;
            }
        }

        self.decision
    }

    /// Check if current metrics indicate degraded conditions.
    fn is_degraded(&self, m: &AbrMetrics) -> bool {
        m.loss_rate > self.config.loss_downgrade_threshold
            || m.srtt > self.config.rtt_downgrade_threshold
            || m.cwnd_occupancy > self.config.cwnd_downgrade_threshold
    }

    /// Reduce quality one step (immediate on degradation).
    fn downgrade(&mut self) {
        // Increase quality_index (worse quality) — clamp at 51
        if self.decision.quality_index < 51 {
            self.decision.quality_index =
                (self.decision.quality_index + 3).min(51);
        }

        // Reduce FPS cap — clamp at 1
        if self.decision.video_fps_cap > 15 {
            self.decision.video_fps_cap =
                self.decision.video_fps_cap.saturating_sub(5).max(1);
        }

        // Increase tile compression — clamp at 6
        if self.decision.tile_compression_level < 6 {
            self.decision.tile_compression_level += 1;
        }

        // Reduce bandwidth budget by 20%
        self.decision.bandwidth_budget =
            (self.decision.bandwidth_budget as f64 * 0.80) as u64;
    }

    /// Improve quality one step (only after sustained stability).
    fn upgrade(&mut self) {
        // Decrease quality_index (better quality) — clamp at 0
        if self.decision.quality_index > 0 {
            self.decision.quality_index =
                self.decision.quality_index.saturating_sub(2);
        }

        // Increase FPS cap — clamp at 60
        if self.decision.video_fps_cap < 60 {
            self.decision.video_fps_cap =
                (self.decision.video_fps_cap + 5).min(60);
        }

        // Decrease tile compression — clamp at 1
        if self.decision.tile_compression_level > 1 {
            self.decision.tile_compression_level -= 1;
        }

        // Increase bandwidth budget by 10%
        self.decision.bandwidth_budget =
            (self.decision.bandwidth_budget as f64 * 1.10) as u64;
    }

    /// Reset to default quality settings.
    pub fn reset(&mut self) {
        self.decision = AbrDecision::default();
        self.history.clear();
        self.stable_ticks = 0;
    }
}
