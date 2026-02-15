//! Telemetry data types.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Complete telemetry snapshot from a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    /// Timestamp when this snapshot was taken.
    pub timestamp: u64,
    
    /// Frame metrics.
    pub frames: FrameMetrics,
    
    /// Per-window metrics.
    pub windows: HashMap<u64, WindowMetrics>,
    
    /// System health status.
    pub health: HealthStatus,
    
    /// Thread pool metrics.
    pub threads: ThreadPoolMetrics,
}

/// Frame-level performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMetrics {
    /// Current FPS.
    pub fps: f64,
    
    /// Average frame time (ms).
    pub avg_frame_time: f64,
    
    /// Minimum frame time (ms).
    pub min_frame_time: f64,
    
    /// Maximum frame time (ms).
    pub max_frame_time: f64,
    
    /// 95th percentile frame time (ms).
    pub p95_frame_time: f64,
    
    /// 99th percentile frame time (ms).
    pub p99_frame_time: f64,
    
    /// Frame time history (last 120 frames).
    pub history: VecDeque<f64>,
}

/// Per-window rendering metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMetrics {
    /// Window ID.
    pub window_id: u64,
    
    /// Average render time for this window (ms).
    pub avg_render_time: f64,
    
    /// Number of nodes in this window's scene graph.
    pub node_count: usize,
    
    /// Whether this window is currently being interacted with.
    pub interactive: bool,
    
    /// Render time history.
    pub render_history: VecDeque<f64>,
}

/// System health classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// All metrics within normal ranges (< 16ms).
    Healthy,
    
    /// Some frame drops but generally responsive (16-25ms).
    Degraded,
    
    /// Noticeable lag (25-50ms).
    Slow,
    
    /// Severe performance issues (> 50ms).
    Critical,
}

impl HealthStatus {
    /// Get a human-readable description.
    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Healthy => "All systems normal",
            Self::Degraded => "Minor frame drops detected",
            Self::Slow => "Noticeable performance degradation",
            Self::Critical => "Severe performance issues",
        }
    }
    
    /// Get a color code for display.
    #[allow(dead_code)]
    pub fn color(&self) -> &'static str {
        match self {
            Self::Healthy => "green",
            Self::Degraded => "yellow",
            Self::Slow => "orange",
            Self::Critical => "red",
        }
    }
}

/// Thread pool utilization metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadPoolMetrics {
    /// Number of active threads.
    pub active_threads: usize,
    
    /// Number of idle threads.
    pub idle_threads: usize,
    
    /// Average task queue depth.
    pub avg_queue_depth: f64,
    
    /// Tasks completed in last second.
    pub tasks_per_second: u64,
}

impl Default for TelemetrySnapshot {
    fn default() -> Self {
        Self {
            timestamp: 0,
            frames: FrameMetrics {
                fps: 0.0,
                avg_frame_time: 0.0,
                min_frame_time: 0.0,
                max_frame_time: 0.0,
                p95_frame_time: 0.0,
                p99_frame_time: 0.0,
                history: VecDeque::new(),
            },
            windows: HashMap::new(),
            health: HealthStatus::Healthy,
            threads: ThreadPoolMetrics {
                active_threads: 0,
                idle_threads: 0,
                avg_queue_depth: 0.0,
                tasks_per_second: 0,
            },
        }
    }
}
