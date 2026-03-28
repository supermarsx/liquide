//! Window timers — one-shot and repeating timers that generate
//! [`WindowMessage::Timer`] messages.
//!
//! The [`TimerManager`] is ticked by the event loop with the elapsed
//! milliseconds since the last tick.  Expired timers produce
//! [`MessageTarget`]s that the caller can feed into the dispatcher.

use crate::message::{MessagePriority, MessageTarget, WindowMessage};
use crate::types::WindowId;

/// Opaque timer identifier.
pub type TimerId = u64;

/// A single timer instance.
#[derive(Debug, Clone)]
pub struct Timer {
    /// Unique identifier.
    pub id: TimerId,
    /// The window that will receive the `Timer` message.
    pub window_id: WindowId,
    /// The timer interval in milliseconds.
    pub interval_ms: u64,
    /// Whether the timer repeats after firing.
    pub repeat: bool,
    /// Milliseconds remaining until the next fire.
    pub remaining_ms: u64,
}

/// Manages a set of active timers.
///
/// Call [`tick`] once per event-loop iteration with the elapsed time.
/// Expired timers produce [`MessageTarget`]s that should be dispatched.
#[derive(Debug)]
pub struct TimerManager {
    timers: Vec<Timer>,
    next_id: TimerId,
}

impl TimerManager {
    /// Create an empty timer manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timers: Vec::new(),
            next_id: 1,
        }
    }

    /// Create a timer.
    ///
    /// * `window_id` — the window that will receive [`WindowMessage::Timer`].
    /// * `interval_ms` — fire interval in milliseconds.
    /// * `repeat` — if `false`, the timer fires once then is automatically
    ///   removed.
    ///
    /// Returns the [`TimerId`] that can be used with [`kill_timer`].
    pub fn set_timer(&mut self, window_id: WindowId, interval_ms: u64, repeat: bool) -> TimerId {
        let id = self.next_id;
        self.next_id += 1;

        self.timers.push(Timer {
            id,
            window_id,
            interval_ms,
            repeat,
            remaining_ms: interval_ms,
        });

        id
    }

    /// Remove a timer by ID.
    ///
    /// Returns `true` if the timer existed and was removed.
    pub fn kill_timer(&mut self, timer_id: TimerId) -> bool {
        let before = self.timers.len();
        self.timers.retain(|t| t.id != timer_id);
        self.timers.len() < before
    }

    /// Advance all timers by `elapsed_ms` milliseconds.
    ///
    /// Returns a list of [`MessageTarget`]s for each timer that fired.  Timers
    /// that fire are either reset (repeating) or removed (one-shot).
    pub fn tick(&mut self, elapsed_ms: u64) -> Vec<MessageTarget> {
        let mut fired = Vec::new();
        let mut to_remove = Vec::new();

        for timer in &mut self.timers {
            if elapsed_ms >= timer.remaining_ms {
                // Timer has fired.
                fired.push(MessageTarget::with_priority(
                    timer.window_id,
                    WindowMessage::Timer(timer.id),
                    MessagePriority::High,
                ));

                if timer.repeat {
                    // Account for overshoot so timers don't drift.
                    let overshoot = elapsed_ms - timer.remaining_ms;
                    timer.remaining_ms =
                        timer.interval_ms.saturating_sub(overshoot % timer.interval_ms);
                    // If interval is 0 (degenerate), prevent infinite re-fire
                    // within a single tick.
                    if timer.interval_ms == 0 {
                        timer.remaining_ms = 0;
                    }
                } else {
                    to_remove.push(timer.id);
                }
            } else {
                timer.remaining_ms -= elapsed_ms;
            }
        }

        // Remove one-shot timers that fired.
        if !to_remove.is_empty() {
            self.timers.retain(|t| !to_remove.contains(&t.id));
        }

        fired
    }

    /// Number of active timers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.timers.len()
    }

    /// Whether there are no active timers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// Look up a timer by ID.
    #[must_use]
    pub fn get(&self, timer_id: TimerId) -> Option<&Timer> {
        self.timers.iter().find(|t| t.id == timer_id)
    }

    /// Remove all timers associated with a window (e.g. when it is destroyed).
    pub fn kill_all_for_window(&mut self, window_id: WindowId) {
        self.timers.retain(|t| t.window_id != window_id);
    }

    /// Remove all timers.
    pub fn clear(&mut self) {
        self.timers.clear();
    }
}

impl Default for TimerManager {
    fn default() -> Self {
        Self::new()
    }
}
