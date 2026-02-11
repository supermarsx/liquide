//! Encoder performance metrics.

use serde::{Deserialize, Serialize};

/// Point-in-time snapshot of encoder metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Number of active encoding sessions.
    pub active_sessions: u32,
    /// Depth of the encoder output queue.
    pub queue_depth: u32,
    /// Average encoding time in microseconds.
    pub avg_encode_time_us: u64,
    /// Total fallback events since last reset.
    pub fallback_total: u32,
    /// Total errors since last reset.
    pub errors_total: u32,
}

/// Tracks encoder performance counters.
pub struct EncoderMetrics {
    active_sessions: u32,
    queue_depth: u32,
    encode_times_us: Vec<u64>,
    fallback_total: u32,
    errors_total: u32,
}

impl EncoderMetrics {
    /// Create a new metrics tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_sessions: 0,
            queue_depth: 0,
            encode_times_us: Vec::new(),
            fallback_total: 0,
            errors_total: 0,
        }
    }

    /// Record an encode operation's duration.
    pub fn record_encode(&mut self, time_us: u64) {
        self.encode_times_us.push(time_us);
    }

    /// Record a fallback event.
    pub fn record_fallback(&mut self) {
        self.fallback_total += 1;
    }

    /// Record an error.
    pub fn record_error(&mut self) {
        self.errors_total += 1;
    }

    /// Set the active session count gauge.
    pub fn set_active_sessions(&mut self, count: u32) {
        self.active_sessions = count;
    }

    /// Set the queue depth gauge.
    pub fn set_queue_depth(&mut self, depth: u32) {
        self.queue_depth = depth;
    }

    /// Take a snapshot of the current metrics.
    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        let avg = if self.encode_times_us.is_empty() {
            0
        } else {
            self.encode_times_us.iter().sum::<u64>() / self.encode_times_us.len() as u64
        };
        MetricsSnapshot {
            active_sessions: self.active_sessions,
            queue_depth: self.queue_depth,
            avg_encode_time_us: avg,
            fallback_total: self.fallback_total,
            errors_total: self.errors_total,
        }
    }

    /// Number of encode samples recorded.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.encode_times_us.len()
    }
}

impl Default for EncoderMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-GPU hardware metrics (read from driver APIs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetrics {
    /// Device name.
    pub device: String,
    /// Currently used VRAM (MB).
    pub vram_used_mb: u64,
    /// Total VRAM (MB).
    pub vram_total_mb: u64,
    /// GPU utilisation percentage.
    pub utilization_pct: f32,
    /// GPU temperature in Celsius (if available).
    pub temperature_celsius: Option<f32>,
    /// Number of active encoder sessions on this GPU.
    pub encoder_active: u32,
}
