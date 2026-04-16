use crate::error::{GbmError, Result};
use crate::format::DrmFourcc;

/// Usage flags for GBM buffer allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferUsage(u32);

impl BufferUsage {
    pub const SCANOUT: Self = Self(1 << 0);
    pub const CURSOR: Self = Self(1 << 1);
    pub const RENDERING: Self = Self(1 << 2);
    pub const WRITE: Self = Self(1 << 3);
    pub const LINEAR: Self = Self(1 << 4);

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

impl std::ops::BitOr for BufferUsage {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for BufferUsage {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// GBM device wrapping a DRM file descriptor.
#[derive(Debug)]
pub struct GbmDevice {
    /// Opaque handle returned by `gbm_create_device`. Reserved for FFI.
    #[allow(dead_code)]
    handle: usize,
    drm_fd: i32,
}

impl GbmDevice {
    /// Create a GBM device from a DRM file descriptor.
    ///
    /// On non-Linux platforms this always returns `GbmError::NotSupported`.
    pub fn new(drm_fd: i32) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_create_device(drm_fd) via FFI
            tracing::debug!(drm_fd, "creating GBM device");
            Ok(Self {
                handle: 0,
                drm_fd,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = drm_fd;
            Err(GbmError::NotSupported)
        }
    }

    /// Check whether a given format + usage combination is supported.
    pub fn is_format_supported(&self, format: DrmFourcc, usage: BufferUsage) -> bool {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_device_is_format_supported via FFI
            let _ = (format, usage);
            false
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (format, usage);
            false
        }
    }

    pub fn drm_fd(&self) -> i32 {
        self.drm_fd
    }

    #[allow(dead_code)] // will be used by FFI calls in buffer/surface creation
    pub(crate) fn handle(&self) -> usize {
        self.handle
    }
}

impl Drop for GbmDevice {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_device_destroy(self.handle) via FFI
            tracing::debug!(handle = self.handle, "destroying GBM device");
        }
    }
}
