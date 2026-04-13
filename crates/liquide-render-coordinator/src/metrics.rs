//! Rendering metrics collection

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

/// Metrics for rendering performance
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RenderMetrics {
    /// Total tasks submitted
    pub tasks_submitted: u64,
    
    /// Total tasks completed
    pub tasks_completed: u64,
    
    /// Total tasks failed
    pub tasks_failed: u64,
    
    /// Average render time (microseconds)
    pub avg_render_time_us: f64,
    
    /// Min render time (microseconds)
    pub min_render_time_us: u64,
    
    /// Max render time (microseconds)
    pub max_render_time_us: u64,
    
    /// P95 render time (microseconds)
    pub p95_render_time_us: u64,
    
    /// P99 render time (microseconds)
    pub p99_render_time_us: u64,
    
    /// Tasks per second
    pub tasks_per_second: f64,
    
    /// Current queue depth
    pub queue_depth: usize,
}

/// Thread-safe metrics collector
pub struct MetricsCollector {
    tasks_submitted: AtomicU64,
    tasks_completed: AtomicU64,
    tasks_failed: AtomicU64,
    render_times: RwLock<Vec<u64>>,
    start_time: Mutex<Instant>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            tasks_submitted: AtomicU64::new(0),
            tasks_completed: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
            render_times: RwLock::new(Vec::with_capacity(1000)),
            start_time: Mutex::new(Instant::now()),
        }
    }
    
    /// Record task submission
    pub fn record_submission(&self) {
        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record task completion
    pub fn record_completion(&self, duration: Duration, success: bool) {
        if success {
            self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.tasks_failed.fetch_add(1, Ordering::Relaxed);
        }
        
        let micros = duration.as_micros() as u64;
        if let Ok(mut times) = self.render_times.write() {
            times.push(micros);
            
            // Keep only last 1000 samples
            if times.len() > 1000 {
                let drain_to = times.len() - 1000;
                times.drain(0..drain_to);
            }
        }
    }
    
    /// Get current metrics snapshot
    pub fn snapshot(&self) -> RenderMetrics {
        let submitted = self.tasks_submitted.load(Ordering::Relaxed);
        let completed = self.tasks_completed.load(Ordering::Relaxed);
        let failed = self.tasks_failed.load(Ordering::Relaxed);
        
        // Clone under read lock, then release before sorting.
        let mut sorted_times = {
            let times = liquide_common::sync::read_or_recover(&self.render_times);
            times.clone()
        };
        sorted_times.sort_unstable();
        
        let avg = if !sorted_times.is_empty() {
            sorted_times.iter().sum::<u64>() as f64 / sorted_times.len() as f64
        } else {
            0.0
        };
        
        let min = sorted_times.first().copied().unwrap_or(0);
        let max = sorted_times.last().copied().unwrap_or(0);
        
        let p95_idx = ((sorted_times.len().saturating_sub(1)) as f64 * 0.95).round() as usize;
        let p99_idx = ((sorted_times.len().saturating_sub(1)) as f64 * 0.99).round() as usize;
        
        let p95 = sorted_times.get(p95_idx).copied().unwrap_or(0);
        let p99 = sorted_times.get(p99_idx).copied().unwrap_or(0);
        
        let elapsed = self.start_time.lock().unwrap().elapsed().as_secs_f64();
        let tasks_per_second = if elapsed > 0.0 {
            completed as f64 / elapsed
        } else {
            0.0
        };
        
        RenderMetrics {
            tasks_submitted: submitted,
            tasks_completed: completed,
            tasks_failed: failed,
            avg_render_time_us: avg,
            min_render_time_us: min,
            max_render_time_us: max,
            p95_render_time_us: p95,
            p99_render_time_us: p99,
            tasks_per_second,
            queue_depth: 0, // Updated externally
        }
    }
    
    /// Reset all metrics
    pub fn reset(&self) {
        self.tasks_submitted.store(0, Ordering::Relaxed);
        self.tasks_completed.store(0, Ordering::Relaxed);
        self.tasks_failed.store(0, Ordering::Relaxed);
        if let Ok(mut times) = self.render_times.write() {
            times.clear();
        }
        if let Ok(mut t) = self.start_time.lock() {
            *t = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_recording() {
        let collector = MetricsCollector::new();
        
        collector.record_submission();
        collector.record_completion(Duration::from_micros(100), true);
        collector.record_submission();
        collector.record_completion(Duration::from_micros(200), true);
        collector.record_submission();
        collector.record_completion(Duration::from_micros(150), false);
        
        let metrics = collector.snapshot();
        assert_eq!(metrics.tasks_submitted, 3);
        assert_eq!(metrics.tasks_completed, 2);
        assert_eq!(metrics.tasks_failed, 1);
        assert!(metrics.avg_render_time_us > 0.0);
    }
}
