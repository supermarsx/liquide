//! GPU blur implementation using Vulkan compute shaders.
//!
//! Provides a separable Gaussian blur engine that dispatches work to the GPU.
//! Supports configurable quality levels with downsample/upsample fast paths
//! for large radii.  SLO target: < 0.5 ms at 1080p.

use serde::{Deserialize, Serialize};

/// Quality level for GPU blur operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlurQuality {
    /// Full-resolution multi-pass Gaussian blur.
    Full,
    /// Balanced: single downsample + blur + upsample.
    Balanced,
    /// Aggressive downsampling with box filter approximation.
    Performance,
    /// Blur disabled — pass-through.
    Disabled,
}

impl Default for BlurQuality {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Parameters for a GPU blur operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlurParams {
    /// Blur radius in pixels.
    pub radius: f32,
    /// Gaussian sigma (standard deviation).
    pub sigma: f32,
    /// Quality level controlling the implementation strategy.
    pub quality: BlurQuality,
    /// Downsample factor for performance modes (1 = no downsampling).
    pub downsample_factor: u32,
}

impl Default for BlurParams {
    fn default() -> Self {
        Self {
            radius: 12.0,
            sigma: 4.0,
            quality: BlurQuality::Balanced,
            downsample_factor: 2,
        }
    }
}

impl BlurParams {
    /// Create blur parameters from a radius, computing sigma automatically.
    ///
    /// Sigma is set to `radius / 3.0` to match the CPU renderer's kernel
    /// truncation at 3 sigma.
    #[must_use]
    pub fn from_radius(radius: f32, quality: BlurQuality) -> Self {
        let sigma = radius / 3.0;
        let downsample_factor = match quality {
            BlurQuality::Full => 1,
            BlurQuality::Balanced => 2,
            BlurQuality::Performance => 4,
            BlurQuality::Disabled => 1,
        };
        Self {
            radius,
            sigma,
            quality,
            downsample_factor,
        }
    }
}

/// The result of a GPU blur computation.
#[derive(Debug, Clone)]
pub struct BlurResult {
    /// Output width in pixels.
    pub output_width: u32,
    /// Output height in pixels.
    pub output_height: u32,
    /// Time taken in microseconds.
    pub time_us: u64,
    /// Number of blur passes executed.
    pub passes: u32,
}

/// GPU blur engine.
///
/// Wraps the compute shader dispatch for Gaussian blur.  The actual
/// shader invocation is stubbed — this models the state and parameters
/// that would be fed to `vkCmdDispatch`.
#[derive(Debug)]
pub struct GpuBlur {
    /// Current blur parameters.
    params: BlurParams,
}

impl GpuBlur {
    /// Create a new GPU blur engine with the given parameters.
    #[must_use]
    pub fn new(params: BlurParams) -> Self {
        Self { params }
    }

    /// Compute blur for a region of the given dimensions.
    ///
    /// Returns metadata about the blur operation.  In production this
    /// would dispatch Vulkan compute shaders and synchronise with a fence.
    #[must_use]
    pub fn compute_blur(&self, input_width: u32, input_height: u32) -> BlurResult {
        if self.params.quality == BlurQuality::Disabled {
            return BlurResult {
                output_width: input_width,
                output_height: input_height,
                time_us: 0,
                passes: 0,
            };
        }

        let ds = self.params.downsample_factor.max(1);
        let work_width = input_width / ds;
        let work_height = input_height / ds;

        // Two passes for separable Gaussian (horizontal + vertical).
        let passes = if ds > 1 { 4 } else { 2 }; // +2 for downsample/upsample

        tracing::trace!(
            radius = self.params.radius,
            sigma = self.params.sigma,
            work_width,
            work_height,
            passes,
            "GPU blur dispatched"
        );

        BlurResult {
            output_width: input_width,
            output_height: input_height,
            time_us: 0, // would be GPU timestamp delta
            passes,
        }
    }

    /// Access the current blur parameters.
    #[must_use]
    pub fn params(&self) -> &BlurParams {
        &self.params
    }

    /// Update the blur parameters.
    pub fn set_params(&mut self, params: BlurParams) {
        self.params = params;
    }
}

impl Default for GpuBlur {
    fn default() -> Self {
        Self::new(BlurParams::default())
    }
}
