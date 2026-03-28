//! Partition / volume representation.

use crate::filesystem::FileSystem;

/// A partition or volume on a storage device.
#[derive(Debug, Clone)]
pub struct Partition {
    /// Partition path or identifier (e.g., "/dev/sda1", "C:").
    pub id: String,
    /// Filesystem label, if set.
    pub label: Option<String>,
    /// Detected filesystem type.
    pub filesystem: FileSystem,
    /// Current mount point, if mounted.
    pub mount_point: Option<String>,
    /// Total partition size in bytes.
    pub size_bytes: u64,
    /// Bytes currently used.
    pub used_bytes: u64,
    /// Bytes available for use.
    pub available_bytes: u64,
    /// Filesystem UUID, if available.
    pub uuid: Option<String>,
    /// Whether this partition contains the operating system.
    pub is_system: bool,
    /// Whether the partition is encrypted.
    pub is_encrypted: bool,
}

impl Partition {
    /// Usage as a percentage (0.0 to 100.0).
    ///
    /// Returns 0.0 if the partition has zero size.
    pub fn usage_percent(&self) -> f32 {
        if self.size_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f64 / self.size_bytes as f64 * 100.0) as f32
    }

    /// Returns `true` if the partition is currently mounted.
    pub fn is_mounted(&self) -> bool {
        self.mount_point.is_some()
    }

    /// Human-readable string for the partition size.
    pub fn formatted_size(&self) -> String {
        crate::analyzer::format_size(self.size_bytes)
    }

    /// Human-readable string for the used space.
    pub fn formatted_used(&self) -> String {
        crate::analyzer::format_size(self.used_bytes)
    }

    /// Human-readable string for the available space.
    pub fn formatted_available(&self) -> String {
        crate::analyzer::format_size(self.available_bytes)
    }
}
