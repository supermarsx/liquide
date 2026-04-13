//! Main render coordinator implementation

use crate::config::RenderConfig;
use crate::error::{RenderError, Result};
use crate::metrics::MetricsCollector;
use crate::render_task::{RenderTask, RenderTaskKind, RenderOutput};
use crate::thread_pool::{RenderThread, RenderThreadPool, ThreadConfig};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Maximum number of pending outputs to buffer before rejecting new insertions.
const MAX_PENDING: usize = 1024;

/// Central coordinator for multi-threaded rendering
pub struct RenderCoordinator {
    /// Configuration
    config: RenderConfig,
    
    /// Window render thread pool
    window_pool: RenderThreadPool,
    
    /// Dock render thread
    dock_thread: Option<RenderThread>,
    
    /// Status bar render thread
    statusbar_thread: Option<RenderThread>,
    
    /// Background render thread
    background_thread: Option<RenderThread>,
    
    /// Wallpaper render thread
    wallpaper_thread: Option<RenderThread>,
    
    /// Task ID counter
    next_task_id: AtomicU64,
    
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
    
    /// Pending outputs
    pending_outputs: Arc<Mutex<HashMap<u64, RenderOutput>>>,
}

impl RenderCoordinator {
    /// Create a new render coordinator
    pub async fn new(config: RenderConfig) -> Result<Self> {
        config.validate()
            .map_err(RenderError::InvalidConfig)?;
        
        info!("Initializing render coordinator with {} window threads", config.window_threads);
        
        // Create window thread pool
        let window_config = ThreadConfig {
            name: "window-render".to_string(),
            queue_capacity: config.queue_size,
            priority_scheduling: config.focused_window_boost,
        };
        let window_pool = RenderThreadPool::new(config.window_threads, window_config)?;
        
        // Create specialized threads
        let dock_thread = if config.enable_dock {
            let config = ThreadConfig {
                name: "dock-render".to_string(),
                queue_capacity: config.queue_size,
                priority_scheduling: false,
            };
            Some(RenderThread::new(config)?)
        } else {
            None
        };
        
        let statusbar_thread = if config.enable_statusbar {
            let config = ThreadConfig {
                name: "statusbar-render".to_string(),
                queue_capacity: config.queue_size,
                priority_scheduling: false,
            };
            Some(RenderThread::new(config)?)
        } else {
            None
        };
        
        let background_thread = if config.enable_background {
            let config = ThreadConfig {
                name: "background-render".to_string(),
                queue_capacity: config.queue_size,
                priority_scheduling: false,
            };
            Some(RenderThread::new(config)?)
        } else {
            None
        };
        
        let wallpaper_thread = if config.enable_wallpaper {
            let config = ThreadConfig {
                name: "wallpaper-render".to_string(),
                queue_capacity: config.queue_size,
                priority_scheduling: false,
            };
            Some(RenderThread::new(config)?)
        } else {
            None
        };
        
        Ok(Self {
            config,
            window_pool,
            dock_thread,
            statusbar_thread,
            background_thread,
            wallpaper_thread,
            next_task_id: AtomicU64::new(1),
            metrics: Arc::new(MetricsCollector::new()),
            pending_outputs: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    /// Submit a render task
    pub async fn submit_task(&self, mut task: RenderTask) -> Result<u64> {
        // Always assign a new unique ID from the atomic counter to prevent collisions.
        task.id = self.next_task_id.fetch_add(1, Ordering::AcqRel);
        
        // Set deadline if not set
        if task.deadline.is_none() {
            task = task.with_deadline(self.config.timeout);
        }
        
        let task_id = task.id;
        self.metrics.record_submission();
        
        // Route to appropriate thread
        match &task.kind {
            RenderTaskKind::Window { .. } => {
                self.window_pool.submit(task)?;
            }
            RenderTaskKind::Dock => {
                if let Some(thread) = &self.dock_thread {
                    thread.submit(task)?;
                } else {
                    return Err(RenderError::InvalidConfig("Dock rendering not enabled".to_string()));
                }
            }
            RenderTaskKind::StatusBar => {
                if let Some(thread) = &self.statusbar_thread {
                    thread.submit(task)?;
                } else {
                    return Err(RenderError::InvalidConfig("Status bar rendering not enabled".to_string()));
                }
            }
            RenderTaskKind::Background => {
                if let Some(thread) = &self.background_thread {
                    thread.submit(task)?;
                } else {
                    return Err(RenderError::InvalidConfig("Background rendering not enabled".to_string()));
                }
            }
            RenderTaskKind::Wallpaper { .. } => {
                if let Some(thread) = &self.wallpaper_thread {
                    thread.submit(task)?;
                } else {
                    return Err(RenderError::InvalidConfig("Wallpaper rendering not enabled".to_string()));
                }
            }
            RenderTaskKind::Composite { .. } => {
                // Composite tasks go to window pool
                self.window_pool.submit(task)?;
            }
        }
        
        Ok(task_id)
    }
    
    /// Poll for completed renders
    pub async fn poll_outputs(&self) -> Result<Vec<RenderOutput>> {
        let mut outputs = Vec::new();
        
        // Poll window pool
        while let Some(output) = self.window_pool.try_recv_any()? {
            self.metrics.record_completion(output.duration, output.success);
            outputs.push(output);
        }
        
        // Poll specialized threads
        for thread in [
            &self.dock_thread,
            &self.statusbar_thread,
            &self.background_thread,
            &self.wallpaper_thread,
        ].iter().filter_map(|t| t.as_ref()) {
            while let Some(output) = thread.try_recv_output()? {
                self.metrics.record_completion(output.duration, output.success);
                outputs.push(output);
            }
        }
        
        Ok(outputs)
    }
    
    /// Wait for a specific task to complete
    pub async fn wait_for_task(&self, task_id: u64, timeout: Duration) -> Result<RenderOutput> {
        // Check if already completed
        {
            let mut pending = liquide_common::sync::lock_or_recover(&self.pending_outputs);
            if let Some(output) = pending.remove(&task_id) {
                return Ok(output);
            }
        }
        
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            let outputs = self.poll_outputs().await?;
            
            // Separate target output from others before locking.
            let mut found = None;
            let mut others = Vec::new();
            for output in outputs {
                if output.task_id == task_id {
                    found = Some(output);
                } else {
                    others.push(output);
                }
            }
            
            // Batch-insert non-target outputs under a single short lock.
            if !others.is_empty() {
                let mut pending = liquide_common::sync::lock_or_recover(&self.pending_outputs);
                if pending.len() + others.len() <= MAX_PENDING {
                    for output in others {
                        pending.insert(output.task_id, output);
                    }
                } else {
                    warn!("Pending outputs at capacity ({}), dropping {} outputs", MAX_PENDING, others.len());
                }
            }
            
            if let Some(output) = found {
                return Ok(output);
            }
            
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
        
        Err(RenderError::Timeout(timeout))
    }
    
    /// Get current metrics
    pub fn metrics(&self) -> crate::metrics::RenderMetrics {
        self.metrics.snapshot()
    }
    
    /// Render a window (convenience method)
    pub async fn render_window(&self, window_id: u64, is_focused: bool) -> Result<u64> {
        let task = RenderTask::new(
            0,
            RenderTaskKind::Window { window_id, is_focused },
        );
        self.submit_task(task).await
    }
    
    /// Render the dock (convenience method)
    pub async fn render_dock(&self) -> Result<u64> {
        let task = RenderTask::new(0, RenderTaskKind::Dock);
        self.submit_task(task).await
    }
    
    /// Render the status bar (convenience method)
    pub async fn render_statusbar(&self) -> Result<u64> {
        let task = RenderTask::new(0, RenderTaskKind::StatusBar);
        self.submit_task(task).await
    }
    
    /// Render the background (convenience method)
    pub async fn render_background(&self) -> Result<u64> {
        let task = RenderTask::new(0, RenderTaskKind::Background);
        self.submit_task(task).await
    }
    
    /// Render wallpaper frame (convenience method)
    pub async fn render_wallpaper(&self, frame: u64) -> Result<u64> {
        let task = RenderTask::new(0, RenderTaskKind::Wallpaper { frame });
        self.submit_task(task).await
    }
    
    /// Get configuration
    pub fn config(&self) -> &RenderConfig {
        &self.config
    }
}

impl Drop for RenderCoordinator {
    fn drop(&mut self) {
        debug!("Shutting down render coordinator");
        // Threads will be cleaned up automatically
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_coordinator_creation() {
        let config = RenderConfig::default();
        let coordinator = RenderCoordinator::new(config).await.unwrap();
        
        let metrics = coordinator.metrics();
        assert_eq!(metrics.tasks_submitted, 0);
    }
    
    #[tokio::test]
    async fn test_window_rendering() {
        let config = RenderConfig::builder()
            .window_threads(2)
            .build();
        
        let coordinator = RenderCoordinator::new(config).await.unwrap();
        
        let task_id = coordinator.render_window(1, true).await.unwrap();
        assert!(task_id > 0);
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let outputs = coordinator.poll_outputs().await.unwrap();
        assert!(!outputs.is_empty());
    }
    
    #[tokio::test]
    async fn test_specialized_threads() {
        let config = RenderConfig::builder()
            .enable_dock(true)
            .enable_statusbar(true)
            .build();
        
        let coordinator = RenderCoordinator::new(config).await.unwrap();
        
        coordinator.render_dock().await.unwrap();
        coordinator.render_statusbar().await.unwrap();
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let outputs = coordinator.poll_outputs().await.unwrap();
        assert!(outputs.len() >= 2);
    }
}
