use crate::buffer::GbmBuffer;
use crate::device::{GbmDevice, BufferUsage};
use crate::error::{GbmError, Result};
use crate::format::DrmFourcc;

/// A GBM surface used for page-flipping with DRM/KMS.
#[derive(Debug)]
pub struct GbmSurface {
    /// Opaque handle returned by `gbm_surface_create`. Reserved for FFI.
    #[allow(dead_code)]
    handle: usize,
    width: u32,
    height: u32,
    format: DrmFourcc,
}

impl GbmSurface {
    /// Create a new GBM surface for scanout.
    ///
    /// On non-Linux platforms this always returns `GbmError::NotSupported`.
    pub fn new(
        device: &GbmDevice,
        width: u32,
        height: u32,
        format: DrmFourcc,
        flags: BufferUsage,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_surface_create via FFI
            let _ = (device, flags);
            tracing::debug!(width, height, format = format.name(), "creating GBM surface");
            Ok(Self {
                handle: 0,
                width,
                height,
                format,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (device, width, height, format, flags);
            Err(GbmError::NotSupported)
        }
    }

    /// Lock the front buffer after a page flip, returning a `GbmBuffer`.
    pub fn lock_front_buffer(&self) -> Result<GbmBuffer> {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_surface_lock_front_buffer via FFI
            Err(GbmError::SurfaceLock(
                "FFI not yet implemented".into(),
            ))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(GbmError::NotSupported)
        }
    }

    /// Release a previously locked front buffer back to the surface.
    pub fn release_buffer(&self, buffer: GbmBuffer) {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_surface_release_buffer via FFI
            let _ = buffer;
            tracing::debug!(handle = self.handle, "releasing GBM surface buffer");
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = buffer;
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> DrmFourcc {
        self.format
    }
}

impl Drop for GbmSurface {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_surface_destroy(self.handle) via FFI
            tracing::debug!(handle = self.handle, "destroying GBM surface");
        }
    }
}
