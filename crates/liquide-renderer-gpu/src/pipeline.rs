//! GPU compositing pipeline definition and frame execution.
//!
//! Defines the ordered stages of the GPU rendering pipeline — from scene
//! graph traversal through alpha compositing, blur, shadows, cursor overlay,
//! and final framebuffer write-back.

use serde::{Deserialize, Serialize};

/// A stage in the GPU compositing pipeline.
///
/// Stages execute in the order defined by their discriminant values,
/// matching the frame pipeline from the spec: scene graph -> rounded rects ->
/// alpha compositing -> blur -> shadows -> cursor -> framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineStage {
    /// Traverse and flatten the scene graph.
    SceneTraversal,
    /// Render rounded rectangles (decorations, panels).
    RoundedRects,
    /// Perform alpha compositing of surfaces.
    AlphaComposite,
    /// Apply Gaussian blur for glass and backdrop effects.
    Blur,
    /// Render drop shadows and box shadows.
    Shadows,
    /// Composite the hardware or software cursor.
    Cursor,
    /// Final pass: write to the output framebuffer.
    Finalize,
}

impl std::fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SceneTraversal => write!(f, "scene-traversal"),
            Self::RoundedRects => write!(f, "rounded-rects"),
            Self::AlphaComposite => write!(f, "alpha-composite"),
            Self::Blur => write!(f, "blur"),
            Self::Shadows => write!(f, "shadows"),
            Self::Cursor => write!(f, "cursor"),
            Self::Finalize => write!(f, "finalize"),
        }
    }
}

/// Quality level for the blur pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlurQuality {
    /// Full-resolution multi-pass Gaussian blur.
    Full,
    /// Balanced: single downsample + blur + upsample.
    Balanced,
    /// Performance: aggressive downsampling, box filter.
    Performance,
    /// Blur disabled entirely.
    Disabled,
}

impl Default for BlurQuality {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Configuration for the GPU compositing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Blur quality level.
    pub blur_quality: BlurQuality,
    /// Whether shadow rendering is enabled.
    pub shadow_enabled: bool,
    /// Maximum allowed blur radius in pixels.
    pub max_blur_radius: u32,
    /// Whether hardware cursor compositing is enabled.
    pub enable_cursor_hw: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            blur_quality: BlurQuality::Balanced,
            shadow_enabled: true,
            max_blur_radius: 64,
            enable_cursor_hw: true,
        }
    }
}

/// The result of executing a single frame through the pipeline.
#[derive(Debug, Clone)]
pub struct FrameResult {
    /// Width of the rendered frame in pixels.
    pub width: u32,
    /// Height of the rendered frame in pixels.
    pub height: u32,
    /// Monotonically increasing frame identifier.
    pub frame_id: u64,
    /// Time spent in the alpha compositing stage in microseconds.
    pub composite_time_us: u64,
    /// Total time for the entire pipeline in microseconds.
    pub total_time_us: u64,
}

/// The GPU compositing pipeline.
///
/// Manages an ordered list of pipeline stages and executes them
/// sequentially for each frame.
#[derive(Debug)]
pub struct ComputePipeline {
    /// Pipeline configuration.
    config: PipelineConfig,
    /// Ordered list of active stages.
    stages: Vec<PipelineStage>,
    /// Total number of frames executed.
    frame_count: u64,
}

impl ComputePipeline {
    /// Create a new pipeline with the given configuration.
    ///
    /// The stage list is built based on the configuration: disabled
    /// features (e.g. blur, shadows, cursor) are excluded.
    #[must_use]
    pub fn new(config: PipelineConfig) -> Self {
        let mut stages = vec![
            PipelineStage::SceneTraversal,
            PipelineStage::RoundedRects,
            PipelineStage::AlphaComposite,
        ];

        if config.blur_quality != BlurQuality::Disabled {
            stages.push(PipelineStage::Blur);
        }

        if config.shadow_enabled {
            stages.push(PipelineStage::Shadows);
        }

        if config.enable_cursor_hw {
            stages.push(PipelineStage::Cursor);
        }

        stages.push(PipelineStage::Finalize);

        Self {
            config,
            stages,
            frame_count: 0,
        }
    }

    /// Execute the pipeline for one frame.
    ///
    /// In a real implementation this would dispatch Vulkan compute shaders.
    /// This version simulates the pipeline by recording stage execution
    /// and returning timing metadata.
    pub fn execute_frame(
        &mut self,
        _scene: &[liquide_compositor::FlatNode],
        width: u32,
        height: u32,
    ) -> crate::Result<FrameResult> {
        if width == 0 || height == 0 {
            return Err(crate::GpuRendererError::InvalidDimensions { width, height });
        }

        self.frame_count += 1;

        tracing::trace!(
            frame_id = self.frame_count,
            stages = self.stages.len(),
            "executing GPU pipeline"
        );

        // Simulate stage timing — in production these would be GPU timestamps.
        let composite_time_us = 0;
        let total_time_us = 0;

        Ok(FrameResult {
            width,
            height,
            frame_id: self.frame_count,
            composite_time_us,
            total_time_us,
        })
    }

    /// Number of stages in the current pipeline.
    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// The ordered list of active stages.
    #[must_use]
    pub fn stages(&self) -> &[PipelineStage] {
        &self.stages
    }

    /// Pipeline configuration.
    #[must_use]
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Total number of frames executed since creation.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Default for ComputePipeline {
    fn default() -> Self {
        Self::new(PipelineConfig::default())
    }
}
