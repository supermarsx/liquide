//! Telemetry and performance monitoring - "State of the Nation" dashboard.
//!
//! Tracks rendering performance, per-window metrics, thread utilization,
//! and system health to identify bottlenecks and ensure fluid rendering.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Maximum number of historical samples to retain per metric.
const MAX_HISTORY: usize = 120; // ~2 seconds at 60fps

/// Telemetry data for a single window.
#[derive(Debug, Clone)]
pub struct WindowMetrics {
    pub window_id: u64,
    /// Recent render times in milliseconds.
    pub render_times: VecDeque<f64>,
    /// Average render time (ms) over recent history.
    pub avg_render_ms: f64,
    /// Maximum render time (ms) in recent history.
    pub max_render_ms: f64,
    /// Number of nodes in this window's scene graph.
    pub node_count: usize,
    /// Whether this window is currently being dragged/resized.
    pub is_interactive: bool,
    /// Last update timestamp.
    pub last_update: Instant,
    /// Number of slow frames (>16ms) in recent history.
    pub slow_frame_count: u32,
}

impl WindowMetrics {
    fn new(window_id: u64) -> Self {
        Self {
            window_id,
            render_times: VecDeque::with_capacity(MAX_HISTORY),
            avg_render_ms: 0.0,
            max_render_ms: 0.0,
            node_count: 0,
            is_interactive: false,
            last_update: Instant::now(),
            slow_frame_count: 0,
        }
    }

    fn record_render(&mut self, render_ms: f64, node_count: usize) {
        self.render_times.push_back(render_ms);
        if self.render_times.len() > MAX_HISTORY {
            self.render_times.pop_front();
        }

        self.node_count = node_count;
        self.last_update = Instant::now();

        // Update statistics
        self.avg_render_ms = self.render_times.iter().sum::<f64>() / self.render_times.len() as f64;
        self.max_render_ms = self.render_times.iter().copied().fold(0.0, f64::max);
        self.slow_frame_count = self.render_times.iter().filter(|&&t| t > 16.0).count() as u32;
    }
}

/// Global frame timing metrics.
#[derive(Debug, Clone)]
pub struct FrameMetrics {
    /// Recent frame times in milliseconds.
    pub frame_times: VecDeque<f64>,
    /// Average frame time (ms).
    pub avg_frame_ms: f64,
    /// Target frame time for configured FPS cap.
    pub target_frame_ms: f64,
    /// Number of frames rendered.
    pub frame_count: u64,
    /// Number of frames that missed the target time.
    pub missed_frames: u64,
    /// Current FPS (calculated from recent frame times).
    pub current_fps: f64,
}

impl FrameMetrics {
    fn new(target_fps: u32) -> Self {
        Self {
            frame_times: VecDeque::with_capacity(MAX_HISTORY),
            avg_frame_ms: 0.0,
            target_frame_ms: if target_fps > 0 {
                1000.0 / target_fps as f64
            } else {
                0.0
            },
            frame_count: 0,
            missed_frames: 0,
            current_fps: 0.0,
        }
    }

    fn record_frame(&mut self, frame_ms: f64) {
        self.frame_times.push_back(frame_ms);
        if self.frame_times.len() > MAX_HISTORY {
            self.frame_times.pop_front();
        }

        self.frame_count += 1;
        if self.target_frame_ms > 0.0 && frame_ms > self.target_frame_ms {
            self.missed_frames += 1;
        }

        self.avg_frame_ms = self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64;
        self.current_fps = if self.avg_frame_ms > 0.0 {
            1000.0 / self.avg_frame_ms
        } else {
            0.0
        };
    }
}

/// Render thread pool metrics.
#[derive(Debug, Clone)]
pub struct ThreadPoolMetrics {
    /// Number of active render threads.
    pub thread_count: usize,
    /// Number of queued render jobs.
    pub queued_jobs: usize,
    /// Number of in-flight render jobs.
    pub active_jobs: usize,
    /// Thread utilization (0.0 - 1.0).
    pub utilization: f64,
}

/// System health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// System is performing optimally.
    Healthy,
    /// System is experiencing minor performance degradation.
    Degraded,
    /// System is experiencing significant slowdown.
    Slow,
    /// System is critically overloaded.
    Critical,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Degraded => "Degraded",
            Self::Slow => "Slow",
            Self::Critical => "Critical",
        }
    }
}

/// Comprehensive telemetry and monitoring system.
pub struct Telemetry {
    /// Per-window performance metrics.
    windows: HashMap<u64, WindowMetrics>,
    /// Global frame timing.
    frames: FrameMetrics,
    /// Thread pool metrics.
    thread_pool: ThreadPoolMetrics,
    /// Overall system health status.
    health: HealthStatus,
    /// Timestamp when telemetry was initialized.
    start_time: Instant,
}

impl Telemetry {
    /// Create a new telemetry system.
    pub fn new(target_fps: u32) -> Self {
        Self {
            windows: HashMap::new(),
            frames: FrameMetrics::new(target_fps),
            thread_pool: ThreadPoolMetrics {
                thread_count: 0,
                queued_jobs: 0,
                active_jobs: 0,
                utilization: 0.0,
            },
            health: HealthStatus::Healthy,
            start_time: Instant::now(),
        }
    }

    /// Record a window render job completion.
    pub fn record_window_render(&mut self, window_id: u64, render_ms: f64, node_count: usize) {
        self.windows
            .entry(window_id)
            .or_insert_with(|| WindowMetrics::new(window_id))
            .record_render(render_ms, node_count);
    }

    /// Record a complete frame render.
    pub fn record_frame(&mut self, frame_ms: f64) {
        self.frames.record_frame(frame_ms);
        self.update_health();
    }

    /// Mark a window as interactive (being dragged/resized).
    pub fn set_window_interactive(&mut self, window_id: u64, interactive: bool) {
        if let Some(metrics) = self.windows.get_mut(&window_id) {
            metrics.is_interactive = interactive;
        }
    }

    /// Update thread pool metrics.
    pub fn update_thread_pool(&mut self, thread_count: usize, queued: usize, active: usize) {
        self.thread_pool.thread_count = thread_count;
        self.thread_pool.queued_jobs = queued;
        self.thread_pool.active_jobs = active;
        self.thread_pool.utilization = if thread_count > 0 {
            active as f64 / thread_count as f64
        } else {
            0.0
        };
    }

    /// Get metrics for a specific window.
    pub fn window_metrics(&self, window_id: u64) -> Option<&WindowMetrics> {
        self.windows.get(&window_id)
    }

    /// Get all window metrics.
    pub fn all_window_metrics(&self) -> &HashMap<u64, WindowMetrics> {
        &self.windows
    }

    /// Get frame metrics.
    pub fn frame_metrics(&self) -> &FrameMetrics {
        &self.frames
    }

    /// Get thread pool metrics.
    pub fn thread_pool_metrics(&self) -> &ThreadPoolMetrics {
        &self.thread_pool
    }

    /// Get overall system health.
    pub fn health(&self) -> HealthStatus {
        self.health
    }

    /// System uptime in seconds.
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Generate a human-readable status report.
    pub fn status_report(&self) -> String {
        let mut report = String::new();
        report.push_str("═══ Liquide Session Telemetry ═══\n");
        report.push_str(&format!("Health: {}\n", self.health.as_str()));
        report.push_str(&format!("Uptime: {}s\n", self.uptime_seconds()));
        report.push_str(&format!(
            "FPS: {:.1} (target: {:.1})\n",
            self.frames.current_fps,
            if self.frames.target_frame_ms > 0.0 {
                1000.0 / self.frames.target_frame_ms
            } else {
                0.0
            }
        ));
        report.push_str(&format!(
            "Frame Time: {:.2}ms (avg), {:.2}ms (target)\n",
            self.frames.avg_frame_ms, self.frames.target_frame_ms
        ));
        report.push_str(&format!(
            "Missed Frames: {}/{} ({:.1}%)\n",
            self.frames.missed_frames,
            self.frames.frame_count,
            if self.frames.frame_count > 0 {
                100.0 * self.frames.missed_frames as f64 / self.frames.frame_count as f64
            } else {
                0.0
            }
        ));
        report.push_str(&format!(
            "Thread Pool: {}/{} active, {} queued ({:.0}% util)\n",
            self.thread_pool.active_jobs,
            self.thread_pool.thread_count,
            self.thread_pool.queued_jobs,
            self.thread_pool.utilization * 100.0
        ));
        report.push_str(&format!("\nWindow Metrics ({} windows):\n", self.windows.len()));
        for (wid, metrics) in &self.windows {
            report.push_str(&format!(
                "  Window {}: {:.2}ms avg, {} nodes, {} slow frames{}\n",
                wid,
                metrics.avg_render_ms,
                metrics.node_count,
                metrics.slow_frame_count,
                if metrics.is_interactive {
                    " [DRAG]"
                } else {
                    ""
                }
            ));
        }
        report
    }

    /// Update overall health status based on current metrics.
    fn update_health(&mut self) {
        // Determine health based on frame timing and window performance
        let avg_frame = self.frames.avg_frame_ms;
        let target = self.frames.target_frame_ms;

        // Count problematic windows
        let slow_windows = self
            .windows
            .values()
            .filter(|m| m.avg_render_ms > 16.0)
            .count();

        self.health = if target > 0.0 && avg_frame > target * 2.0 {
            HealthStatus::Critical
        } else if target > 0.0 && avg_frame > target * 1.5 {
            HealthStatus::Slow
        } else if slow_windows > 0 || avg_frame > 16.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
    }
}

/// Thread-safe telemetry handle for sharing across threads.
pub type TelemetryHandle = Arc<RwLock<Telemetry>>;

/// Create a new shared telemetry instance.
pub fn create_telemetry(target_fps: u32) -> TelemetryHandle {
    Arc::new(RwLock::new(Telemetry::new(target_fps)))
}
