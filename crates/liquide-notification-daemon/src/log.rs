//! Notification event log.
//!
//! [`NotificationLog`] records every notification lifecycle event (shown,
//! clicked, dismissed, expired, action invoked) with timestamps. This is
//! separate from [`crate::history::NotificationHistory`] which stores the
//! full notification payload for "missed notifications" — the log is a
//! lightweight audit trail.

use serde::{Deserialize, Serialize};

/// What happened to a notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogAction {
    /// The notification was shown to the user.
    Shown,
    /// The user clicked the notification body.
    Clicked,
    /// The user explicitly dismissed the notification.
    Dismissed,
    /// The notification expired due to timeout.
    Expired,
    /// The user invoked a named action button.
    ActionInvoked(String),
}

/// A single entry in the notification log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// The notification's server-assigned ID.
    pub notification_id: u64,
    /// The application that sent the notification.
    pub app_id: String,
    /// The notification summary text.
    pub summary: String,
    /// The notification body text.
    pub body: String,
    /// Timestamp in milliseconds (e.g. since epoch) when this event occurred.
    pub timestamp_ms: u64,
    /// What action was taken on the notification.
    pub action: LogAction,
}

/// A bounded, in-memory notification event log.
///
/// Stores log entries in chronological order (oldest first). When `max_entries`
/// is reached, the oldest entry is evicted.
pub struct NotificationLog {
    /// All recorded entries (oldest first).
    pub entries: Vec<LogEntry>,
    /// Maximum number of entries before eviction.
    pub max_entries: usize,
}

impl NotificationLog {
    /// Creates a new log with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Records a notification event.
    pub fn record(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.max_entries && self.max_entries > 0 {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Convenience method to record an event from individual fields.
    pub fn record_event(
        &mut self,
        notification_id: u64,
        app_id: &str,
        summary: &str,
        body: &str,
        timestamp_ms: u64,
        action: LogAction,
    ) {
        self.record(LogEntry {
            notification_id,
            app_id: app_id.to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            timestamp_ms,
            action,
        });
    }

    /// Returns all entries for a specific application (chronological order).
    pub fn entries_for_app(&self, app_id: &str) -> Vec<&LogEntry> {
        self.entries.iter().filter(|e| e.app_id == app_id).collect()
    }

    /// Returns all entries with a timestamp >= `timestamp_ms` (chronological).
    pub fn entries_since(&self, timestamp_ms: u64) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp_ms >= timestamp_ms)
            .collect()
    }

    /// Clears the entire log.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Removes all entries for a specific application.
    pub fn clear_for_app(&mut self, app_id: &str) {
        self.entries.retain(|e| e.app_id != app_id);
    }

    /// Returns the total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all entries as a slice (oldest first).
    pub fn all_entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Returns entries filtered by action type.
    pub fn entries_by_action(&self, action: &LogAction) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| &e.action == action)
            .collect()
    }
}

impl Default for NotificationLog {
    fn default() -> Self {
        Self::new(5000)
    }
}
