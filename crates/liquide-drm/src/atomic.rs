use crate::device::DrmDevice;
use crate::error::{DrmError, Result};

/// Bitflags for atomic commit operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicFlags(u32);

impl AtomicFlags {
    pub const NONBLOCK: Self = Self(1 << 0);
    pub const ALLOW_MODESET: Self = Self(1 << 1);
    pub const PAGE_FLIP_EVENT: Self = Self(1 << 2);
    pub const TEST_ONLY: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for AtomicFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for AtomicFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// A single property change within an atomic request.
#[derive(Debug, Clone)]
pub struct PropertyChange {
    pub object_id: u32,
    pub property_id: u32,
    pub value: u64,
}

/// An atomic modesetting request that batches property changes.
#[derive(Debug, Clone)]
pub struct AtomicRequest {
    changes: Vec<PropertyChange>,
}

impl AtomicRequest {
    /// Creates an empty atomic request.
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    /// Adds a property change to this request.
    pub fn add_property(&mut self, object_id: u32, property_id: u32, value: u64) {
        self.changes.push(PropertyChange {
            object_id,
            property_id,
            value,
        });
    }

    /// Returns the list of queued property changes.
    pub fn changes(&self) -> &[PropertyChange] {
        &self.changes
    }

    /// Commits the batched property changes to the DRM device.
    #[cfg(target_os = "linux")]
    pub fn commit(&self, _device: &DrmDevice, _flags: AtomicFlags) -> Result<()> {
        // TODO: implement via DRM_IOCTL_MODE_ATOMIC
        Err(DrmError::AtomicCommit("not yet implemented".to_string()))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn commit(&self, _device: &DrmDevice, _flags: AtomicFlags) -> Result<()> {
        Err(DrmError::NoDevice)
    }
}

impl Default for AtomicRequest {
    fn default() -> Self {
        Self::new()
    }
}
