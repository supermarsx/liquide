//! Window event history — ring-buffer audit trail of window operations.

use std::collections::VecDeque;
use std::fmt;

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

use crate::window::{WindowFlags, WindowId, WindowState};

/// The kind of window event recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WindowEventKind {
    /// Window was opened.
    Opened,
    /// Window was closed.
    Closed,
    /// Window was moved.
    Moved { from: Rect, to: Rect },
    /// Window was resized.
    Resized { from: Rect, to: Rect },
    /// Window state changed (e.g. Normal -> Maximized).
    StateChanged { from: WindowState, to: WindowState },
    /// Window gained focus.
    Focused,
    /// Window lost focus.
    Unfocused,
    /// Window title changed.
    TitleChanged { from: String, to: String },
    /// Window z-order changed.
    ZOrderChanged { from: i32, to: i32 },
    /// Window visibility changed.
    VisibilityChanged { from: bool, to: bool },
    /// Window flags changed.
    FlagsChanged { from: WindowFlags, to: WindowFlags },
}

impl fmt::Display for WindowEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Opened => write!(f, "Opened"),
            Self::Closed => write!(f, "Closed"),
            Self::Moved { from, to } => write!(
                f,
                "Moved from ({},{},{},{}) to ({},{},{},{})",
                from.x, from.y, from.width, from.height, to.x, to.y, to.width, to.height
            ),
            Self::Resized { from, to } => write!(
                f,
                "Resized from ({},{},{},{}) to ({},{},{},{})",
                from.x, from.y, from.width, from.height, to.x, to.y, to.width, to.height
            ),
            Self::StateChanged { from, to } => write!(f, "StateChanged {from} -> {to}"),
            Self::Focused => write!(f, "Focused"),
            Self::Unfocused => write!(f, "Unfocused"),
            Self::TitleChanged { from, to } => {
                write!(f, "TitleChanged \"{from}\" -> \"{to}\"")
            }
            Self::ZOrderChanged { from, to } => write!(f, "ZOrderChanged {from} -> {to}"),
            Self::VisibilityChanged { from, to } => {
                write!(f, "VisibilityChanged {from} -> {to}")
            }
            Self::FlagsChanged { from, to } => write!(f, "FlagsChanged {from} -> {to}"),
        }
    }
}

/// A timestamped window event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowEvent {
    pub window_id: WindowId,
    pub timestamp_us: u64,
    pub kind: WindowEventKind,
}

impl fmt::Display for WindowEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}us] {} {}",
            self.timestamp_us, self.window_id, self.kind
        )
    }
}

/// Ring-buffer based window event history.
pub struct WindowHistory {
    events: VecDeque<WindowEvent>,
    capacity: usize,
    next_timestamp: u64,
}

impl WindowHistory {
    /// Create a new history with the given maximum capacity.
    ///
    /// A capacity of 0 disables recording (events are silently dropped).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
            next_timestamp: 1,
        }
    }

    /// Record an event using the internal monotonic timestamp.
    /// Returns the assigned timestamp.
    pub fn record(&mut self, window_id: WindowId, kind: WindowEventKind) -> u64 {
        let ts = self.next_timestamp;
        self.next_timestamp += 1;
        self.record_at(window_id, kind, ts);
        ts
    }

    /// Record an event with an explicit timestamp.
    pub fn record_at(&mut self, window_id: WindowId, kind: WindowEventKind, timestamp_us: u64) {
        if self.capacity == 0 {
            return;
        }
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(WindowEvent {
            window_id,
            timestamp_us,
            kind,
        });
    }

    /// Get all events for a specific window.
    #[must_use]
    pub fn events_for_window(&self, id: WindowId) -> Vec<&WindowEvent> {
        self.events.iter().filter(|e| e.window_id == id).collect()
    }

    /// Get the most recent `n` events (across all windows).
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<&WindowEvent> {
        self.events.iter().rev().take(n).rev().collect()
    }

    /// Get events filtered by a predicate on the event kind.
    #[must_use]
    pub fn events_by_kind(&self, filter: &dyn Fn(&WindowEventKind) -> bool) -> Vec<&WindowEvent> {
        self.events.iter().filter(|e| filter(&e.kind)).collect()
    }

    /// Get events in a time range \[start_us, end_us\] (inclusive).
    #[must_use]
    pub fn events_in_range(&self, start_us: u64, end_us: u64) -> Vec<&WindowEvent> {
        self.events
            .iter()
            .filter(|e| e.timestamp_us >= start_us && e.timestamp_us <= end_us)
            .collect()
    }

    /// Total number of events currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl fmt::Display for WindowHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WindowHistory({}/{} events)", self.len(), self.capacity)
    }
}
