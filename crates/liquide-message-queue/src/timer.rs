//! Timer registration and expiry checking.
//!
//! Each timer is identified by a `(WindowId, timer_id)` pair.  When the timer
//! fires it produces a `MessageType::Timer(timer_id)` message targeted at the
//! owning window.

use crate::message::{MessageType, QueueMessage, WindowId};

/// A registered timer entry.
#[derive(Debug, Clone)]
pub struct TimerEntry {
    /// Window that owns this timer.
    pub window_id: WindowId,
    /// Application-chosen timer identifier.
    pub timer_id: u32,
    /// Interval in milliseconds.
    pub interval_ms: u32,
    /// Next fire time in microseconds (same clock domain as `QueueMessage::time`).
    pub next_fire: u64,
    /// Optional callback identifier (reserved for future use).
    pub callback: Option<u64>,
}

/// Manages the set of active timers for a single thread queue.
#[derive(Debug, Default)]
pub struct TimerManager {
    timers: Vec<TimerEntry>,
}

impl TimerManager {
    /// Create an empty timer manager.
    #[must_use]
    pub fn new() -> Self {
        Self { timers: Vec::new() }
    }

    /// Register (or replace) a timer.
    ///
    /// If a timer with the same `(window_id, timer_id)` already exists, it is
    /// replaced.  `now_us` is the current timestamp used to compute the first
    /// fire time.
    pub fn set_timer(
        &mut self,
        window_id: WindowId,
        timer_id: u32,
        interval_ms: u32,
        now_us: u64,
    ) {
        let next_fire = now_us + (interval_ms as u64) * 1000;
        // Replace existing timer with same key
        if let Some(entry) = self
            .timers
            .iter_mut()
            .find(|t| t.window_id == window_id && t.timer_id == timer_id)
        {
            entry.interval_ms = interval_ms;
            entry.next_fire = next_fire;
            entry.callback = None;
            return;
        }
        self.timers.push(TimerEntry {
            window_id,
            timer_id,
            interval_ms,
            next_fire,
            callback: None,
        });
    }

    /// Register a timer with an optional callback id.
    pub fn set_timer_with_callback(
        &mut self,
        window_id: WindowId,
        timer_id: u32,
        interval_ms: u32,
        now_us: u64,
        callback: u64,
    ) {
        let next_fire = now_us + (interval_ms as u64) * 1000;
        if let Some(entry) = self
            .timers
            .iter_mut()
            .find(|t| t.window_id == window_id && t.timer_id == timer_id)
        {
            entry.interval_ms = interval_ms;
            entry.next_fire = next_fire;
            entry.callback = Some(callback);
            return;
        }
        self.timers.push(TimerEntry {
            window_id,
            timer_id,
            interval_ms,
            next_fire,
            callback: Some(callback),
        });
    }

    /// Remove a timer.  Returns `true` if the timer existed.
    pub fn kill_timer(&mut self, window_id: WindowId, timer_id: u32) -> bool {
        let before = self.timers.len();
        self.timers
            .retain(|t| !(t.window_id == window_id && t.timer_id == timer_id));
        self.timers.len() != before
    }

    /// Remove all timers associated with a window.
    pub fn kill_all_for_window(&mut self, window_id: WindowId) {
        self.timers.retain(|t| t.window_id != window_id);
    }

    /// Scan the timer list and generate `Timer` messages for all timers that
    /// have fired.  Each fired timer is automatically rescheduled for its next
    /// interval.
    ///
    /// Returns the generated timer messages (may be empty).
    pub fn check_timers(&mut self, now_us: u64) -> Vec<QueueMessage> {
        let mut msgs = Vec::new();
        for entry in &mut self.timers {
            if now_us >= entry.next_fire {
                let mut msg = QueueMessage::new(entry.window_id, MessageType::Timer(entry.timer_id));
                msg.time = now_us;
                if let Some(cb) = entry.callback {
                    msg.lparam = cb as i64;
                }
                msgs.push(msg);
                // Reschedule — skip missed intervals to avoid storm.
                let interval_us = (entry.interval_ms as u64) * 1000;
                if interval_us > 0 {
                    // Jump forward to the next future fire time.
                    let elapsed = now_us - entry.next_fire;
                    let periods = elapsed / interval_us + 1;
                    entry.next_fire += periods * interval_us;
                } else {
                    // Zero-interval: fire every check, advance by 1us to avoid infinite loop.
                    entry.next_fire = now_us + 1;
                }
            }
        }
        msgs
    }

    /// Returns the number of active timers.
    #[must_use]
    pub fn count(&self) -> usize {
        self.timers.len()
    }

    /// Returns `true` if any timer has fired (i.e., `next_fire <= now_us`).
    #[must_use]
    pub fn any_expired(&self, now_us: u64) -> bool {
        self.timers.iter().any(|t| now_us >= t.next_fire)
    }

    /// Returns the nearest fire time across all timers, or `None` if no timers
    /// are registered.
    #[must_use]
    pub fn nearest_deadline(&self) -> Option<u64> {
        self.timers.iter().map(|t| t.next_fire).min()
    }

    /// Iterate over all timer entries (read-only).
    pub fn iter(&self) -> impl Iterator<Item = &TimerEntry> {
        self.timers.iter()
    }
}
