//! Persistent notification log.
//!
//! [`NotificationHistory`] records closed notifications along with display/close
//! timestamps, allowing the shell to show a "missed notifications" panel.

use crate::spec::{CloseReason, Notification};
use serde::{Deserialize, Serialize};

/// A single entry in the notification history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The notification that was displayed.
    pub notification: Notification,
    /// Why the notification was closed.
    pub close_reason: CloseReason,
    /// Timestamp (ms since epoch) when the notification was first displayed.
    pub displayed_at: u64,
    /// Timestamp (ms since epoch) when the notification was closed.
    pub closed_at: u64,
}

/// In-memory notification history log.
///
/// Entries are stored in chronological order (newest last). An optional
/// capacity limit prevents unbounded growth.
pub struct NotificationHistory {
    entries: Vec<HistoryEntry>,
    capacity: usize,
}

impl NotificationHistory {
    /// Creates a new history with the given maximum capacity.
    /// When capacity is reached, the oldest entry is evicted.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Records a closed notification in the history.
    ///
    /// Transient notifications (with the `transient` hint set) are NOT recorded.
    pub fn record(
        &mut self,
        notification: &Notification,
        close_reason: CloseReason,
        displayed_at: u64,
        closed_at: u64,
    ) {
        // Skip transient notifications per spec.
        if notification.hints.transient {
            return;
        }

        // Evict oldest if at capacity.
        if self.entries.len() >= self.capacity && self.capacity > 0 {
            self.entries.remove(0);
        }

        self.entries.push(HistoryEntry {
            notification: notification.clone(),
            close_reason,
            displayed_at,
            closed_at,
        });
    }

    /// Returns the most recent `count` entries (newest first).
    pub fn recent(&self, count: usize) -> Vec<HistoryEntry> {
        let len = self.entries.len();
        let start = len.saturating_sub(count);
        let mut result: Vec<HistoryEntry> = self.entries[start..].to_vec();
        result.reverse();
        result
    }

    /// Returns all entries from a specific application (newest first).
    pub fn by_app(&self, app_name: &str) -> Vec<HistoryEntry> {
        let mut result: Vec<HistoryEntry> = self
            .entries
            .iter()
            .filter(|e| e.notification.app_name == app_name)
            .cloned()
            .collect();
        result.reverse();
        result
    }

    /// Clears the entire history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the total number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the capacity of the history.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns all entries as a slice (oldest first).
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }
}

impl Default for NotificationHistory {
    fn default() -> Self {
        Self::new(1000)
    }
}
