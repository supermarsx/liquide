//! Central storage management facade.

use crate::analyzer::DiskUsage;
use crate::device::StorageDevice;
use crate::error::StorageError;
use crate::monitor::SpaceMonitor;
use crate::partition::Partition;
use crate::platform;

/// Central manager for storage devices, partitions, mount/unmount, and space
/// monitoring.
///
/// `StorageManager` provides a unified, platform-independent API for all
/// storage operations.
#[derive(Debug)]
pub struct StorageManager {
    /// The embedded space monitor.
    pub monitor: SpaceMonitor,
    /// Cached list of known devices (populated by `refresh()`).
    cached_devices: Vec<StorageDevice>,
}

impl StorageManager {
    /// Create a new `StorageManager`.
    pub fn new() -> Self {
        Self {
            monitor: SpaceMonitor::new(),
            cached_devices: Vec::new(),
        }
    }

    /// Enumerate all storage devices on the system.
    ///
    /// This calls into the platform backend (`lsblk`, PowerShell, `diskutil`)
    /// and returns a fresh snapshot. The result is also cached internally.
    pub fn list_devices(&mut self) -> Result<Vec<StorageDevice>, StorageError> {
        let devices = platform::list_devices()?;
        self.cached_devices = devices.clone();
        Ok(devices)
    }

    /// Return the most recently cached list of devices without querying the
    /// platform again. Call `list_devices()` first to populate this cache.
    pub fn cached_devices(&self) -> &[StorageDevice] {
        &self.cached_devices
    }

    /// List all currently mounted partitions across all devices.
    pub fn list_partitions(&self) -> Result<Vec<Partition>, StorageError> {
        platform::list_partitions()
    }

    /// Mount a partition at the given mount point.
    ///
    /// On Linux this calls `mount` or `udisksctl`. On macOS this calls
    /// `diskutil mount`. On Windows this returns `NotSupported` (Windows
    /// auto-assigns drive letters).
    pub fn mount(&self, partition_id: &str, mount_point: &str) -> Result<(), StorageError> {
        platform::mount_partition(partition_id, mount_point)
    }

    /// Unmount a partition.
    ///
    /// On Linux: `umount` / `udisksctl unmount`.
    /// On macOS: `diskutil unmount`.
    /// On Windows: `mountvol /P`.
    pub fn unmount(&self, partition_id: &str) -> Result<(), StorageError> {
        platform::unmount_partition(partition_id)
    }

    /// Safely eject a removable device.
    ///
    /// This first checks that the device is removable and does not contain a
    /// system partition, then calls the platform eject command.
    pub fn eject(&self, device_id: &str) -> Result<(), StorageError> {
        // Check cached devices for safety.
        if let Some(dev) = self.cached_devices.iter().find(|d| d.id == device_id) {
            if !dev.removable {
                return Err(StorageError::CannotEject(
                    "device is not removable".to_string(),
                ));
            }
            if dev.has_system_partition() {
                return Err(StorageError::CannotEject(
                    "device contains system partition".to_string(),
                ));
            }
        }
        platform::eject_device(device_id)
    }

    /// Query disk usage for a given filesystem path (mount point or any file
    /// path on the volume).
    pub fn disk_usage(&self, path: &str) -> Result<DiskUsage, StorageError> {
        platform::disk_usage(path)
    }

    /// Refresh the device cache by re-enumerating. Returns the fresh device
    /// list.
    pub fn refresh(&mut self) -> Result<Vec<StorageDevice>, StorageError> {
        self.list_devices()
    }

    /// Find a device by its id in the cache.
    pub fn find_device(&self, device_id: &str) -> Option<&StorageDevice> {
        self.cached_devices.iter().find(|d| d.id == device_id)
    }

    /// Find a partition by its id across all cached devices.
    pub fn find_partition(&self, partition_id: &str) -> Option<&Partition> {
        self.cached_devices
            .iter()
            .flat_map(|d| d.partitions.iter())
            .find(|p| p.id == partition_id)
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}
