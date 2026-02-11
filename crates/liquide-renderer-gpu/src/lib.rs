#![doc = "GPU-accelerated renderer using Vulkan compute shaders for the Liquide compositor."]
#![doc = ""]
#![doc = "Implements GPU-based rendering of scene graph primitives using Vulkan"]
#![doc = "compute pipelines for compositing, blur, shadow, and cursor rendering."]
#![doc = "This renderer must produce visually equivalent output to the CPU reference"]
#![doc = "renderer in `liquide-renderer-cpu`.  If no GPU is available or the device"]
#![doc = "is lost, rendering falls back to the CPU path transparently."]

pub mod audit;
pub mod blur;
pub mod composite;
pub mod device;
pub mod dmabuf;
pub mod fallback;
pub mod pipeline;
pub mod profile;
pub mod render_target;
pub mod renderer;
pub mod resource;
pub mod stats;

pub use renderer::{GpuRenderer, RenderedFrame};

use thiserror::Error;

/// Errors produced by the GPU renderer.
#[derive(Debug, Error)]
pub enum GpuRendererError {
    /// Invalid render target dimensions.
    #[error("invalid render target dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    /// Unsupported pixel format for this operation.
    #[error("unsupported pixel format: {0:?}")]
    UnsupportedFormat(liquide_compositor::PixelFormat),

    /// Vulkan device was lost and needs to be reset.
    #[error("vulkan device lost: {reason}")]
    DeviceLost { reason: String },

    /// Out of VRAM budget.
    #[error("VRAM budget exceeded: {allocated_mb}MB of {budget_mb}MB used")]
    OutOfVram { allocated_mb: u64, budget_mb: u64 },

    /// No suitable GPU device found.
    #[error("no suitable GPU device found")]
    NoDevice,

    /// Pipeline stage failure.
    #[error("pipeline stage {stage} failed: {reason}")]
    PipelineError { stage: String, reason: String },

    /// DMA-BUF import failure.
    #[error("DMA-BUF import failed: {0}")]
    DmaBufError(String),

    /// Render target pool exhausted.
    #[error("render target pool exhausted (active: {active})")]
    RenderTargetPoolExhausted { active: usize },

    /// Generic internal error.
    #[error("GPU render error: {0}")]
    Internal(String),
}

/// Convenience result type for GPU renderer operations.
pub type Result<T> = std::result::Result<T, GpuRendererError>;

#[cfg(test)]
mod tests;
