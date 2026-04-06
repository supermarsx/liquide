//! GPU-accelerated renderer for LiquiDE using `wgpu`.
//!
//! This crate provides hardware-accelerated rendering through the `wgpu`
//! abstraction layer, supporting:
//!
//! - **D3D12** (Windows)
//! - **Vulkan** (Linux, Windows, Android)
//! - **Metal** (macOS, iOS)
//! - **OpenGL / GLES** fallback
//!
//! The renderer consumes the same `SceneNode` / `FlatNode` types as the CPU
//! renderer and produces frames into GPU textures.
//!
//! # Architecture
//!
//! ```text
//! SceneNode tree ─► flatten() ─► FlatNode list ─► WgpuRenderer
//!                                                     │
//!                                    ┌────────────────┼────────────────┐
//!                                    │                │                │
//!                                 RectPipeline  BlurPipeline   BlendPipeline
//!                                    │                │                │
//!                                    └────────────────┼────────────────┘
//!                                                     │
//!                                               GPU Texture
//! ```

pub mod device;
pub mod pipeline;
pub mod renderer;
pub mod shader;
pub mod texture;

pub use device::{GpuBackend, WgpuDevice};
pub use renderer::{GlyphKey, GlyphMetrics, WgpuRenderer};

use thiserror::Error;

/// Errors from the wgpu renderer.
#[derive(Debug, Error)]
pub enum WgpuError {
    #[error("no suitable GPU adapter found")]
    NoAdapter,

    #[error("failed to request GPU device: {0}")]
    DeviceRequest(String),

    #[error("surface configuration failed: {0}")]
    SurfaceConfig(String),

    #[error("shader compilation failed: {0}")]
    ShaderCompilation(String),

    #[error("texture creation failed: {0}")]
    TextureCreation(String),

    #[error("frame rendering failed: {0}")]
    RenderFailed(String),
}

pub type Result<T> = std::result::Result<T, WgpuError>;
