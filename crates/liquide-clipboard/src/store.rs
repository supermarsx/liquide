//! Local clipboard data store with size limits.

use std::collections::HashMap;

use crate::format::ClipboardFormat;
use crate::{ClipboardError, Result};

/// A single clipboard entry (format + data).
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub format: ClipboardFormat,
    pub data: Vec<u8>,
    pub timestamp_us: u64,
}

/// In-memory clipboard store keyed by format.
pub struct ClipboardStore {
    entries: HashMap<ClipboardFormat, ClipboardEntry>,
    owner_id: Option<u64>,
    max_total_bytes: usize,
    serial: u64,
}

impl ClipboardStore {
    /// Create a new store with a total byte limit.
    #[must_use]
    pub fn new(max_total_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            owner_id: None,
            max_total_bytes,
            serial: 0,
        }
    }

    /// Store data for a format.
    pub fn set(
        &mut self,
        format: ClipboardFormat,
        data: Vec<u8>,
        owner_id: u64,
        timestamp_us: u64,
    ) -> Result<()> {
        // Check total size after adding this entry
        let other_bytes: usize = self
            .entries
            .iter()
            .filter(|(k, _)| **k != format)
            .map(|(_, v)| v.data.len())
            .sum();
        let new_total = other_bytes + data.len();
        if new_total > self.max_total_bytes {
            return Err(ClipboardError::PayloadTooLarge {
                size: new_total,
                max: self.max_total_bytes,
            });
        }

        self.entries.insert(
            format.clone(),
            ClipboardEntry {
                format,
                data,
                timestamp_us,
            },
        );
        self.owner_id = Some(owner_id);
        self.serial += 1;
        Ok(())
    }

    /// Get data for a format.
    #[must_use]
    pub fn get(&self, format: &ClipboardFormat) -> Option<&[u8]> {
        self.entries.get(format).map(|e| e.data.as_slice())
    }

    /// List available formats.
    #[must_use]
    pub fn available_formats(&self) -> Vec<&ClipboardFormat> {
        self.entries.keys().collect()
    }

    /// Get the current owner.
    #[must_use]
    pub fn owner(&self) -> Option<u64> {
        self.owner_id
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.owner_id = None;
    }

    /// Total bytes stored.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.entries.values().map(|e| e.data.len()).sum()
    }

    /// Current serial number (incremented on each set).
    #[must_use]
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Check if a format is available.
    #[must_use]
    pub fn has_format(&self, format: &ClipboardFormat) -> bool {
        self.entries.contains_key(format)
    }

    /// Number of entries in the store.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get a full clipboard entry by format.
    #[must_use]
    pub fn get_entry(&self, format: &ClipboardFormat) -> Option<&ClipboardEntry> {
        self.entries.get(format)
    }
}
