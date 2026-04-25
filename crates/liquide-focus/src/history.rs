//! Activation history — ring buffer of recent activation changes.

use serde::{Deserialize, Serialize};

use crate::types::{ActivateReason, WindowId};

/// A single activation record stored in the history ring buffer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationRecord {
    /// The window that was activated.
    pub window_id: WindowId,
    /// Monotonic timestamp in milliseconds when the activation occurred.
    pub timestamp_ms: u64,
    /// Why the activation happened.
    pub reason: ActivateReason,
}

/// Ring buffer of recent activation changes.
///
/// Fixed capacity (default 64).  When full, the oldest entry is overwritten.
#[derive(Debug, Clone)]
pub struct ActivationHistory {
    entries: Vec<ActivationRecord>,
    /// Maximum number of entries.
    capacity: usize,
    /// Write cursor — points to the slot that will be written next.
    head: usize,
    /// How many slots are currently occupied (≤ capacity).
    len: usize,
}

impl ActivationHistory {
    /// Default ring buffer capacity.
    pub const DEFAULT_CAPACITY: usize = 64;

    /// Create a new history with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push a new record into the ring buffer.
    pub fn push(&mut self, record: ActivationRecord) {
        if self.entries.len() < self.capacity {
            // Still filling the initial buffer.
            self.entries.push(record);
            self.head = self.entries.len() % self.capacity;
            self.len = self.entries.len();
        } else {
            self.entries[self.head] = record;
            self.head = (self.head + 1) % self.capacity;
            self.len = self.capacity;
        }
    }

    /// Return the most recent `count` records, newest first.
    #[must_use]
    pub fn recent(&self, count: usize) -> Vec<ActivationRecord> {
        let count = count.min(self.len);
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            // Walk backwards from head.
            let idx = if self.head == 0 {
                self.entries.len() - 1 - i
            } else {
                (self.head + self.entries.len() - 1 - i) % self.entries.len()
            };
            out.push(self.entries[idx].clone());
        }
        out
    }

    /// Check if `window_id` was activated within the last `within_ms`
    /// milliseconds relative to `now_ms`.
    #[must_use]
    pub fn was_recently_active(&self, window_id: WindowId, within_ms: u64, now_ms: u64) -> bool {
        let cutoff = now_ms.saturating_sub(within_ms);
        for i in 0..self.len {
            let idx = if self.head == 0 {
                self.entries.len() - 1 - i
            } else {
                (self.head + self.entries.len() - 1 - i) % self.entries.len()
            };
            let entry = &self.entries[idx];
            if entry.timestamp_ms < cutoff {
                // All older entries are also before the cutoff.
                return false;
            }
            if entry.window_id == window_id {
                return true;
            }
        }
        false
    }

    /// Number of records currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear all records.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.head = 0;
        self.len = 0;
    }
}

impl Default for ActivationHistory {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}
