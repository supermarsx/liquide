//! Storage events emitted by the storage subsystem.

use crate::device::StorageDevice;

/// Events related to storage device changes.
#[derive(Debug, Clone)]
pub enum StorageEvent {
    /// A new storage device was detected.
    DeviceAdded(StorageDevice),
    /// A storage device was removed (by device id).
    DeviceRemoved(String),
    /// A partition was mounted.
    PartitionMounted {
        partition_id: String,
        mount_point: String,
    },
    /// A partition was unmounted (by partition id).
    PartitionUnmounted(String),
    /// Available space on a partition has dropped below a threshold.
    SpaceLow {
        partition_id: String,
        available_bytes: u64,
        threshold_bytes: u64,
    },
}

impl StorageEvent {
    /// Returns `true` if this is a `SpaceLow` event.
    pub fn is_space_low(&self) -> bool {
        matches!(self, Self::SpaceLow { .. })
    }

    /// Returns `true` if this is a device add or remove event.
    pub fn is_device_event(&self) -> bool {
        matches!(self, Self::DeviceAdded(_) | Self::DeviceRemoved(_))
    }

    /// Returns `true` if this is a mount or unmount event.
    pub fn is_mount_event(&self) -> bool {
        matches!(
            self,
            Self::PartitionMounted { .. } | Self::PartitionUnmounted(_)
        )
    }
}

impl std::fmt::Display for StorageEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceAdded(dev) => write!(f, "device added: {} ({})", dev.name, dev.id),
            Self::DeviceRemoved(id) => write!(f, "device removed: {id}"),
            Self::PartitionMounted {
                partition_id,
                mount_point,
            } => write!(f, "partition {partition_id} mounted at {mount_point}"),
            Self::PartitionUnmounted(id) => write!(f, "partition {id} unmounted"),
            Self::SpaceLow {
                partition_id,
                available_bytes,
                threshold_bytes,
            } => write!(
                f,
                "low space on {partition_id}: {} available (threshold: {})",
                crate::analyzer::format_size(*available_bytes),
                crate::analyzer::format_size(*threshold_bytes),
            ),
        }
    }
}
