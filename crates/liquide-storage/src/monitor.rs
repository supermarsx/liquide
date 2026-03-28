//! Low disk space monitoring.

use crate::error::StorageError;
use crate::event::StorageEvent;
use crate::platform;

/// Default low-space threshold: 1 GiB.
pub const DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024 * 1024;

/// A watch entry tracking a partition and its low-space threshold.
#[derive(Debug, Clone)]
struct Watch {
    partition_id: String,
    threshold_bytes: u64,
}

/// Monitors partitions for low disk space.
///
/// Add watches with `add_watch()`, then call `check_all()` periodically to
/// get `StorageEvent::SpaceLow` events for any partition whose available space
/// has dropped below its configured threshold.
#[derive(Debug)]
pub struct SpaceMonitor {
    watches: Vec<Watch>,
}

impl SpaceMonitor {
    /// Create a new, empty `SpaceMonitor`.
    pub fn new() -> Self {
        Self {
            watches: Vec::new(),
        }
    }

    /// Add a watch for the given partition with a custom threshold.
    ///
    /// If a watch already exists for this partition, the threshold is updated.
    pub fn add_watch(&mut self, partition_id: &str, threshold_bytes: u64) {
        if let Some(existing) = self
            .watches
            .iter_mut()
            .find(|w| w.partition_id == partition_id)
        {
            existing.threshold_bytes = threshold_bytes;
        } else {
            self.watches.push(Watch {
                partition_id: partition_id.to_string(),
                threshold_bytes,
            });
        }
    }

    /// Add a watch for the given partition with the default threshold (1 GiB).
    pub fn add_watch_default(&mut self, partition_id: &str) {
        self.add_watch(partition_id, DEFAULT_THRESHOLD_BYTES);
    }

    /// Remove a watch for the given partition.
    ///
    /// Returns `true` if a watch was found and removed.
    pub fn remove_watch(&mut self, partition_id: &str) -> bool {
        let before = self.watches.len();
        self.watches.retain(|w| w.partition_id != partition_id);
        self.watches.len() < before
    }

    /// Returns the number of active watches.
    pub fn watch_count(&self) -> usize {
        self.watches.len()
    }

    /// Check all watched partitions and return `SpaceLow` events for any
    /// whose available space is below their threshold.
    ///
    /// This queries the platform for current disk usage for each watched
    /// partition. Partitions that cannot be queried (e.g., unmounted) are
    /// silently skipped.
    pub fn check_all(&self) -> Vec<StorageEvent> {
        let mut events = Vec::new();

        for watch in &self.watches {
            match platform::query_partition_usage(&watch.partition_id) {
                Ok((_total, available)) => {
                    if available < watch.threshold_bytes {
                        events.push(StorageEvent::SpaceLow {
                            partition_id: watch.partition_id.clone(),
                            available_bytes: available,
                            threshold_bytes: watch.threshold_bytes,
                        });
                    }
                }
                Err(_) => {
                    // Skip partitions we cannot query (unmounted, permission error, etc.).
                }
            }
        }

        events
    }

    /// Check a single watched partition.
    ///
    /// Returns `Some(StorageEvent::SpaceLow { .. })` if below threshold,
    /// `None` if above threshold, or an error if the partition cannot be queried.
    pub fn check_one(&self, partition_id: &str) -> Result<Option<StorageEvent>, StorageError> {
        let watch = self
            .watches
            .iter()
            .find(|w| w.partition_id == partition_id)
            .ok_or_else(|| {
                StorageError::PartitionNotFound(format!("no watch for {partition_id}"))
            })?;

        let (_total, available) = platform::query_partition_usage(&watch.partition_id)?;
        if available < watch.threshold_bytes {
            Ok(Some(StorageEvent::SpaceLow {
                partition_id: watch.partition_id.clone(),
                available_bytes: available,
                threshold_bytes: watch.threshold_bytes,
            }))
        } else {
            Ok(None)
        }
    }
}

impl Default for SpaceMonitor {
    fn default() -> Self {
        Self::new()
    }
}
