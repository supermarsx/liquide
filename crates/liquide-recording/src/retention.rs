//! Retention policy — controls recording lifecycle based on age, size, and count.

use serde::{Deserialize, Serialize};

/// Policy controlling when recordings should be deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Maximum age in hours before deletion.
    pub max_age_hours: Option<u64>,
    /// Maximum total size in bytes across all recordings.
    pub max_size_bytes: Option<u64>,
    /// Maximum number of recordings to keep.
    pub max_recordings: Option<u32>,
}

impl RetentionPolicy {
    /// Create a policy with no limits.
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            max_age_hours: None,
            max_size_bytes: None,
            max_recordings: None,
        }
    }

    /// Check if a single entry should be deleted based on age.
    #[must_use]
    pub fn should_delete(&self, entry: &RecordingEntry, now_us: u64) -> bool {
        if let Some(max_hours) = self.max_age_hours {
            let max_age_us = max_hours * 3_600_000_000;
            if now_us.saturating_sub(entry.created_us) > max_age_us {
                return true;
            }
        }
        false
    }

    /// Enforce the policy on a list of entries, returning IDs to delete.
    ///
    /// Entries should be sorted oldest-first. Deletions happen by age first,
    /// then by count, then by total size.
    #[must_use]
    pub fn enforce(&self, entries: &[RecordingEntry], now_us: u64) -> Vec<String> {
        let mut to_delete = Vec::new();
        let mut remaining: Vec<&RecordingEntry> = Vec::new();

        // Age-based deletion
        for entry in entries {
            if self.should_delete(entry, now_us) {
                to_delete.push(entry.id.clone());
            } else {
                remaining.push(entry);
            }
        }

        // Count-based deletion (remove oldest first)
        if let Some(max_count) = self.max_recordings {
            while remaining.len() > max_count as usize {
                if let Some(entry) = remaining.first() {
                    to_delete.push(entry.id.clone());
                }
                remaining.remove(0);
            }
        }

        // Size-based deletion (remove oldest first until under limit)
        if let Some(max_size) = self.max_size_bytes {
            let mut total_size: u64 = remaining.iter().map(|e| e.size_bytes).sum();
            while total_size > max_size && !remaining.is_empty() {
                let entry = remaining.remove(0);
                total_size -= entry.size_bytes;
                to_delete.push(entry.id.clone());
            }
        }

        to_delete
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::unlimited()
    }
}

impl std::fmt::Display for RetentionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RetentionPolicy(age={:?}h, size={:?}B, count={:?})",
            self.max_age_hours, self.max_size_bytes, self.max_recordings
        )
    }
}

/// A recording entry for retention evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingEntry {
    /// Unique recording ID.
    pub id: String,
    /// Creation timestamp in microseconds.
    pub created_us: u64,
    /// Size of this recording in bytes.
    pub size_bytes: u64,
}

impl RecordingEntry {
    /// Create a new recording entry.
    #[must_use]
    pub fn new(id: &str, created_us: u64, size_bytes: u64) -> Self {
        Self {
            id: id.to_string(),
            created_us,
            size_bytes,
        }
    }
}

impl std::fmt::Display for RecordingEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RecordingEntry({}, {} bytes)", self.id, self.size_bytes)
    }
}
