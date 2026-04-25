//! Main GPU renderer — the public interface to the GPU rendering subsystem.
//!
//! `GpuRenderer` mirrors the role of `SoftwareRenderer` in
//! `liquide-renderer-cpu`, managing the GPU device, pipeline, VRAM
//! allocator, and fallback state.  It produces `RenderedFrame` values
//! that the encoder can consume.

use crate::audit::GpuAuditEvent;
use crate::device::GpuDevice;
use crate::dmabuf::DmaBufManager;
use crate::fallback::{FallbackManager, FallbackReason};
use crate::pipeline::{ComputePipeline, PipelineConfig};
use crate::profile::GpuProfile;
use crate::render_target::RenderTargetPool;
use crate::resource::{VramAllocator, VramBudget};
use crate::stats::{GpuFrameStats, StatsCollector};

/// A rendered frame produced by the GPU renderer.
#[derive(Debug, Clone)]
pub struct RenderedFrame {
    /// Width of the rendered frame in pixels.
    pub width: u32,
    /// Height of the rendered frame in pixels.
    pub height: u32,
    /// Monotonically increasing frame identifier.
    pub frame_id: u64,
    /// Total render time in microseconds.
    pub render_time_us: u64,
}

/// The GPU renderer.
///
/// Owns all GPU subsystems and coordinates frame rendering.  When the
/// GPU is unavailable or encounters an error, the renderer activates
/// CPU fallback transparently.
#[derive(Debug)]
pub struct GpuRenderer {
    /// The GPU device handle.
    device: GpuDevice,
    /// Selected GPU profile.
    profile: GpuProfile,
    /// VRAM allocation manager.
    vram_allocator: VramAllocator,
    /// Compute pipeline.
    pipeline: ComputePipeline,
    /// Render target pool.
    render_target_pool: RenderTargetPool,
    /// DMA-BUF import manager.
    dmabuf_manager: DmaBufManager,
    /// CPU fallback manager.
    fallback: FallbackManager,
    /// Performance statistics collector.
    stats: StatsCollector,
    /// Pending audit events to be drained by the session.
    audit_events: Vec<GpuAuditEvent>,
    /// Current output width.
    width: u32,
    /// Current output height.
    height: u32,
}

impl GpuRenderer {
    /// Create a new GPU renderer with the given device, profile, and config.
    #[must_use]
    pub fn new(device: GpuDevice, profile: GpuProfile, config: PipelineConfig) -> Self {
        let supports_dmabuf = device.capabilities().supports_dmabuf;
        let vram_budget = VramBudget {
            total_mb: device.vram_total(),
            allocated_mb: 0,
            session_budget_mb: 256,
        };

        Self {
            device,
            profile,
            vram_allocator: VramAllocator::new(vram_budget),
            pipeline: ComputePipeline::new(config),
            render_target_pool: RenderTargetPool::new(),
            dmabuf_manager: DmaBufManager::new(supports_dmabuf),
            fallback: FallbackManager::new(),
            stats: StatsCollector::new(),
            audit_events: Vec::new(),
            width: 0,
            height: 0,
        }
    }

    /// Render a frame from the given flattened scene graph.
    ///
    /// If the GPU pipeline fails, the fallback manager is activated and
    /// an error is returned so the caller can dispatch to the CPU renderer.
    pub fn render_frame(
        &mut self,
        scene: &[liquide_compositor::FlatNode],
        width: u32,
        height: u32,
    ) -> crate::Result<RenderedFrame> {
        if width == 0 || height == 0 {
            return Err(crate::GpuRendererError::InvalidDimensions { width, height });
        }

        if self.fallback.is_active() {
            return Err(crate::GpuRendererError::Internal(
                "GPU renderer is in fallback mode".to_string(),
            ));
        }

        self.width = width;
        self.height = height;

        // Execute the pipeline.
        match self.pipeline.execute_frame(scene, width, height) {
            Ok(frame_result) => {
                let frame_stats = GpuFrameStats {
                    composite_time_us: frame_result.composite_time_us,
                    blur_time_us: 0,
                    total_time_us: frame_result.total_time_us,
                    vram_used_mb: self.vram_allocator.budget().allocated_mb,
                    frame_id: frame_result.frame_id,
                };
                self.stats.record_frame(frame_stats);

                self.audit_events.push(GpuAuditEvent::FrameRendered {
                    time_us: frame_result.total_time_us,
                });

                // Check VRAM pressure.
                let usage = self.vram_allocator.usage_pct();
                if usage > 90.0 {
                    self.audit_events
                        .push(GpuAuditEvent::VramWarning { used_pct: usage });
                }

                Ok(RenderedFrame {
                    width: frame_result.width,
                    height: frame_result.height,
                    frame_id: frame_result.frame_id,
                    render_time_us: frame_result.total_time_us,
                })
            }
            Err(e) => {
                // Activate fallback on device loss.
                let reason = match &e {
                    crate::GpuRendererError::DeviceLost { .. } => {
                        self.stats.record_device_lost();
                        FallbackReason::DeviceLost
                    }
                    crate::GpuRendererError::OutOfVram { .. } => FallbackReason::OutOfVram,
                    _ => FallbackReason::DriverError(e.to_string()),
                };

                self.fallback.activate(reason.clone());
                self.stats.record_fallback();

                self.audit_events.push(GpuAuditEvent::FallbackActivated {
                    reason: reason.to_string(),
                });

                Err(e)
            }
        }
    }

    /// Resize the renderer output.
    pub fn resize(&mut self, width: u32, height: u32) {
        tracing::debug!(width, height, "GPU renderer resized");
        self.width = width;
        self.height = height;
    }

    /// The currently selected GPU profile.
    #[must_use]
    pub fn current_profile(&self) -> GpuProfile {
        self.profile
    }

    /// Access aggregate rendering statistics.
    #[must_use]
    pub fn stats(&self) -> crate::stats::GpuRenderStats {
        self.stats.summary()
    }

    /// Whether the renderer is currently in CPU fallback mode.
    #[must_use]
    pub fn is_fallback_active(&self) -> bool {
        self.fallback.is_active()
    }

    /// Drain pending audit events.
    ///
    /// Returns all audit events accumulated since the last drain and
    /// clears the internal buffer.
    pub fn drain_audit_events(&mut self) -> Vec<GpuAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }

    /// Access the GPU device.
    #[must_use]
    pub fn device(&self) -> &GpuDevice {
        &self.device
    }

    /// Access the VRAM allocator.
    #[must_use]
    pub fn vram_allocator(&self) -> &VramAllocator {
        &self.vram_allocator
    }

    /// Mutable access to the VRAM allocator.
    pub fn vram_allocator_mut(&mut self) -> &mut VramAllocator {
        &mut self.vram_allocator
    }

    /// Access the render target pool.
    #[must_use]
    pub fn render_target_pool(&self) -> &RenderTargetPool {
        &self.render_target_pool
    }

    /// Mutable access to the render target pool.
    pub fn render_target_pool_mut(&mut self) -> &mut RenderTargetPool {
        &mut self.render_target_pool
    }

    /// Access the DMA-BUF manager.
    #[must_use]
    pub fn dmabuf_manager(&self) -> &DmaBufManager {
        &self.dmabuf_manager
    }

    /// Mutable access to the DMA-BUF manager.
    pub fn dmabuf_manager_mut(&mut self) -> &mut DmaBufManager {
        &mut self.dmabuf_manager
    }

    /// Access the fallback manager.
    #[must_use]
    pub fn fallback_manager(&self) -> &FallbackManager {
        &self.fallback
    }

    /// Mutable access to the fallback manager.
    pub fn fallback_manager_mut(&mut self) -> &mut FallbackManager {
        &mut self.fallback
    }

    /// Access the pipeline.
    #[must_use]
    pub fn pipeline(&self) -> &ComputePipeline {
        &self.pipeline
    }
}
