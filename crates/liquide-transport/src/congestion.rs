//! Congestion control: RTT estimation, loss detection, bandwidth measurement,
//! and BBRv2-style congestion window management.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// RTT Estimator
// ---------------------------------------------------------------------------

/// Smoothed RTT estimator using EWMA (RFC 6298 style).
#[derive(Debug, Clone)]
pub struct RttEstimator {
    /// Smoothed RTT (EWMA, alpha = 1/8).
    srtt: Option<Duration>,
    /// RTT variation (EWMA, beta = 1/4).
    rttvar: Option<Duration>,
    /// Minimum RTT observed.
    min_rtt: Option<Duration>,
    /// Latest raw sample.
    latest: Option<Duration>,
}

impl RttEstimator {
    /// Create a new estimator with no samples.
    #[must_use]
    pub fn new() -> Self {
        Self {
            srtt: None,
            rttvar: None,
            min_rtt: None,
            latest: None,
        }
    }

    /// Record a new RTT sample.
    pub fn update(&mut self, rtt: Duration) {
        self.latest = Some(rtt);
        self.min_rtt = Some(match self.min_rtt {
            Some(prev) => prev.min(rtt),
            None => rtt,
        });

        match self.srtt {
            None => {
                // First sample — RFC 6298 §2.2
                self.srtt = Some(rtt);
                self.rttvar = Some(rtt / 2);
            }
            Some(prev_srtt) => {
                // Subsequent samples
                let diff = if rtt > prev_srtt {
                    rtt - prev_srtt
                } else {
                    prev_srtt - rtt
                };
                let prev_var = self.rttvar.unwrap_or(Duration::ZERO);
                // rttvar = 3/4 * rttvar + 1/4 * |srtt - rtt|
                self.rttvar = Some(prev_var.mul_f64(0.75) + diff.mul_f64(0.25));
                // srtt = 7/8 * srtt + 1/8 * rtt
                self.srtt = Some(prev_srtt.mul_f64(0.875) + rtt.mul_f64(0.125));
            }
        }
    }

    /// Smoothed RTT, or `None` if no samples yet.
    #[must_use]
    pub fn srtt(&self) -> Option<Duration> {
        self.srtt
    }

    /// Minimum observed RTT.
    #[must_use]
    pub fn min_rtt(&self) -> Option<Duration> {
        self.min_rtt
    }

    /// Latest raw sample.
    #[must_use]
    pub fn latest_rtt(&self) -> Option<Duration> {
        self.latest
    }

    /// RTT variation.
    #[must_use]
    pub fn rttvar(&self) -> Option<Duration> {
        self.rttvar
    }

    /// Retransmission timeout: `srtt + max(4 * rttvar, 1ms)`, clamped to [200ms, 60s].
    #[must_use]
    pub fn rto(&self) -> Duration {
        match (self.srtt, self.rttvar) {
            (Some(srtt), Some(rttvar)) => {
                let rto = srtt + (rttvar * 4).max(Duration::from_millis(1));
                rto.clamp(Duration::from_millis(200), Duration::from_secs(60))
            }
            _ => Duration::from_secs(1), // Initial RTO per RFC 6298
        }
    }
}

impl Default for RttEstimator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Loss Detector
// ---------------------------------------------------------------------------

/// Sliding-window loss rate detector.
#[derive(Debug, Clone)]
pub struct LossDetector {
    /// Ring buffer of (acked, timestamp) for the sliding window.
    window: VecDeque<(bool, Instant)>,
    /// Window duration.
    window_duration: Duration,
    /// Running count of losses in the window.
    losses: u64,
    /// Running count of total events in the window.
    total: u64,
}

impl LossDetector {
    /// Create a new loss detector with the given window duration.
    #[must_use]
    pub fn new(window_duration: Duration) -> Self {
        Self {
            window: VecDeque::new(),
            window_duration,
            losses: 0,
            total: 0,
        }
    }

    /// Record a successfully acknowledged packet.
    pub fn on_ack(&mut self) {
        self.record(true);
    }

    /// Record a lost packet.
    pub fn on_loss(&mut self) {
        self.record(false);
    }

    fn record(&mut self, acked: bool) {
        let now = Instant::now();
        self.expire(now);
        self.window.push_back((acked, now));
        self.total += 1;
        if !acked {
            self.losses += 1;
        }
    }

    fn expire(&mut self, now: Instant) {
        while let Some(&(acked, ts)) = self.window.front() {
            if now.duration_since(ts) > self.window_duration {
                self.window.pop_front();
                self.total -= 1;
                if !acked {
                    self.losses -= 1;
                }
            } else {
                break;
            }
        }
    }

    /// Current loss rate (0.0–1.0).
    #[must_use]
    pub fn loss_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.losses as f64 / self.total as f64
        }
    }

    /// Total events in the current window.
    #[must_use]
    pub fn sample_count(&self) -> u64 {
        self.total
    }
}

// ---------------------------------------------------------------------------
// Bandwidth Estimator
// ---------------------------------------------------------------------------

/// Sliding-window bandwidth estimator.
#[derive(Debug, Clone)]
pub struct BandwidthEstimator {
    /// Ring buffer of (timestamp, cumulative_bytes_acked).
    samples: VecDeque<(Instant, u64)>,
    /// Window duration (default 2s per spec).
    window: Duration,
    /// Cumulative bytes acked.
    total_bytes: u64,
    /// Maximum delivery rate observed in current window.
    max_bw: f64,
}

impl BandwidthEstimator {
    /// Create with the given sliding-window duration.
    #[must_use]
    pub fn new(window: Duration) -> Self {
        Self {
            samples: VecDeque::new(),
            window,
            total_bytes: 0,
            max_bw: 0.0,
        }
    }

    /// Record acknowledged bytes.
    pub fn on_ack(&mut self, bytes: u64) {
        let now = Instant::now();
        self.total_bytes += bytes;
        self.samples.push_back((now, self.total_bytes));
        self.expire(now);
        self.recalculate(now);
    }

    fn expire(&mut self, now: Instant) {
        while let Some(&(ts, _)) = self.samples.front() {
            if now.duration_since(ts) > self.window {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    fn recalculate(&mut self, now: Instant) {
        if let Some(&(oldest_ts, oldest_bytes)) = self.samples.front() {
            let dt = now.duration_since(oldest_ts).as_secs_f64();
            if dt > 0.001 {
                let bw = (self.total_bytes - oldest_bytes) as f64 / dt;
                self.max_bw = self.max_bw.max(bw);
            }
        }
    }

    /// Estimated maximum bandwidth in bytes/sec.
    #[must_use]
    pub fn max_bandwidth(&self) -> f64 {
        self.max_bw
    }

    /// Estimated current delivery rate in bytes/sec.
    #[must_use]
    pub fn current_rate(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let (oldest_ts, oldest_bytes) = self.samples.front().copied().unwrap();
        let (newest_ts, newest_bytes) = self.samples.back().copied().unwrap();
        let dt = newest_ts.duration_since(oldest_ts).as_secs_f64();
        if dt > 0.001 {
            (newest_bytes - oldest_bytes) as f64 / dt
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// Congestion Controller Trait
// ---------------------------------------------------------------------------

/// Common interface for congestion control algorithms.
pub trait CongestionController: Send + Sync {
    /// Notification: `bytes` were successfully acknowledged.
    fn on_ack(&mut self, bytes: u64, rtt: Duration);

    /// Notification: a packet was detected as lost.
    fn on_loss(&mut self, bytes: u64);

    /// Current congestion window in bytes.
    fn cwnd(&self) -> u64;

    /// Recommended pacing rate in bytes/sec.
    fn pacing_rate(&self) -> f64;

    /// Whether the controller allows sending `bytes` right now.
    fn can_send(&self, bytes_in_flight: u64) -> bool {
        bytes_in_flight < self.cwnd()
    }
}

// ---------------------------------------------------------------------------
// BBR Controller
// ---------------------------------------------------------------------------

/// BBR state machine phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BbrState {
    /// Exponential probing for maximum bandwidth.
    Startup,
    /// Drain the queue built up during Startup.
    Drain,
    /// Steady-state bandwidth probing.
    ProbeBw,
    /// Periodically probe for minimum RTT.
    ProbeRtt,
}

/// Configuration for the BBR controller.
#[derive(Debug, Clone)]
pub struct BbrConfig {
    /// Minimum congestion window (default: 2 * 1400 = 2800).
    pub min_cwnd: u64,
    /// Initial congestion window (default: 10 * 1400 = 14000).
    pub initial_cwnd: u64,
    /// Loss rate threshold to trigger drain / reduce (default: 0.01 = 1%).
    pub loss_threshold: f64,
    /// Bandwidth filter window (default: 2s).
    pub bw_filter_window: Duration,
    /// Pacing gain during startup.
    pub startup_pacing_gain: f64,
    /// Pacing gain during ProbeBw.
    pub probe_bw_pacing_gain: f64,
    /// RTT probe interval.
    pub probe_rtt_interval: Duration,
    /// Duration to stay in ProbeRtt state.
    pub probe_rtt_duration: Duration,
}

impl Default for BbrConfig {
    fn default() -> Self {
        Self {
            min_cwnd: 2 * 1400,
            initial_cwnd: 10 * 1400,
            loss_threshold: 0.01,
            bw_filter_window: Duration::from_secs(2),
            startup_pacing_gain: 2.885,
            probe_bw_pacing_gain: 1.25,
            probe_rtt_interval: Duration::from_secs(10),
            probe_rtt_duration: Duration::from_millis(200),
        }
    }
}

/// BBRv2-style congestion controller.
#[derive(Debug)]
pub struct BbrController {
    config: BbrConfig,
    state: BbrState,
    rtt: RttEstimator,
    bw: BandwidthEstimator,
    loss: LossDetector,
    cwnd: u64,
    /// Bytes acked during the current Startup phase.
    startup_acked: u64,
    /// Previous max_bw sample — for Startup exit detection.
    prev_max_bw: f64,
    /// Rounds without bandwidth growth (triggers Startup→Drain).
    rounds_without_growth: u32,
    /// When we last entered ProbeRtt.
    last_probe_rtt: Option<Instant>,
    /// When we entered the current ProbeRtt cycle.
    probe_rtt_start: Option<Instant>,
}

impl BbrController {
    /// Create a new BBR controller with the given config.
    #[must_use]
    pub fn new(config: BbrConfig) -> Self {
        let cwnd = config.initial_cwnd;
        Self {
            bw: BandwidthEstimator::new(config.bw_filter_window),
            loss: LossDetector::new(config.bw_filter_window),
            config,
            state: BbrState::Startup,
            rtt: RttEstimator::new(),
            cwnd,
            startup_acked: 0,
            prev_max_bw: 0.0,
            rounds_without_growth: 0,
            last_probe_rtt: None,
            probe_rtt_start: None,
        }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(BbrConfig::default())
    }

    /// Current BBR state.
    #[must_use]
    pub fn state(&self) -> BbrState {
        self.state
    }

    /// Access the RTT estimator.
    #[must_use]
    pub fn rtt_estimator(&self) -> &RttEstimator {
        &self.rtt
    }

    /// Access the bandwidth estimator.
    #[must_use]
    pub fn bandwidth_estimator(&self) -> &BandwidthEstimator {
        &self.bw
    }

    /// Access the loss detector.
    #[must_use]
    pub fn loss_detector(&self) -> &LossDetector {
        &self.loss
    }

    fn transition(&mut self) {
        let now = Instant::now();
        match self.state {
            BbrState::Startup => {
                let cur_bw = self.bw.max_bandwidth();
                if cur_bw > 0.0 {
                    let growth = (cur_bw - self.prev_max_bw) / cur_bw;
                    if growth < 0.25 {
                        self.rounds_without_growth += 1;
                    } else {
                        self.rounds_without_growth = 0;
                    }
                    self.prev_max_bw = cur_bw;
                }
                // Exit Startup after 3 rounds without 25% growth, or on loss
                if self.rounds_without_growth >= 3
                    || self.loss.loss_rate() > self.config.loss_threshold
                {
                    self.state = BbrState::Drain;
                }
            }
            BbrState::Drain => {
                // Drain until inflight ≤ BDP estimate
                let bdp = self.estimate_bdp();
                if self.cwnd <= bdp.max(self.config.min_cwnd) {
                    self.state = BbrState::ProbeBw;
                    self.last_probe_rtt = Some(now);
                }
            }
            BbrState::ProbeBw => {
                // Periodically enter ProbeRtt
                if let Some(last) = self.last_probe_rtt {
                    if now.duration_since(last) > self.config.probe_rtt_interval {
                        self.state = BbrState::ProbeRtt;
                        self.probe_rtt_start = Some(now);
                    }
                }
            }
            BbrState::ProbeRtt => {
                if let Some(start) = self.probe_rtt_start {
                    if now.duration_since(start) > self.config.probe_rtt_duration {
                        self.state = BbrState::ProbeBw;
                        self.last_probe_rtt = Some(now);
                        self.probe_rtt_start = None;
                    }
                }
            }
        }
    }

    fn estimate_bdp(&self) -> u64 {
        let bw = self.bw.max_bandwidth();
        let rtt = self
            .rtt
            .min_rtt()
            .unwrap_or(Duration::from_millis(50))
            .as_secs_f64();
        (bw * rtt) as u64
    }

    fn update_cwnd(&mut self) {
        let bdp = self.estimate_bdp();
        let target = match self.state {
            BbrState::Startup => bdp.max(self.config.initial_cwnd) * 2,
            BbrState::Drain => bdp,
            BbrState::ProbeBw => {
                let gain = self.config.probe_bw_pacing_gain;
                ((bdp as f64) * gain) as u64
            }
            BbrState::ProbeRtt => {
                // Reduce to 4 packets worth
                4 * 1400
            }
        };
        self.cwnd = target.max(self.config.min_cwnd);
    }
}

impl CongestionController for BbrController {
    fn on_ack(&mut self, bytes: u64, rtt: Duration) {
        self.rtt.update(rtt);
        self.bw.on_ack(bytes);
        self.loss.on_ack();
        self.startup_acked += bytes;
        self.transition();
        self.update_cwnd();
    }

    fn on_loss(&mut self, _bytes: u64) {
        self.loss.on_loss();
        self.transition();
        // On loss in ProbeBw, reduce cwnd
        if self.state == BbrState::ProbeBw {
            let bdp = self.estimate_bdp();
            self.cwnd = (bdp * 7 / 8).max(self.config.min_cwnd);
        }
        self.update_cwnd();
    }

    fn cwnd(&self) -> u64 {
        self.cwnd
    }

    fn pacing_rate(&self) -> f64 {
        let bw = self.bw.max_bandwidth();
        if bw == 0.0 {
            // No estimate yet — allow initial burst
            return (self.config.initial_cwnd as f64) * 10.0;
        }
        let gain = match self.state {
            BbrState::Startup => self.config.startup_pacing_gain,
            BbrState::Drain => 0.75,
            BbrState::ProbeBw => self.config.probe_bw_pacing_gain,
            BbrState::ProbeRtt => 1.0,
        };
        bw * gain
    }
}

// ---------------------------------------------------------------------------
// Fixed-Rate Controller (for testing)
// ---------------------------------------------------------------------------

/// A simple fixed-rate congestion controller, useful for testing and
/// deterministic benchmarks.
#[derive(Debug, Clone)]
pub struct FixedRateController {
    cwnd: u64,
    rate: f64,
}

impl FixedRateController {
    /// Create with a fixed window and pacing rate.
    #[must_use]
    pub fn new(cwnd: u64, rate_bytes_per_sec: f64) -> Self {
        Self {
            cwnd,
            rate: rate_bytes_per_sec,
        }
    }
}

impl CongestionController for FixedRateController {
    fn on_ack(&mut self, _bytes: u64, _rtt: Duration) {}
    fn on_loss(&mut self, _bytes: u64) {}
    fn cwnd(&self) -> u64 {
        self.cwnd
    }
    fn pacing_rate(&self) -> f64 {
        self.rate
    }
}
