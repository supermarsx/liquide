//! Zero-copy framebuffer import handles for GPU memory sharing.

/// DMA-BUF handle for VAAPI zero-copy import (Linux).
#[derive(Debug, Clone, Copy)]
pub struct DmaBufHandle {
    /// File descriptor for the DMA-BUF.
    pub fd: i32,
    /// Byte offset into the DMA-BUF.
    pub offset: u64,
    /// Row stride in bytes.
    pub stride: u32,
    /// Total size in bytes.
    pub size: u64,
}

/// CUDA device pointer handle for NVENC zero-copy import.
#[derive(Debug, Clone, Copy)]
pub struct CudaHandle {
    /// CUDA device pointer.
    pub device_ptr: u64,
    /// Allocation size in bytes.
    pub size: u64,
}

/// Vulkan memory handle for AMF/V4L2 zero-copy import.
#[derive(Debug, Clone, Copy)]
pub struct VulkanHandle {
    /// Vulkan device memory handle.
    pub memory: u64,
    /// Byte offset into the memory.
    pub offset: u64,
    /// Allocation size in bytes.
    pub size: u64,
    /// Vulkan image handle.
    pub image: u64,
}

/// Trait for importing GPU memory handles into an encoder session.
pub trait ZeroCopyImport {
    /// Import a DMA-BUF into the encoder.
    fn import_dmabuf(&mut self, handle: &DmaBufHandle) -> crate::Result<()>;

    /// Import CUDA device memory into the encoder.
    fn import_cuda(&mut self, handle: &CudaHandle) -> crate::Result<()>;

    /// Import Vulkan memory into the encoder.
    fn import_vulkan(&mut self, handle: &VulkanHandle) -> crate::Result<()>;
}
