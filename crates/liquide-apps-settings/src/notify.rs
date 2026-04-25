//! Change notifications for other system components.

use crate::entry::SettingValue;

/// A notification that a setting has changed.
#[derive(Debug, Clone)]
pub struct SettingNotification {
    /// The setting key that changed.
    pub key: String,
    /// The new value.
    pub value: SettingValue,
    /// Timestamp (epoch seconds).
    pub timestamp: u64,
}

/// Collects notifications for dispatch to other components.
pub struct NotificationQueue {
    queue: Vec<SettingNotification>,
    /// Whether to batch notifications (defer until flush).
    batching: bool,
}

impl NotificationQueue {
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            batching: false,
        }
    }

    /// Enable or disable batching.
    pub fn set_batching(&mut self, batching: bool) {
        self.batching = batching;
    }

    /// Whether batching is enabled.
    #[must_use]
    pub fn is_batching(&self) -> bool {
        self.batching
    }

    /// Push a notification.
    pub fn push(&mut self, key: impl Into<String>, value: SettingValue, timestamp: u64) {
        self.queue.push(SettingNotification {
            key: key.into(),
            value,
            timestamp,
        });
    }

    /// Drain all queued notifications.
    pub fn drain(&mut self) -> Vec<SettingNotification> {
        std::mem::take(&mut self.queue)
    }

    /// Number of queued notifications.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Peek at queued notifications without consuming them.
    #[must_use]
    pub fn peek(&self) -> &[SettingNotification] {
        &self.queue
    }
}

impl Default for NotificationQueue {
    fn default() -> Self {
        Self::new()
    }
}
