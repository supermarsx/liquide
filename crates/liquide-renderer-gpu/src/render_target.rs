//! Render target management and pooling.
//!
//! Manages a pool of GPU render targets (Vulkan images) that can be
//! acquired for rendering and released back when no longer needed.
//! Pooling avoids the cost of repeated image creation/destruction.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Pixel format of a GPU render target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RenderTargetFormat {
    /// 8-bit BGRA, unorm (matches Wayland/X11 default).
    Bgra8Unorm,
    /// 8-bit RGBA, unorm.
    Rgba8Unorm,
    /// 8-bit BGRA, sRGB.
    Bgra8Srgb,
    /// 16-bit RGBA, float (HDR intermediate).
    Rgba16Float,
    /// 10-bit RGB + 2-bit alpha, unorm (HDR output).
    Rgb10A2Unorm,
}

impl Default for RenderTargetFormat {
    fn default() -> Self {
        Self::Bgra8Unorm
    }
}

impl std::fmt::Display for RenderTargetFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bgra8Unorm => write!(f, "BGRA8_UNORM"),
            Self::Rgba8Unorm => write!(f, "RGBA8_UNORM"),
            Self::Bgra8Srgb => write!(f, "BGRA8_SRGB"),
            Self::Rgba16Float => write!(f, "RGBA16_FLOAT"),
            Self::Rgb10A2Unorm => write!(f, "RGB10A2_UNORM"),
        }
    }
}

/// A GPU render target (backed by a Vulkan image in production).
#[derive(Debug, Clone)]
pub struct RenderTarget {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: RenderTargetFormat,
    /// Unique identifier for this render target.
    pub id: String,
}

impl RenderTarget {
    /// Size of this render target in bytes (uncompressed).
    #[must_use]
    pub fn size_bytes(&self) -> u64 {
        let bpp: u64 = match self.format {
            RenderTargetFormat::Bgra8Unorm
            | RenderTargetFormat::Rgba8Unorm
            | RenderTargetFormat::Bgra8Srgb
            | RenderTargetFormat::Rgb10A2Unorm => 4,
            RenderTargetFormat::Rgba16Float => 8,
        };
        self.width as u64 * self.height as u64 * bpp
    }
}

/// Pool of reusable GPU render targets.
#[derive(Debug)]
pub struct RenderTargetPool {
    /// Active render targets keyed by ID.
    pool: HashMap<String, RenderTarget>,
    /// Counter for generating unique IDs.
    next_id: u64,
}

impl RenderTargetPool {
    /// Create a new empty render target pool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pool: HashMap::new(),
            next_id: 0,
        }
    }

    /// Acquire a render target with the given dimensions and format.
    ///
    /// In production this would either reuse a pooled Vulkan image or
    /// create a new one via `vkCreateImage`.
    pub fn acquire(
        &mut self,
        width: u32,
        height: u32,
        format: RenderTargetFormat,
    ) -> crate::Result<RenderTarget> {
        if width == 0 || height == 0 {
            return Err(crate::GpuRendererError::InvalidDimensions { width, height });
        }

        let id = format!("rt-{}", self.next_id);
        self.next_id += 1;

        let target = RenderTarget {
            width,
            height,
            format,
            id: id.clone(),
        };

        tracing::debug!(
            id = %id,
            width,
            height,
            format = %format,
            "render target acquired"
        );

        self.pool.insert(id, target.clone());
        Ok(target)
    }

    /// Release a render target back to the pool.
    ///
    /// Returns `true` if the target was found and released.
    pub fn release(&mut self, id: &str) -> bool {
        let removed = self.pool.remove(id).is_some();
        if removed {
            tracing::debug!(id = %id, "render target released");
        }
        removed
    }

    /// Number of currently active (acquired) render targets.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.pool.len()
    }
}

impl Default for RenderTargetPool {
    fn default() -> Self {
        Self::new()
    }
}
