//! Heartbeat monitoring for managed sessions.

use std::collections::HashMap;
use std::time::Instant;

/// Configuration for heartbeat monitoring.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval in seconds between expected heartbeats.
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

/// Health state of a monitored session's heartbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatState {
    /// Heartbeats arriving normally.
    Healthy,
    /// Some heartbeats missed but below threshold.
    Warning,
    /// Heartbeat timeout threshold exceeded.
    TimedOut,
}

impl std::fmt::Display for HeartbeatState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "Healthy"),
            Self::Warning => write!(f, "Warning"),
            Self::TimedOut => write!(f, "TimedOut"),
        }
    }
}

/// Heartbeat tracking entry for a single session.
#[derive(Debug)]
pub struct HeartbeatEntry {
    /// Session identifier.
    pub session_id: String,
    /// Timestamp of the last received heartbeat.
    pub last_received: Instant,
    /// Number of consecutive missed heartbeats.
    pub missed_count: u32,
    /// Current health state.
    pub state: HeartbeatState,
}

/// Alert produced when a heartbeat check detects a problem.
#[derive(Debug, Clone)]
pub struct HeartbeatAlert {
    /// Session that produced the alert.
    pub session_id: String,
    /// Number of missed heartbeats.
    pub missed_count: u32,
    /// State at the time of the alert.
    pub state: HeartbeatState,
}

/// Tracks heartbeat health across all managed sessions.
pub struct HeartbeatTracker {
    config: HeartbeatConfig,
    entries: HashMap<String, HeartbeatEntry>,
}

impl HeartbeatTracker {
    /// Create a new heartbeat tracker.
    #[must_use]
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
        }
    }

    /// Register a session for heartbeat tracking.
    pub fn register(&mut self, session_id: String) {
        self.entries.insert(
            session_id.clone(),
            HeartbeatEntry {
                session_id,
                last_received: Instant::now(),
                missed_count: 0,
                state: HeartbeatState::Healthy,
            },
        );
    }

    /// Unregister a session from heartbeat tracking.
    pub fn unregister(&mut self, session_id: &str) {
        self.entries.remove(session_id);
    }

    /// Record a heartbeat received from a session.
    pub fn record_heartbeat(&mut self, session_id: &str) {
        if let Some(entry) = self.entries.get_mut(session_id) {
            entry.last_received = Instant::now();
            entry.missed_count = 0;
            entry.state = HeartbeatState::Healthy;
        }
    }

    /// Check all tracked sessions and return alerts for unhealthy ones.
    ///
    /// This increments the missed count for each session, simulating a
    /// heartbeat tick. Sessions that have called `record_heartbeat` since
    /// the last check will have had their counter reset.
    pub fn check_all(&mut self) -> Vec<HeartbeatAlert> {
        let timeout = self.config.timeout_count;
        let mut alerts = Vec::new();

        for entry in self.entries.values_mut() {
            entry.missed_count += 1;

            if entry.missed_count >= timeout {
                entry.state = HeartbeatState::TimedOut;
                alerts.push(HeartbeatAlert {
                    session_id: entry.session_id.clone(),
                    missed_count: entry.missed_count,
                    state: HeartbeatState::TimedOut,
                });
            } else if entry.missed_count > 0 {
                entry.state = HeartbeatState::Warning;
                alerts.push(HeartbeatAlert {
                    session_id: entry.session_id.clone(),
                    missed_count: entry.missed_count,
                    state: HeartbeatState::Warning,
                });
            }
        }

        alerts
    }

    /// Get the heartbeat entry for a session.
    #[must_use]
    pub fn get_entry(&self, session_id: &str) -> Option<&HeartbeatEntry> {
        self.entries.get(session_id)
    }

    /// Number of tracked sessions.
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.entries.len()
    }

    /// The configured timeout count.
    #[must_use]
    pub fn timeout_count(&self) -> u32 {
        self.config.timeout_count
    }
}
