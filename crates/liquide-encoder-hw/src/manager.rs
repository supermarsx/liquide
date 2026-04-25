//! Top-level encoder manager and `VideoEncoderTrait` bridge.

use crate::config::{FallbackConfig, GpuProfile, HwEncoderConfig};
use crate::fallback::FallbackManager;
use crate::metrics::EncoderMetrics;
use crate::probe::EncoderProber;
use crate::queue::EncoderQueueManager;
use crate::rate_control::QualityController;
use crate::session::SessionConfig;

/// Orchestrates hardware encoder sessions across multiple GPUs.
pub struct HwEncoderManager {
    #[allow(dead_code)]
    config: HwEncoderConfig,
    queue: EncoderQueueManager,
    fallback: FallbackManager,
    metrics: EncoderMetrics,
    quality_controller: QualityController,
    next_session_id: u64,
    gpu_profile: GpuProfile,
}

impl HwEncoderManager {
    /// Create a new manager.
    #[must_use]
    pub fn new(config: HwEncoderConfig, fallback_config: FallbackConfig) -> Self {
        Self {
            quality_controller: QualityController::new(60),
            fallback: FallbackManager::new(fallback_config, Vec::new()),
            config,
            queue: EncoderQueueManager::new(),
            metrics: EncoderMetrics::new(),
            next_session_id: 1,
            gpu_profile: GpuProfile::CpuOnly,
        }
    }

    /// Probe the system for hardware encoders and initialise GPU slots.
    pub fn probe_and_init(&mut self) -> crate::Result<()> {
        let prober = EncoderProber::new();
        let results = prober.probe_all();

        if results.is_empty() {
            self.gpu_profile = GpuProfile::CpuOnly;
            return Ok(());
        }

        for (idx, result) in results.iter().enumerate() {
            self.queue
                .register_gpu(idx, result.api, result.max_sessions, result.vram_total_mb);
        }

        self.gpu_profile = GpuProfile::GpuFull;
        Ok(())
    }

    /// Create a new encoding session on the best available GPU.
    pub fn create_session(&mut self, _config: SessionConfig) -> crate::Result<u64> {
        let gpu = self
            .queue
            .best_gpu()
            .ok_or(crate::HwEncoderError::NoHardwareEncoder)?;
        let vram_estimate = 64; // MB estimate per session
        self.queue.allocate_session(gpu, vram_estimate)?;

        let id = self.next_session_id;
        self.next_session_id += 1;
        self.metrics
            .set_active_sessions(self.queue.total_active_sessions());

        Ok(id)
    }

    /// Destroy a session and free its GPU resources.
    pub fn destroy_session(&mut self, _id: u64) -> crate::Result<()> {
        // In a real implementation we'd look up which GPU the session is on.
        // For now, just decrement the first GPU that has sessions.
        for slot in self.queue.slots() {
            if slot.active_sessions > 0 {
                self.queue.release_session(slot.gpu_index, 64);
                break;
            }
        }
        self.metrics
            .set_active_sessions(self.queue.total_active_sessions());
        Ok(())
    }

    /// Current GPU profile.
    #[must_use]
    pub fn gpu_profile(&self) -> GpuProfile {
        self.gpu_profile
    }

    /// Access the metrics tracker.
    #[must_use]
    pub fn metrics(&self) -> &EncoderMetrics {
        &self.metrics
    }

    /// Total active sessions across all GPUs.
    #[must_use]
    pub fn active_sessions(&self) -> u32 {
        self.queue.total_active_sessions()
    }

    /// Access the quality controller.
    #[must_use]
    pub fn quality_controller(&self) -> &QualityController {
        &self.quality_controller
    }

    /// Mutable access to the quality controller.
    pub fn quality_controller_mut(&mut self) -> &mut QualityController {
        &mut self.quality_controller
    }

    /// Access the fallback manager.
    #[must_use]
    pub fn fallback(&self) -> &FallbackManager {
        &self.fallback
    }
}

/// Wrapper that implements `liquide_encoder::encoder::VideoEncoderTrait`
/// by delegating to the hardware encoder pipeline.
pub struct HwVideoEncoder {
    config: HwEncoderConfig,
}

impl HwVideoEncoder {
    /// Create a new hardware video encoder wrapper.
    #[must_use]
    pub fn new(config: HwEncoderConfig) -> Self {
        Self { config }
    }

    /// Whether hardware encoding is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl liquide_encoder::encoder::VideoEncoderTrait for HwVideoEncoder {
    fn encode_region(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> liquide_encoder::Result<Vec<u8>> {
        // Stub: produce a minimal encoded representation
        let mut output = Vec::with_capacity(12 + 64);
        output.extend_from_slice(&width.to_le_bytes());
        output.extend_from_slice(&height.to_le_bytes());
        output.extend_from_slice(&stride.to_le_bytes());
        let sample_len = pixels.len().min(64);
        output.extend_from_slice(&pixels[..sample_len]);
        Ok(output)
    }

    fn flush(&mut self) -> liquide_encoder::Result<Vec<Vec<u8>>> {
        Ok(Vec::new())
    }
}
