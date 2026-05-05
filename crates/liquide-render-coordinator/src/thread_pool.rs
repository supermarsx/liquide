//! Dedicated render thread pool

use crate::error::{RenderError, Result};
use crate::render_task::{
    RenderData, RenderDataFormat, RenderOutput, RenderOutputMetadata, RenderScene, RenderTarget,
    RenderTask, RenderTaskKind,
};
use crossbeam_channel::{Receiver, Sender, bounded};
use liquide_compositor::Renderer;
use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::scene::{FlatNode, SceneNodeKind};
use liquide_renderer_cpu::SoftwareRenderer;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::panic::AssertUnwindSafe;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering as AtomicOrdering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Maximum number of times a thread will respawn after panics before giving up
const MAX_RESPAWNS: u32 = 3;

/// Base backoff duration in milliseconds for respawn delays (100ms, 200ms, 400ms)
const RESPAWN_BACKOFF_BASE_MS: u64 = 100;

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

    /// Number of times the thread has panicked and been respawned
    panic_count: Arc<AtomicU32>,
}

impl RenderThread {
    /// Create a new render thread
    pub fn new(config: ThreadConfig) -> Result<Self> {
        let (task_tx, task_rx) = bounded(config.queue_capacity);
        let (output_tx, output_rx) = bounded(config.queue_capacity);

        let shutdown = Arc::new(AtomicBool::new(false));
        let task_counter = Arc::new(AtomicU64::new(0));
        let panic_count = Arc::new(AtomicU32::new(0));

        let thread_shutdown = shutdown.clone();
        let thread_counter = task_counter.clone();
        let thread_panic_count = panic_count.clone();
        let thread_config = config.clone();

        let handle = thread::Builder::new()
            .name(config.name.clone())
            .spawn(move || {
                Self::run_with_respawn(
                    thread_config,
                    task_rx,
                    output_tx,
                    thread_shutdown,
                    thread_counter,
                    thread_panic_count,
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
            panic_count,
        })
    }

    /// Submit a task to this thread
    pub fn submit(&self, task: RenderTask) -> Result<()> {
        if self.shutdown.load(AtomicOrdering::Relaxed) {
            return Err(RenderError::RenderTaskFailed(
                "Thread is shutting down".to_string(),
            ));
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
        self.output_rx.recv_timeout(timeout).map_err(|e| match e {
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

    /// Get the number of times this thread has panicked and been respawned
    pub fn panic_count(&self) -> u32 {
        self.panic_count.load(AtomicOrdering::Relaxed)
    }

    /// Shutdown the thread
    pub fn shutdown(mut self) -> Result<()> {
        self.shutdown.store(true, AtomicOrdering::Relaxed);

        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|e| RenderError::ThreadJoin(format!("{:?}", e)))?;
        }

        Ok(())
    }

    /// Run the render loop with automatic respawn on panic.
    ///
    /// Wraps `run_loop` in `catch_unwind` so that if the loop panics, we log
    /// the error, apply exponential backoff, and re-enter the loop — up to
    /// `MAX_RESPAWNS` times.  After that, the thread gives up permanently.
    ///
    /// # Safety note on `AssertUnwindSafe`
    /// The values captured by the closure (channels, atomics, config) are all
    /// either `Clone` or behind `Arc`, so they remain valid after a panic.
    /// We clone them into each `catch_unwind` invocation so that any mid-panic
    /// drops only affect the clones.
    fn run_with_respawn(
        config: ThreadConfig,
        task_rx: Receiver<RenderTask>,
        output_tx: Sender<RenderOutput>,
        shutdown: Arc<AtomicBool>,
        task_counter: Arc<AtomicU64>,
        panic_count: Arc<AtomicU32>,
    ) {
        loop {
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                Self::run_loop(
                    config.clone(),
                    task_rx.clone(),
                    output_tx.clone(),
                    shutdown.clone(),
                    task_counter.clone(),
                );
            }));

            match result {
                Ok(()) => break, // Normal shutdown
                Err(payload) => {
                    let count = panic_count.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                    let msg = panic_payload_message(&payload);
                    error!(
                        "Render thread '{}' panicked (attempt {}/{}): {}",
                        config.name, count, MAX_RESPAWNS, msg
                    );

                    if count >= MAX_RESPAWNS {
                        error!(
                            "Render thread '{}' exceeded max respawn count ({}), giving up",
                            config.name, MAX_RESPAWNS
                        );
                        break;
                    }

                    if shutdown.load(AtomicOrdering::Relaxed) {
                        debug!(
                            "Render thread '{}' shutdown requested, not respawning",
                            config.name
                        );
                        break;
                    }

                    // Exponential backoff: 100ms, 200ms, 400ms
                    let backoff =
                        Duration::from_millis(RESPAWN_BACKOFF_BASE_MS * 2u64.pow(count - 1));
                    warn!(
                        "Respawning render thread '{}' after {:?} backoff",
                        config.name, backoff
                    );
                    thread::sleep(backoff);

                    info!(
                        "Render thread '{}' respawned (attempt {})",
                        config.name,
                        count + 1
                    );
                }
            }
        }
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
        let mut renderer = SoftwareRenderer::new();

        while !shutdown.load(AtomicOrdering::Relaxed) {
            if config.priority_scheduling {
                // Block until a task arrives (with timeout to check shutdown)
                if priority_queue.is_empty() {
                    match task_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(task) => {
                            priority_queue.push(PrioritizedTask { task });
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            debug!("Task channel closed for thread '{}'", config.name);
                            break;
                        }
                    }
                }

                // Batch-drain any additional pending tasks
                while let Ok(task) = task_rx.try_recv() {
                    priority_queue.push(PrioritizedTask { task });
                }

                // Process the highest-priority task
                if let Some(prioritized) = priority_queue.pop() {
                    Self::execute_task_safe(
                        prioritized.task,
                        &mut renderer,
                        &output_tx,
                        &task_counter,
                    );
                }
            } else {
                // Without priority scheduling, block until a task arrives
                match task_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(task) => {
                        Self::execute_task_safe(task, &mut renderer, &output_tx, &task_counter);
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        debug!("Task channel closed for thread '{}'", config.name);
                        break;
                    }
                }
            }
        }

        // Process remaining tasks in priority queue
        while let Some(prioritized) = priority_queue.pop() {
            Self::execute_task_safe(prioritized.task, &mut renderer, &output_tx, &task_counter);
        }

        // Drain any remaining tasks from the channel
        while let Ok(task) = task_rx.try_recv() {
            Self::execute_task_safe(task, &mut renderer, &output_tx, &task_counter);
        }

        info!("Render thread '{}' stopped", config.name);
    }

    /// Execute a task, catching any panic so the thread loop stays alive.
    ///
    /// If the task panics, we log the error and send a failure output for that
    /// task.  The thread itself continues processing subsequent tasks.
    fn execute_task_safe(
        task: RenderTask,
        renderer: &mut SoftwareRenderer,
        output_tx: &Sender<RenderOutput>,
        task_counter: &Arc<AtomicU64>,
    ) {
        let task_id = task.id;
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            Self::execute_task(task, renderer, output_tx, task_counter);
        }));

        if let Err(payload) = result {
            let msg = panic_payload_message(&payload);
            error!("Task {} panicked: {}", task_id, msg);
            let output =
                RenderOutput::failure(task_id, Duration::ZERO, format!("Task panicked: {}", msg));
            let _ = output_tx.send(output);
        }
    }

    /// Execute a single render task
    fn execute_task(
        task: RenderTask,
        renderer: &mut SoftwareRenderer,
        output_tx: &Sender<RenderOutput>,
        task_counter: &Arc<AtomicU64>,
    ) {
        let start = Instant::now();
        let task_id = task.id;

        debug!(
            "Executing task {} (kind: {:?}, priority: {:?})",
            task.id, task.kind, task.priority
        );

        let output = if task.is_overdue() {
            warn!("Task {} is overdue, skipping", task.id);
            RenderOutput::failure(
                task.id,
                start.elapsed(),
                "Task exceeded deadline".to_string(),
            )
        } else {
            match &task.kind {
                RenderTaskKind::Window {
                    window_id,
                    is_focused,
                } => {
                    debug!("Rendering window {} (focused={})", window_id, is_focused);
                }
                RenderTaskKind::Dock => {
                    debug!("Rendering dock");
                }
                RenderTaskKind::StatusBar => {
                    debug!("Rendering status bar");
                }
                RenderTaskKind::Background => {
                    debug!("Rendering background");
                }
                RenderTaskKind::Wallpaper { frame } => {
                    debug!("Rendering wallpaper frame {}", frame);
                }
                RenderTaskKind::Composite { layer_ids } => {
                    debug!("Compositing {} layers", layer_ids.len());
                }
            }
            Self::render_task_output(task, renderer, start)
        };

        if let Err(e) = output_tx.send(output) {
            error!("Failed to send output for task {}: {}", task_id, e);
        }

        task_counter.fetch_add(1, AtomicOrdering::Relaxed);
    }

    fn render_task_output(
        task: RenderTask,
        renderer: &mut SoftwareRenderer,
        start: Instant,
    ) -> RenderOutput {
        let scene = task
            .scene
            .unwrap_or_else(|| default_scene_for_task(&task.kind));
        let target = scene.target();

        if let Err(error) = target.validate() {
            return RenderOutput::failure(task.id, start.elapsed(), error);
        }

        let Some(pixel_format) = pixel_format_for_render_data(target.format) else {
            return RenderOutput::failure(
                task.id,
                start.elapsed(),
                format!("unsupported render data format {:?}", target.format),
            );
        };

        let mut framebuffer = FrameBuffer::new(target.width, target.height, pixel_format);
        framebuffer.clear(Color::BLACK);

        let tile_size = target.tile_size.max(1);
        let damage = DamageSet::full(
            tile_size,
            target.width.div_ceil(tile_size),
            target.height.div_ceil(tile_size),
            DamageClass::UiPrimitive,
        );

        let damage_tiles = match renderer.render(scene.nodes(), &mut framebuffer, &damage) {
            Ok(tiles) if !tiles.is_empty() => tiles,
            Ok(_) => damage.materialize_tiles(),
            Err(error) => {
                return RenderOutput::failure(
                    task.id,
                    start.elapsed(),
                    format!("CPU renderer failed: {error}"),
                );
            }
        };

        let pixels = framebuffer.pixels().to_vec();
        if pixels.is_empty() {
            return RenderOutput::failure(
                task.id,
                start.elapsed(),
                "CPU renderer produced an empty framebuffer".to_string(),
            );
        }

        let metadata = RenderOutputMetadata {
            width: framebuffer.width,
            height: framebuffer.height,
            stride: framebuffer.stride,
            format: target.format,
            tile_size,
            damage_tiles,
        };

        RenderOutput::success_with_metadata(
            task.id,
            RenderData::new(pixels, target.format),
            metadata,
            start.elapsed(),
        )
    }
}

fn pixel_format_for_render_data(format: RenderDataFormat) -> Option<PixelFormat> {
    match format {
        RenderDataFormat::Bgra8 => Some(PixelFormat::Bgra8),
        RenderDataFormat::Rgba8 => Some(PixelFormat::Rgba8),
        RenderDataFormat::Compressed | RenderDataFormat::CommandBuffer => None,
    }
}

fn default_scene_for_task(kind: &RenderTaskKind) -> RenderScene {
    let color = match kind {
        RenderTaskKind::Window { is_focused, .. } => {
            if *is_focused {
                Color::new(42, 117, 255, 255)
            } else {
                Color::new(76, 86, 106, 255)
            }
        }
        RenderTaskKind::Dock => Color::new(26, 42, 72, 255),
        RenderTaskKind::StatusBar => Color::new(16, 24, 40, 255),
        RenderTaskKind::Background => Color::new(10, 14, 28, 255),
        RenderTaskKind::Wallpaper { frame } => {
            let accent = (*frame as u8).wrapping_mul(13).max(32);
            Color::new(accent, 48, 118, 255)
        }
        RenderTaskKind::Composite { layer_ids } => {
            let accent = (layer_ids.len() as u8).saturating_mul(40).max(64);
            Color::new(48, accent, 128, 255)
        }
    };

    let target = RenderTarget::new(64, 64);
    RenderScene::with_target(target, vec![background_node(1, 64.0, 64.0, color)])
}

fn background_node(id: u64, width: f32, height: f32, color: Color) -> FlatNode {
    FlatNode {
        id,
        kind: Arc::new(SceneNodeKind::Background { color }),
        absolute_bounds: Rect::new(0.0, 0.0, width, height),
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

/// Extract a human-readable message from a panic payload.
fn panic_payload_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
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

        Err(last_err.unwrap_or_else(|| {
            RenderError::RenderTaskFailed("All threads failed to accept task".to_string())
        }))
    }

    /// Submit task to specific thread
    pub fn submit_to(&self, thread_idx: usize, task: RenderTask) -> Result<()> {
        self.threads
            .get(thread_idx)
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

    fn test_scene(width: u32, height: u32, color: Color) -> RenderScene {
        RenderScene::new(
            width,
            height,
            vec![background_node(1, width as f32, height as f32, color)],
        )
    }

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

        let task = RenderTask::new(1, RenderTaskKind::Dock).with_scene(test_scene(
            16,
            16,
            Color::new(0, 120, 200, 255),
        ));
        thread.submit(task).unwrap();

        let output = thread.recv_output(Duration::from_secs(1)).unwrap();
        assert_eq!(output.task_id, 1);
        assert!(output.success);
        assert_eq!(
            output.metadata.as_ref().map(|m| (m.width, m.height)),
            Some((16, 16))
        );
        assert!(
            output
                .data
                .as_ref()
                .is_some_and(|data| !data.data().is_empty())
        );

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

    #[test]
    fn test_multiple_task_outputs() {
        let config = ThreadConfig::default();
        let thread = RenderThread::new(config).unwrap();
        for i in 1..=5 {
            let task = RenderTask::new(i, RenderTaskKind::Dock).with_scene(test_scene(
                8,
                8,
                Color::new(i as u8, 60, 120, 255),
            ));
            thread.submit(task).unwrap();
        }
        let mut received = Vec::new();
        for _ in 0..5 {
            let output = thread.recv_output(Duration::from_secs(2)).unwrap();
            received.push(output.task_id);
        }
        assert_eq!(received.len(), 5);
        thread.shutdown().unwrap();
    }

    #[test]
    fn test_pool_submit_to_specific() {
        let config = ThreadConfig::default();
        let pool = RenderThreadPool::new(2, config).unwrap();
        let task = RenderTask::new(1, RenderTaskKind::Dock);
        pool.submit_to(0, task).unwrap();
        let task2 = RenderTask::new(2, RenderTaskKind::StatusBar);
        pool.submit_to(1, task2).unwrap();
        pool.shutdown().unwrap();
    }

    #[test]
    fn test_pool_invalid_thread_index() {
        let config = ThreadConfig::default();
        let pool = RenderThreadPool::new(2, config).unwrap();
        let task = RenderTask::new(1, RenderTaskKind::Dock);
        let result = pool.submit_to(99, task);
        assert!(result.is_err());
        pool.shutdown().unwrap();
    }

    #[test]
    fn test_thread_config_default() {
        let config = ThreadConfig::default();
        assert_eq!(config.name, "render-thread");
        assert_eq!(config.queue_capacity, 128);
        assert!(config.priority_scheduling);
    }

    #[test]
    fn test_panic_payload_message_str() {
        let payload: Box<dyn std::any::Any + Send> = Box::new("boom");
        assert_eq!(panic_payload_message(&payload), "boom");
    }

    #[test]
    fn test_panic_payload_message_string() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(String::from("string boom"));
        assert_eq!(panic_payload_message(&payload), "string boom");
    }

    #[test]
    fn test_panic_payload_message_unknown() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(42i32);
        assert_eq!(panic_payload_message(&payload), "unknown panic payload");
    }

    #[test]
    fn test_thread_panic_count_zero_on_normal_operation() {
        let config = ThreadConfig::default();
        let thread = RenderThread::new(config).unwrap();

        for i in 1..=5 {
            let task = RenderTask::new(i, RenderTaskKind::Dock);
            thread.submit(task).unwrap();
        }
        for _ in 0..5 {
            let output = thread.recv_output(Duration::from_secs(2)).unwrap();
            assert!(output.success);
        }

        assert_eq!(thread.panic_count(), 0);
        thread.shutdown().unwrap();
    }

    #[test]
    fn test_respawn_loop_with_catch_unwind() {
        // Directly exercises the catch_unwind + respawn pattern used by
        // run_with_respawn: a counter tracks how many times the inner
        // closure has been entered; the first two invocations panic, and
        // the third completes normally.
        use std::sync::atomic::AtomicU32;

        let attempts = Arc::new(AtomicU32::new(0));
        let panic_count = Arc::new(AtomicU32::new(0));
        let attempts_c = attempts.clone();
        let panic_count_c = panic_count.clone();

        let handle = thread::spawn(move || {
            loop {
                let attempts_inner = attempts_c.clone();
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let n = attempts_inner.fetch_add(1, AtomicOrdering::SeqCst);
                    if n < 2 {
                        panic!("test panic #{}", n);
                    }
                    // Third attempt succeeds
                }));

                match result {
                    Ok(()) => break,
                    Err(_) => {
                        let c = panic_count_c.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                        if c >= MAX_RESPAWNS {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        });

        handle.join().unwrap();
        assert_eq!(panic_count.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(attempts.load(AtomicOrdering::SeqCst), 3);
    }

    #[test]
    fn test_respawn_gives_up_after_max() {
        // Verifies that a perpetually-panicking closure stops after
        // MAX_RESPAWNS panics instead of looping forever.
        use std::sync::atomic::AtomicU32;

        let panic_count = Arc::new(AtomicU32::new(0));
        let panic_count_c = panic_count.clone();

        let handle = thread::spawn(move || {
            loop {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    panic!("always panic");
                }));

                match result {
                    Ok(()) => break,
                    Err(_) => {
                        let c = panic_count_c.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                        if c >= MAX_RESPAWNS {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        });

        handle.join().unwrap();
        assert_eq!(panic_count.load(AtomicOrdering::SeqCst), MAX_RESPAWNS);
    }
}
