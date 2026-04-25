//! Threaded render coordinator that runs on its own thread

use crate::config::RenderConfig;
use crate::coordinator::RenderCoordinator;
use crate::error::{RenderError, Result};
use crate::metrics::RenderMetrics;
use crate::render_task::{RenderOutput, RenderTask};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, error, info};

/// Message sent to the coordinator thread
enum CoordinatorMessage {
    /// Submit a render task
    SubmitTask {
        task: RenderTask,
        response: Sender<Result<u64>>,
    },

    /// Poll for outputs
    PollOutputs {
        response: Sender<Result<Vec<RenderOutput>>>,
    },

    /// Wait for a specific task
    WaitForTask {
        task_id: u64,
        timeout: Duration,
        response: Sender<Result<RenderOutput>>,
    },

    /// Get metrics
    GetMetrics { response: Sender<RenderMetrics> },

    /// Shutdown the coordinator
    Shutdown,
}

/// Threaded render coordinator that runs on its own thread
///
/// This wrapper runs the actual `RenderCoordinator` on a dedicated thread,
/// allowing the main thread to interact with it via message passing.
pub struct ThreadedRenderCoordinator {
    /// Channel to send messages to coordinator
    tx: Sender<CoordinatorMessage>,

    /// Thread handle
    handle: Option<JoinHandle<()>>,
}

impl ThreadedRenderCoordinator {
    /// Create a new threaded render coordinator
    ///
    /// Spawns a new thread that runs the coordinator event loop.
    pub fn new(config: RenderConfig) -> Result<Self> {
        let (tx, rx) = channel();

        let handle = thread::Builder::new()
            .name("render-coordinator".to_string())
            .spawn(move || {
                Self::coordinator_thread(config, rx);
            })
            .map_err(|e| RenderError::ThreadCreation(e.to_string()))?;

        Ok(Self {
            tx,
            handle: Some(handle),
        })
    }

    /// Coordinator thread main loop
    fn coordinator_thread(config: RenderConfig, rx: Receiver<CoordinatorMessage>) {
        info!("Render coordinator thread started");

        // Create tokio runtime for the coordinator.
        // Use multi-thread to prevent deadlock when async operations spawn tasks.
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                error!("Failed to create tokio runtime: {}", e);
                return;
            }
        };

        // Initialize coordinator
        let coordinator = match rt.block_on(RenderCoordinator::new(config)) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to initialize coordinator: {}", e);
                return;
            }
        };

        // Main message loop
        loop {
            match rx.recv() {
                Ok(msg) => match msg {
                    CoordinatorMessage::SubmitTask { task, response } => {
                        let result = rt.block_on(coordinator.submit_task(task));
                        let _ = response.send(result);
                    }

                    CoordinatorMessage::PollOutputs { response } => {
                        let result = rt.block_on(coordinator.poll_outputs());
                        let _ = response.send(result);
                    }

                    CoordinatorMessage::WaitForTask {
                        task_id,
                        timeout,
                        response,
                    } => {
                        let result = rt.block_on(coordinator.wait_for_task(task_id, timeout));
                        let _ = response.send(result);
                    }

                    CoordinatorMessage::GetMetrics { response } => {
                        let metrics = coordinator.metrics();
                        let _ = response.send(metrics);
                    }

                    CoordinatorMessage::Shutdown => {
                        debug!("Coordinator received shutdown signal");
                        break;
                    }
                },
                Err(_) => {
                    debug!("Coordinator channel closed, shutting down");
                    break;
                }
            }
        }

        info!("Render coordinator thread stopped");
    }

    /// Submit a render task
    pub fn submit_task(&self, task: RenderTask) -> Result<u64> {
        let (tx, rx) = channel();
        self.tx
            .send(CoordinatorMessage::SubmitTask { task, response: tx })
            .map_err(|_| RenderError::ChannelSend("Coordinator thread died".to_string()))?;

        rx.recv()
            .map_err(|_| RenderError::ChannelRecv("Coordinator thread died".to_string()))?
    }

    /// Poll for completed renders
    pub fn poll_outputs(&self) -> Result<Vec<RenderOutput>> {
        let (tx, rx) = channel();
        self.tx
            .send(CoordinatorMessage::PollOutputs { response: tx })
            .map_err(|_| RenderError::ChannelSend("Coordinator thread died".to_string()))?;

        rx.recv()
            .map_err(|_| RenderError::ChannelRecv("Coordinator thread died".to_string()))?
    }

    /// Wait for a specific task to complete
    pub fn wait_for_task(&self, task_id: u64, timeout: Duration) -> Result<RenderOutput> {
        let (tx, rx) = channel();
        self.tx
            .send(CoordinatorMessage::WaitForTask {
                task_id,
                timeout,
                response: tx,
            })
            .map_err(|_| RenderError::ChannelSend("Coordinator thread died".to_string()))?;

        rx.recv()
            .map_err(|_| RenderError::ChannelRecv("Coordinator thread died".to_string()))?
    }

    /// Get current metrics
    pub fn metrics(&self) -> Result<RenderMetrics> {
        let (tx, rx) = channel();
        self.tx
            .send(CoordinatorMessage::GetMetrics { response: tx })
            .map_err(|_| RenderError::ChannelSend("Coordinator thread died".to_string()))?;

        rx.recv()
            .map_err(|_| RenderError::ChannelRecv("Coordinator thread died".to_string()))
    }

    /// Convenience method: Render a window
    pub fn render_window(&self, window_id: u64, is_focused: bool) -> Result<u64> {
        let task = RenderTask::new(
            0,
            crate::render_task::RenderTaskKind::Window {
                window_id,
                is_focused,
            },
        );
        self.submit_task(task)
    }

    /// Convenience method: Render the dock
    pub fn render_dock(&self) -> Result<u64> {
        let task = RenderTask::new(0, crate::render_task::RenderTaskKind::Dock);
        self.submit_task(task)
    }

    /// Convenience method: Render the status bar
    pub fn render_statusbar(&self) -> Result<u64> {
        let task = RenderTask::new(0, crate::render_task::RenderTaskKind::StatusBar);
        self.submit_task(task)
    }

    /// Convenience method: Render the background
    pub fn render_background(&self) -> Result<u64> {
        let task = RenderTask::new(0, crate::render_task::RenderTaskKind::Background);
        self.submit_task(task)
    }

    /// Convenience method: Render wallpaper frame
    pub fn render_wallpaper(&self, frame: u64) -> Result<u64> {
        let task = RenderTask::new(0, crate::render_task::RenderTaskKind::Wallpaper { frame });
        self.submit_task(task)
    }
}

impl Drop for ThreadedRenderCoordinator {
    fn drop(&mut self) {
        debug!("Shutting down threaded render coordinator");

        // Send shutdown signal
        let _ = self.tx.send(CoordinatorMessage::Shutdown);

        // Wait for thread to finish
        if let Some(handle) = self.handle.take() {
            if let Err(e) = handle.join() {
                error!("Error joining coordinator thread: {:?}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threaded_coordinator() {
        let config = RenderConfig::default();
        let coordinator = ThreadedRenderCoordinator::new(config).unwrap();

        let metrics = coordinator.metrics().unwrap();
        assert_eq!(metrics.tasks_submitted, 0);
    }

    #[test]
    fn test_threaded_window_rendering() {
        let config = RenderConfig::builder().window_threads(2).build();

        let coordinator = ThreadedRenderCoordinator::new(config).unwrap();

        let task_id = coordinator.render_window(1, true).unwrap();
        assert!(task_id > 0);

        std::thread::sleep(Duration::from_millis(100));

        let outputs = coordinator.poll_outputs().unwrap();
        assert!(!outputs.is_empty());
    }
}
