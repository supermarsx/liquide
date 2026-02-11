//! Alpha compositing operations for the GPU pipeline.
//!
//! Defines Porter-Duff compositing operators and the task descriptions
//! submitted to the GPU for batch compositing of surface regions.

use serde::{Deserialize, Serialize};

/// Porter-Duff compositing operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositeOp {
    /// Source over destination (default).
    SrcOver,
    /// Source clipped to destination alpha.
    SrcIn,
    /// Source where destination is transparent.
    SrcOut,
    /// Destination over source.
    DstOver,
    /// Destination clipped to source alpha.
    DstIn,
    /// Clear the region to transparent.
    Clear,
    /// Copy source to destination (ignore destination).
    Copy,
}

impl Default for CompositeOp {
    fn default() -> Self {
        Self::SrcOver
    }
}

impl std::fmt::Display for CompositeOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SrcOver => write!(f, "src-over"),
            Self::SrcIn => write!(f, "src-in"),
            Self::SrcOut => write!(f, "src-out"),
            Self::DstOver => write!(f, "dst-over"),
            Self::DstIn => write!(f, "dst-in"),
            Self::Clear => write!(f, "clear"),
            Self::Copy => write!(f, "copy"),
        }
    }
}

/// A rectangular region on a surface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CompositeRegion {
    /// X origin in pixels.
    pub x: u32,
    /// Y origin in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl CompositeRegion {
    /// Create a new composite region.
    #[must_use]
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Area of the region in pixels.
    #[must_use]
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// A single compositing task to be executed on the GPU.
#[derive(Debug, Clone)]
pub struct CompositeTask {
    /// The compositing operation to apply.
    pub op: CompositeOp,
    /// Source region to read from.
    pub src_region: CompositeRegion,
    /// Destination region to write to.
    pub dst_region: CompositeRegion,
    /// Global alpha multiplier (0.0 = transparent, 1.0 = opaque).
    pub alpha: f32,
}

impl CompositeTask {
    /// Create a new compositing task with default SrcOver operation.
    #[must_use]
    pub fn new(
        src_region: CompositeRegion,
        dst_region: CompositeRegion,
        alpha: f32,
    ) -> Self {
        Self {
            op: CompositeOp::SrcOver,
            src_region,
            dst_region,
            alpha: alpha.clamp(0.0, 1.0),
        }
    }
}

/// GPU compositing engine.
///
/// Batches compositing tasks and dispatches them as compute shader
/// invocations.  This implementation stubs the GPU dispatch — in
/// production it would record Vulkan command buffers.
#[derive(Debug)]
pub struct GpuCompositor {
    /// Number of compositing operations executed.
    ops_executed: u64,
}

impl GpuCompositor {
    /// Create a new GPU compositor.
    #[must_use]
    pub fn new() -> Self {
        Self { ops_executed: 0 }
    }

    /// Execute a batch of compositing tasks on the GPU.
    ///
    /// In production this would record and submit a Vulkan command buffer
    /// containing compute shader dispatches for each task.
    pub fn composite(&mut self, tasks: &[CompositeTask]) -> crate::Result<()> {
        if tasks.is_empty() {
            return Ok(());
        }

        tracing::trace!(task_count = tasks.len(), "dispatching composite tasks");

        for task in tasks {
            tracing::trace!(
                op = %task.op,
                alpha = task.alpha,
                src_area = task.src_region.area(),
                "compositing region"
            );
        }

        self.ops_executed += tasks.len() as u64;
        Ok(())
    }

    /// Total number of compositing operations executed.
    #[must_use]
    pub fn ops_executed(&self) -> u64 {
        self.ops_executed
    }
}

impl Default for GpuCompositor {
    fn default() -> Self {
        Self::new()
    }
}
