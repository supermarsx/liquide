//! DMA-BUF zero-copy import support.
//!
//! Manages the import of client surface buffers via Linux DMA-BUF file
//! descriptors, enabling zero-copy texture sharing between Wayland
//! clients and the GPU compositor.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata describing a DMA-BUF buffer to import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmaBufInfo {
    /// File descriptor of the DMA-BUF.
    pub fd: i32,
    /// Buffer width in pixels.
    pub width: u32,
    /// Buffer height in pixels.
    pub height: u32,
    /// Row stride in bytes.
    pub stride: u32,
    /// DRM fourcc pixel format code.
    pub format: u32,
    /// DRM format modifier (DRM_FORMAT_MOD_LINEAR, etc.).
    pub modifier: u64,
}

/// A tracked DMA-BUF import.
#[derive(Debug, Clone)]
pub struct DmaBufImport {
    /// The buffer metadata.
    pub info: DmaBufInfo,
    /// Whether the buffer has been successfully imported into Vulkan.
    pub imported: bool,
}

/// Manager for DMA-BUF imports.
///
/// Tracks imported buffers and their lifecycle.  In production this
/// would use `VK_EXT_external_memory_dma_buf` to import the file
/// descriptors as Vulkan images.
#[derive(Debug)]
pub struct DmaBufManager {
    /// Active imports keyed by a generated ID.
    imports: HashMap<String, DmaBufImport>,
    /// Whether DMA-BUF import is supported on this system.
    supported: bool,
    /// Counter for generating unique import IDs.
    next_id: u64,
}

impl DmaBufManager {
    /// Create a new DMA-BUF manager.
    ///
    /// The `supported` flag indicates whether the Vulkan device
    /// advertises `VK_EXT_external_memory_dma_buf`.
    #[must_use]
    pub fn new(supported: bool) -> Self {
        Self {
            imports: HashMap::new(),
            supported,
            next_id: 0,
        }
    }

    /// Import a DMA-BUF into the GPU renderer.
    ///
    /// Returns the import ID on success.  Fails if DMA-BUF is not
    /// supported or the buffer metadata is invalid.
    pub fn import(&mut self, info: DmaBufInfo) -> crate::Result<String> {
        if !self.supported {
            return Err(crate::GpuRendererError::DmaBufError(
                "DMA-BUF import not supported on this device".to_string(),
            ));
        }

        if info.width == 0 || info.height == 0 {
            return Err(crate::GpuRendererError::DmaBufError(format!(
                "invalid DMA-BUF dimensions: {}x{}",
                info.width, info.height
            )));
        }

        let id = format!("dmabuf-{}", self.next_id);
        self.next_id += 1;

        tracing::debug!(
            id = %id,
            fd = info.fd,
            width = info.width,
            height = info.height,
            format = info.format,
            "DMA-BUF imported"
        );

        let import = DmaBufImport {
            info,
            imported: true,
        };

        self.imports.insert(id.clone(), import);
        Ok(id)
    }

    /// Release a previously imported DMA-BUF.
    ///
    /// Returns `true` if the import was found and released.
    pub fn release(&mut self, id: &str) -> bool {
        let removed = self.imports.remove(id).is_some();
        if removed {
            tracing::debug!(id = %id, "DMA-BUF released");
        }
        removed
    }

    /// Whether DMA-BUF import is supported on this system.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Number of active imports.
    #[must_use]
    pub fn import_count(&self) -> usize {
        self.imports.len()
    }
}

impl Default for DmaBufManager {
    fn default() -> Self {
        Self::new(false)
    }
}
