//! Adaptive bitrate (ABR) quality controller per the LiquiDE specification.

/// Adjustment deltas returned by the quality controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityAdjustment {
    /// Change to quality (positive = lower quality / higher QP).
    pub quality_delta: i32,
    /// Change to target FPS (negative = reduce framerate).
    pub fps_delta: i32,
    /// Change to keyframe interval.
    pub keyframe_interval_delta: i32,
}

/// Adaptive rate controller implementing the specification's ABR pseudocode.
pub struct QualityController {
    current_quality: u32,
    current_fps: u32,
    target_fps: u32,
    backpressure_threshold: f32,
}

impl QualityController {
    /// Create a new controller targeting the given framerate.
    #[must_use]
    pub fn new(target_fps: u32) -> Self {
        Self {
            current_quality: 23,
            current_fps: target_fps,
            target_fps,
            backpressure_threshold: 0.8,
        }
    }

    /// Run one ABR iteration given current network/system metrics.
    ///
    /// - `loss_rate`: packet loss ratio (0.0–1.0)
    /// - `queue_occupancy`: encoder output queue fill ratio (0.0–1.0)
    /// - `cpu_util`: CPU utilisation ratio (0.0–1.0)
    /// - `_client_decode_time_us`: client-side decode latency (reserved)
    pub fn adjust(
        &mut self,
        loss_rate: f32,
        queue_occupancy: f32,
        cpu_util: f32,
        _client_decode_time_us: u64,
    ) -> QualityAdjustment {
        let mut quality_delta: i32 = 0;
        let mut fps_delta: i32 = 0;
        let keyframe_interval_delta: i32 = 0;

        if loss_rate > 0.03 {
            quality_delta += 5;
            fps_delta -= 5;
        } else if loss_rate > 0.01 {
            quality_delta += 2;
        }

        if cpu_util > 0.90 {
            fps_delta -= 5;
        }

        if queue_occupancy > self.backpressure_threshold {
            quality_delta += 1;
        }

        // Gradually improve quality when conditions are good
        if quality_delta == 0 && fps_delta == 0 {
            quality_delta = -1;
        }

        // Clamp quality to valid CRF range
        let new_quality = (self.current_quality as i32 + quality_delta).clamp(0, 51) as u32;
        let new_fps = (self.current_fps as i32 + fps_delta).max(1) as u32;
        self.current_quality = new_quality;
        self.current_fps = new_fps;

        QualityAdjustment {
            quality_delta,
            fps_delta,
            keyframe_interval_delta,
        }
    }

    /// Current quality level (0–51 CRF scale, lower = better).
    #[must_use]
    pub fn current_quality(&self) -> u32 {
        self.current_quality
    }

    /// Current target FPS.
    #[must_use]
    pub fn current_fps(&self) -> u32 {
        self.current_fps
    }

    /// Original target FPS.
    #[must_use]
    pub fn target_fps(&self) -> u32 {
        self.target_fps
    }
}
