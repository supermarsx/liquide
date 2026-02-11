//! Heartbeat monitoring for session health.

use std::time::Instant;

/// Configuration for heartbeat monitoring.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between heartbeats in seconds.
    pub interval_sec: u64,
    /// Number of consecutive missed heartbeats before timeout.
    pub timeout_count: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval_sec: 5,
            timeout_count: 3,
        }
    }
}

/// Current heartbeat monitoring state.
pub struct HeartbeatState {
    /// Number of consecutive heartbeats missed.
    pub consecutive_missed: u32,
    /// Timestamp of the last received heartbeat.
    pub last_received: Option<Instant>,
    /// Timestamp of the last sent heartbeat.
    pub last_sent: Option<Instant>,
}

/// Result of a heartbeat check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatStatus {
    /// Heartbeats are arriving within expected intervals.
    Healthy,
    /// Some heartbeats have been missed, but below threshold.
    Warning { missed: u32 },
    /// Heartbeat timeout threshold exceeded.
    TimedOut { missed: u32 },
}

/// Monitors heartbeat health for a session.
pub struct HeartbeatMonitor {
    config: HeartbeatConfig,
    consecutive_missed: u32,
    last_received: Option<Instant>,
    last_sent: Option<Instant>,
    total_sent: u64,
    total_received: u64,
}

impl HeartbeatMonitor {
    /// Create a new heartbeat monitor.
    #[must_use]
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            consecutive_missed: 0,
            last_received: None,
            last_sent: None,
            total_sent: 0,
            total_received: 0,
        }
    }

    /// Record that a heartbeat was sent.
    pub fn record_sent(&mut self) {
        self.last_sent = Some(Instant::now());
        self.total_sent += 1;
        self.consecutive_missed += 1;
    }

    /// Record that a heartbeat response was received.
    pub fn record_received(&mut self) {
        self.last_received = Some(Instant::now());
        self.total_received += 1;
        self.consecutive_missed = 0;
    }

    /// Whether the heartbeat is considered healthy (no missed beats above threshold).
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.consecutive_missed < self.config.timeout_count
    }

    /// Number of consecutive heartbeats missed.
    #[must_use]
    pub fn missed_count(&self) -> u32 {
        self.consecutive_missed
    }

    /// Check the current heartbeat status.
    #[must_use]
    pub fn check(&self) -> HeartbeatStatus {
        if self.consecutive_missed == 0 {
            HeartbeatStatus::Healthy
        } else if self.consecutive_missed >= self.config.timeout_count {
            HeartbeatStatus::TimedOut {
                missed: self.consecutive_missed,
            }
        } else {
            HeartbeatStatus::Warning {
                missed: self.consecutive_missed,
            }
        }
    }

    /// The current state snapshot.
    #[must_use]
    pub fn state(&self) -> HeartbeatState {
        HeartbeatState {
            consecutive_missed: self.consecutive_missed,
            last_received: self.last_received,
            last_sent: self.last_sent,
        }
    }

    /// Total heartbeats sent.
    #[must_use]
    pub fn total_sent(&self) -> u64 {
        self.total_sent
    }

    /// Total heartbeat responses received.
    #[must_use]
    pub fn total_received(&self) -> u64 {
        self.total_received
    }

    /// The configured timeout count.
    #[must_use]
    pub fn timeout_count(&self) -> u32 {
        self.config.timeout_count
    }

    /// Reset the monitor state.
    pub fn reset(&mut self) {
        self.consecutive_missed = 0;
        self.last_received = None;
        self.last_sent = None;
        self.total_sent = 0;
        self.total_received = 0;
    }
}
