//! Dedicated render thread pool

use crate::error::{RenderError, Result};
use crate::render_task::{RenderTask, RenderOutput};
use crossbeam_channel::{Sender, Receiver, bounded, select};
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering}};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::{debug, error, warn, info};

/// A render task wrapper for priority queue
struct PrioritizedTask {
    task: RenderTask,
}

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool {
        self.task.priority == other.task.priority && self.task.id == other.task.id
    }
}

impl Eq for PrioritizedTask {}

impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Tasks past their deadline get highest priority.
        let self_overdue = self.task.is_overdue();
        let other_overdue = other.task.is_overdue();
        other_overdue
            .cmp(&self_overdue)
            .then_with(|| {
                // Reverse ordering for max-heap (higher priority = smaller value)
                other.task.priority.cmp(&self.task.priority)
            })
            .then_with(|| self.task.created_at.cmp(&other.task.created_at))
    }
}

/// Configuration for a render thread
#[derive(Debug, Clone)]
pub struct ThreadConfig {
    /// Thread name
    pub name: String,
    
    /// Queue capacity
    pub queue_capacity: usize,
    
    /// Enable priority scheduling
    pub priority_scheduling: bool,
}

impl Default for ThreadConfig {
    fn default() -> Self {
        Self {
            name: "render-thread".to_string(),
            queue_capacity: 128,
            priority_scheduling: true,
        }
    }
}

/// A dedicated render thread
pub struct RenderThread {
    /// Thread configuration
    #[allow(dead_code)]
    config: ThreadConfig,
    
    /// Task sender
    task_tx: Sender<RenderTask>,
    
    /// Output receiver
    output_rx: Receiver<RenderOutput>,
    
    /// Thread handle
    handle: Option<JoinHandle<()>>,
    
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
    
    /// Task counter
    task_counter: Arc<AtomicU64>,
}

impl RenderThread {
    /// Create a new render thread
    pub fn new(config: ThreadConfig) -> Result<Self> {
        let (task_tx, task_rx) = bounded(config.queue_capacity);
        let (output_tx, output_rx) = bounded(config.queue_capacity);
        
        let shutdown = Arc::new(AtomicBool::new(false));
        let task_counter = Arc::new(AtomicU64::new(0));
        
        let thread_shutdown = shutdown.clone();
        let thread_counter = task_counter.clone();
        let thread_config = config.clone();
        
        let handle = thread::Builder::new()
            .name(config.name.clone())
            .spawn(move || {
                Self::run_loop(
                    thread_config,
                    task_rx,
                    output_tx,
                    thread_shutdown,
                    thread_counter,
                );
            })
            .map_err(|e| RenderError::ThreadPoolInit(e.to_string()))?;
        
        Ok(Self {
            config,
            task_tx,
            output_rx,
            handle: Some(handle),
            shutdown,
            task_counter,
        })
    }
    
    /// Submit a task to this thread
    pub fn submit(&self, task: RenderTask) -> Result<()> {
        if self.shutdown.load(AtomicOrdering::Relaxed) {
            return Err(RenderError::RenderTaskFailed("Thread is shutting down".to_string()));
        }
        
        self.task_tx
            .send(task)
            .map_err(|e| RenderError::ChannelSend(e.to_string()))?;
        
        self.task_counter.fetch_add(1, AtomicOrdering::Relaxed);
        Ok(())
    }
    
    /// Try to receive an output (non-blocking)
    pub fn try_recv_output(&self) -> Result<Option<RenderOutput>> {
        match self.output_rx.try_recv() {
            Ok(output) => Ok(Some(output)),
            Err(crossbeam_channel::TryRecvError::Empty) => Ok(None),
            Err(e) => Err(RenderError::ChannelRecv(e.to_string())),
        }
    }
    
    /// Receive output with timeout
    pub fn recv_output(&self, timeout: Duration) -> Result<RenderOutput> {
        self.output_rx
            .recv_timeout(timeout)
            .map_err(|e| match e {
                crossbeam_channel::RecvTimeoutError::Timeout => RenderError::Timeout(timeout),
                crossbeam_channel::RecvTimeoutError::Disconnected => {
                    RenderError::ChannelRecv("Channel disconnected".to_string())
                }
            })
    }
    
    /// Get number of tasks processed
    pub fn task_count(&self) -> u64 {
        self.task_counter.load(AtomicOrdering::Relaxed)
    }
    
    /// Shutdown the thread
    pub fn shutdown(mut self) -> Result<()> {
        self.shutdown.store(true, AtomicOrdering::Relaxed);
        
        if let Some(handle) = self.handle.take() {
            handle.join()
                .map_err(|e| RenderError::ThreadJoin(format!("{:?}", e)))?;
        }
        
        Ok(())
    }
    
    /// Main render loop
    fn run_loop(
        config: ThreadConfig,
        task_rx: Receiver<RenderTask>,
        output_tx: Sender<RenderOutput>,
        shutdown: Arc<AtomicBool>,
        task_counter: Arc<AtomicU64>,
    ) {
        info!("Render thread '{}' started", config.name);
        
        let mut priority_queue: BinaryHeap<PrioritizedTask> = BinaryHeap::new();
        
        while !shutdown.load(AtomicOrdering::Relaxed) {
            // Try to receive tasks with timeout
            select! {
                recv(task_rx) -> result => {
                    match result {
                        Ok(task) => {
                            if config.priority_scheduling {
                                priority_queue.push(PrioritizedTask { task });
                            } else {
                                Self::execute_task(task, &output_tx, &task_counter);
                            }
                        }
                        Err(_) => {
                            debug!("Task channel closed for thread '{}'", config.name);
                            break;
                        }
                    }
                }
                default(Duration::from_millis(1)) => {
                    // Process priority queue
                    if let Some(prioritized) = priority_queue.pop() {
                        Self::execute_task(prioritized.task, &output_tx, &task_counter);
                    }
                }
            }
        }
        
        // Process remaining tasks in priority queue
        while let Some(prioritized) = priority_queue.pop() {
            Self::execute_task(prioritized.task, &output_tx, &task_counter);
        }
        
        // Drain any remaining tasks from the channel
        while let Ok(task) = task_rx.try_recv() {
            Self::execute_task(task, &output_tx, &task_counter);
        }
        
        info!("Render thread '{}' stopped", config.name);
    }
    
    /// Execute a single render task
    fn execute_task(
        task: RenderTask,
        output_tx: &Sender<RenderOutput>,
        task_counter: &Arc<AtomicU64>,
    ) {
        let start = Instant::now();
        
        debug!(
            "Executing task {} (kind: {:?}, priority: {:?})",
            task.id, task.kind, task.priority
        );
        
        // TODO: Actual rendering implementation
        // For now, just simulate work
        let output = if task.is_overdue() {
            warn!("Task {} is overdue", task.id);
            RenderOutput::failure(
                task.id,
                start.elapsed(),
                "Task exceeded deadline".to_string(),
            )
        } else {
            // Simulate render work
            std::thread::sleep(Duration::from_micros(100));
            RenderOutput::success(task.id, task.data, start.elapsed())
        };
        
        if let Err(e) = output_tx.send(output) {
            error!("Failed to send output for task {}: {}", task.id, e);
        }
        
        task_counter.fetch_add(1, AtomicOrdering::Relaxed);
    }
}

/// Pool of render threads
pub struct RenderThreadPool {
    threads: Vec<RenderThread>,
    next_thread: AtomicU64,
}

impl RenderThreadPool {
    /// Create a new thread pool
    pub fn new(count: usize, config: ThreadConfig) -> Result<Self> {
        let mut threads = Vec::with_capacity(count);
        
        for i in 0..count {
            let mut thread_config = config.clone();
            thread_config.name = format!("{}-{}", config.name, i);
            threads.push(RenderThread::new(thread_config)?);
        }
        
        Ok(Self {
            threads,
            next_thread: AtomicU64::new(0),
        })
    }
    
    /// Submit task to the pool (round-robin with retry on dead threads)
    pub fn submit(&self, task: RenderTask) -> Result<()> {
        let count = self.threads.len();
        let start_idx = self.next_thread.fetch_add(1, AtomicOrdering::AcqRel) as usize % count;
        
        let mut last_err = None;
        for attempt in 0..count {
            let idx = (start_idx + attempt) % count;
            match self.threads[idx].submit(task.clone()) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!("Thread {} failed to accept task: {}, trying next", idx, e);
                    last_err = Some(e);
                }
            }
        }
        
        Err(last_err.unwrap_or_else(|| RenderError::RenderTaskFailed("All threads failed to accept task".to_string())))
    }
    
    /// Submit task to specific thread
    pub fn submit_to(&self, thread_idx: usize, task: RenderTask) -> Result<()> {
        self.threads.get(thread_idx)
            .ok_or_else(|| RenderError::InvalidConfig("Invalid thread index".to_string()))?
            .submit(task)
    }
    
    /// Try to receive any output
    pub fn try_recv_any(&self) -> Result<Option<RenderOutput>> {
        for thread in &self.threads {
            if let Some(output) = thread.try_recv_output()? {
                return Ok(Some(output));
            }
        }
        Ok(None)
    }
    
    /// Get number of threads
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
    
    /// Get total tasks processed
    pub fn total_tasks(&self) -> u64 {
        self.threads.iter().map(|t| t.task_count()).sum()
    }
    
    /// Shutdown all threads
    pub fn shutdown(self) -> Result<()> {
        for thread in self.threads {
            thread.shutdown()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_task::RenderTaskKind;
    
    #[test]
    fn test_thread_creation() {
        let config = ThreadConfig::default();
        let thread = RenderThread::new(config).unwrap();
        assert!(thread.handle.is_some());
        thread.shutdown().unwrap();
    }
    
    #[test]
    fn test_task_submission() {
        let config = ThreadConfig::default();
        let thread = RenderThread::new(config).unwrap();
        
        let task = RenderTask::new(1, RenderTaskKind::Dock);
        thread.submit(task).unwrap();
        
        let output = thread.recv_output(Duration::from_secs(1)).unwrap();
        assert_eq!(output.task_id, 1);
        assert!(output.success);
        
        thread.shutdown().unwrap();
    }
    
    #[test]
    fn test_thread_pool() {
        let config = ThreadConfig::default();
        let pool = RenderThreadPool::new(4, config).unwrap();
        
        assert_eq!(pool.thread_count(), 4);
        
        for i in 0..10 {
            let task = RenderTask::new(i, RenderTaskKind::Dock);
            pool.submit(task).unwrap();
        }
        
        pool.shutdown().unwrap();
    }
}
