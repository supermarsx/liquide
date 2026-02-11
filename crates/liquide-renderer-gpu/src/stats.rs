//! Performance statistics for the GPU renderer.
//!
//! Collects per-frame timing data and produces aggregate statistics
//! for monitoring, diagnostics, and quality-of-service enforcement.

/// Per-frame statistics from the GPU renderer.
#[derive(Debug, Clone)]
pub struct GpuFrameStats {
    /// Time spent in the compositing stage in microseconds.
    pub composite_time_us: u64,
    /// Time spent in the blur stage in microseconds.
    pub blur_time_us: u64,
    /// Total frame time in microseconds.
    pub total_time_us: u64,
    /// VRAM usage at the end of this frame in megabytes.
    pub vram_used_mb: u64,
    /// Frame identifier.
    pub frame_id: u64,
}

/// Aggregate rendering statistics.
#[derive(Debug, Clone)]
pub struct GpuRenderStats {
    /// Total number of frames rendered.
    pub frames_rendered: u64,
    /// Average compositing time in microseconds.
    pub avg_composite_us: f64,
    /// Average total frame time in microseconds.
    pub avg_total_us: f64,
    /// Peak VRAM usage in megabytes.
    pub peak_vram_mb: u64,
    /// Number of VK_ERROR_DEVICE_LOST events.
    pub device_lost_count: u64,
    /// Number of times the renderer fell back to CPU.
    pub fallback_count: u64,
}

impl Default for GpuRenderStats {
    fn default() -> Self {
        Self {
            frames_rendered: 0,
            avg_composite_us: 0.0,
            avg_total_us: 0.0,
            peak_vram_mb: 0,
            device_lost_count: 0,
            fallback_count: 0,
        }
    }
}

/// Collector for GPU frame statistics.
///
/// Records per-frame data and computes aggregate summaries for
/// monitoring dashboards and SLO tracking.
#[derive(Debug)]
pub struct StatsCollector {
    /// Running totals for computing averages.
    total_composite_us: u64,
    total_time_us: u64,
    frames_rendered: u64,
    peak_vram_mb: u64,
    device_lost_count: u64,
    fallback_count: u64,
}

impl StatsCollector {
    /// Create a new empty stats collector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total_composite_us: 0,
            total_time_us: 0,
            frames_rendered: 0,
            peak_vram_mb: 0,
            device_lost_count: 0,
            fallback_count: 0,
        }
    }

    /// Record a frame's statistics.
    pub fn record_frame(&mut self, stats: GpuFrameStats) {
        self.total_composite_us += stats.composite_time_us;
        self.total_time_us += stats.total_time_us;
        self.frames_rendered += 1;

        if stats.vram_used_mb > self.peak_vram_mb {
            self.peak_vram_mb = stats.vram_used_mb;
        }

        tracing::trace!(
            frame_id = stats.frame_id,
            composite_us = stats.composite_time_us,
            total_us = stats.total_time_us,
            vram_mb = stats.vram_used_mb,
            "frame stats recorded"
        );
    }

    /// Record a device-lost event.
    pub fn record_device_lost(&mut self) {
        self.device_lost_count += 1;
    }

    /// Record a fallback activation.
    pub fn record_fallback(&mut self) {
        self.fallback_count += 1;
    }

    /// Compute and return aggregate statistics.
    #[must_use]
    pub fn summary(&self) -> GpuRenderStats {
        let avg_composite_us = if self.frames_rendered > 0 {
            self.total_composite_us as f64 / self.frames_rendered as f64
        } else {
            0.0
        };

        let avg_total_us = if self.frames_rendered > 0 {
            self.total_time_us as f64 / self.frames_rendered as f64
        } else {
            0.0
        };

        GpuRenderStats {
            frames_rendered: self.frames_rendered,
            avg_composite_us,
            avg_total_us,
            peak_vram_mb: self.peak_vram_mb,
            device_lost_count: self.device_lost_count,
            fallback_count: self.fallback_count,
        }
    }

    /// Reset all collected statistics.
    pub fn reset(&mut self) {
        self.total_composite_us = 0;
        self.total_time_us = 0;
        self.frames_rendered = 0;
        self.peak_vram_mb = 0;
        self.device_lost_count = 0;
        self.fallback_count = 0;
    }
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}
