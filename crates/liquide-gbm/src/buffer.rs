use crate::device::{BufferUsage, GbmDevice};
use crate::error::{GbmError, Result};
use crate::format::{DrmFourcc, DrmModifier};

/// Flags controlling GBM buffer allocation (mirrors `BufferUsage`).
pub type GbmBufferFlags = BufferUsage;

/// A GBM buffer object backed by GPU memory.
#[derive(Debug)]
pub struct GbmBuffer {
    /// Opaque handle returned by `gbm_bo_create`. Reserved for FFI.
    #[allow(dead_code)]
    handle: usize,
    width: u32,
    height: u32,
    stride: u32,
    format: DrmFourcc,
    modifier: DrmModifier,
}

impl GbmBuffer {
    /// Allocate a new GBM buffer.
    ///
    /// On non-Linux platforms this always returns `GbmError::NotSupported`.
    pub fn create(
        device: &GbmDevice,
        width: u32,
        height: u32,
        format: DrmFourcc,
        flags: GbmBufferFlags,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_bo_create via FFI
            let _ = (device, flags);
            tracing::debug!(width, height, format = format.name(), "allocating GBM buffer");
            Ok(Self {
                handle: 0,
                width,
                height,
                stride: width * 4,
                format,
                modifier: DrmModifier::LINEAR,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (device, width, height, format, flags);
            Err(GbmError::NotSupported)
        }
    }

    /// Export this buffer as a DMA-BUF file descriptor.
    pub fn export_dmabuf(&self) -> Result<i32> {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_bo_get_fd via FFI
            Err(GbmError::DmaBufExport(
                "FFI not yet implemented".into(),
            ))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(GbmError::NotSupported)
        }
    }

    /// Map the buffer for CPU write access.
    ///
    /// # Stub
    /// Not yet implemented — requires GBM FFI bindings (`gbm_bo_map`).
    pub fn map_write(&self) -> Result<BufferMapping> {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_bo_map via FFI
            Err(GbmError::BufferAlloc {
                width: self.width,
                height: self.height,
                format: self.format.name().to_string(),
                reason: "map not yet implemented".into(),
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(GbmError::NotSupported)
        }
    }

    pub fn stride(&self) -> u32 {
        self.stride
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

    pub fn modifier(&self) -> DrmModifier {
        self.modifier
    }

    #[allow(dead_code)] // will be used by FFI calls (gbm_bo_get_fd, etc.)
    pub(crate) fn handle(&self) -> usize {
        self.handle
    }
}

/// A CPU-mapped region of a GBM buffer.
///
/// The mapping is automatically unmapped on drop.
#[derive(Debug)]
pub struct BufferMapping {
    ptr: *mut u8,
    stride: u32,
    length: usize,
    /// Opaque cookie returned by `gbm_bo_map`, passed to `gbm_bo_unmap`.
    #[allow(dead_code)]
    map_data: usize,
}

impl BufferMapping {
    #[allow(dead_code)] // will be used when map_write() is implemented
    pub(crate) fn new(ptr: *mut u8, stride: u32, length: usize, map_data: usize) -> Self {
        Self {
            ptr,
            stride,
            length,
            map_data,
        }
    }

    /// Returns a raw pointer to the mapped memory.
    ///
    /// # Safety
    /// The caller must ensure writes through this pointer stay within
    /// `self.length` bytes and respect the stride layout.
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Row stride in bytes.
    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// Total mapped region length in bytes.
    pub fn length(&self) -> usize {
        self.length
    }
}

impl Drop for BufferMapping {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // TODO: call gbm_bo_unmap(map_data) via FFI
            tracing::debug!(map_data = self.map_data, "unmapping GBM buffer");
        }
    }
}
