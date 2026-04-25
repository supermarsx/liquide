use crate::device::DrmDevice;
use crate::error::{DrmError, Result};

/// A DRM framebuffer object backed by a GEM/dumb buffer.
///
/// Created via [`DrmFramebuffer::create`]; destroyed automatically on drop.
#[derive(Debug)]
pub struct DrmFramebuffer {
    /// KMS framebuffer object ID.
    pub id: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row stride in bytes.
    pub stride: u32,
    /// Bits per pixel.
    pub bpp: u32,
    /// Colour depth.
    pub depth: u32,
    /// GEM buffer handle (driver-private).
    #[allow(dead_code)] // used by the pending DRM ioctl implementation
    handle: u32,
    #[allow(dead_code)] // used by the pending Drop implementation for RMFB ioctl
    device_fd: i32,
}

impl DrmFramebuffer {
    /// Creates a new DRM framebuffer (dumb buffer + fb object).
    #[cfg(target_os = "linux")]
    pub fn create(
        device: &DrmDevice,
        width: u32,
        height: u32,
        bpp: u32,
        depth: u32,
    ) -> Result<Self> {
        // TODO: implement via DRM_IOCTL_MODE_CREATE_DUMB + DRM_IOCTL_MODE_ADDFB
        let _ = (device, width, height, bpp, depth);
        Err(DrmError::BufferAlloc("not yet implemented".to_string()))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn create(
        _device: &DrmDevice,
        _width: u32,
        _height: u32,
        _bpp: u32,
        _depth: u32,
    ) -> Result<Self> {
        Err(DrmError::NoDevice)
    }

    /// Memory-maps the framebuffer for CPU writes.
    #[cfg(target_os = "linux")]
    pub fn map(&self, device: &DrmDevice) -> Result<*mut u8> {
        // TODO: implement via DRM_IOCTL_MODE_MAP_DUMB + mmap
        let _ = device;
        Err(DrmError::BufferAlloc("map not yet implemented".to_string()))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn map(&self, _device: &DrmDevice) -> Result<*mut u8> {
        Err(DrmError::NoDevice)
    }
}

impl Drop for DrmFramebuffer {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // TODO: DRM_IOCTL_MODE_RMFB + DRM_IOCTL_MODE_DESTROY_DUMB
            // SAFETY: device_fd would be passed to DRM ioctls here to release the
            // framebuffer and destroy the dumb buffer. Currently a no-op stub.
            let _ = self.device_fd;
        }
    }
}
