//! Per-window render pipeline using `liquide-render-thread`.
//!
//! `WindowRenderManager` maintains a map of window ID → per-window
//! `RenderCoordinator` (from `liquide-render-thread`), each with its
//! own chrome and content worker threads.
//!
//! Per-window rendering is **opt-in**: the main render thread remains the
//! primary path. Windows that benefit from fault isolation (e.g. heavy
//! content or plugin-hosting windows) can be registered here so their
//! decorations stay responsive even if content rendering stalls.

use std::collections::HashMap;
use std::thread;

use liquide_compositor::scene::FlatNode;
use liquide_render_thread::coordinator::RenderCoordinator;
use liquide_render_thread::message::FrameComplete;
use liquide_render_thread::{ChromeThread, ContentThread, chrome_thread, content_thread};
use liquide_renderer_cpu::SoftwareRenderer;
use tracing::{debug, info, warn};

/// Per-window pipeline wrapping a `RenderCoordinator` and its OS threads.
struct WindowPipeline {
    coordinator: RenderCoordinator,
    chrome_handle: Option<thread::JoinHandle<()>>,
    content_handle: Option<thread::JoinHandle<()>>,
}

/// Manages per-window chrome/content render thread pairs.
pub(super) struct WindowRenderManager {
    /// Active per-window pipelines.
    pipelines: HashMap<u64, WindowPipeline>,
}

impl WindowRenderManager {
    pub(super) fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
        }
    }

    /// Register a window for per-window rendering.
    ///
    /// Spawns chrome and content worker threads immediately.
    /// Each thread creates its own `SoftwareRenderer` instance.
    #[allow(dead_code)]
    pub(super) fn register_window(&mut self, window_id: u64, width: u32, height: u32) {
        if self.pipelines.contains_key(&window_id) {
            return;
        }

        let mut coord = RenderCoordinator::new(width, height);
        // Standard CSD title bar inset.
        coord.set_chrome_insets(30, 0, 1, 1);

        // Chrome thread.
        let (chrome, chrome_rx, chrome_comp_tx) = ChromeThread::new(width, height);
        let chrome_handle = thread::Builder::new()
            .name(format!("chrome-{}", window_id))
            .spawn(move || {
                let renderer = Box::new(SoftwareRenderer::new());
                chrome_thread::chrome_worker(chrome_rx, chrome_comp_tx, renderer);
            })
            .expect("failed to spawn chrome thread");

        // Content thread.
        let (content_vw, content_vh) = coord.content_viewport();
        let (content, content_rx, content_comp_tx) = ContentThread::new(content_vw, content_vh);
        let content_handle = thread::Builder::new()
            .name(format!("content-{}", window_id))
            .spawn(move || {
                let renderer = Box::new(SoftwareRenderer::new());
                content_thread::content_worker(content_rx, content_comp_tx, renderer);
            })
            .expect("failed to spawn content thread");

        coord.attach_chrome(chrome);
        coord.attach_content(content);

        info!(window_id, "per-window render threads spawned");

        self.pipelines.insert(
            window_id,
            WindowPipeline {
                coordinator: coord,
                chrome_handle: Some(chrome_handle),
                content_handle: Some(content_handle),
            },
        );
    }

    /// Check if a window has an active per-window pipeline.
    #[allow(dead_code)]
    pub(super) fn has_window(&self, window_id: u64) -> bool {
        self.pipelines.contains_key(&window_id)
    }

    /// Request a frame for a specific window.
    ///
    /// `chrome_nodes` are the decoration FlatNodes (title bar, borders).
    /// `content_nodes` are the application content FlatNodes.
    #[allow(dead_code)]
    pub(super) fn request_frame(
        &mut self,
        window_id: u64,
        chrome_nodes: Vec<FlatNode>,
        content_nodes: Vec<FlatNode>,
    ) {
        if let Some(pipeline) = self.pipelines.get_mut(&window_id) {
            if let Err(e) = pipeline
                .coordinator
                .request_frame(chrome_nodes, content_nodes)
            {
                warn!(window_id, error = %e, "failed to submit per-window frame");
            }
        }
    }

    /// Poll all pipelines for completed frames.
    ///
    /// Returns `(window_id, Vec<FrameComplete>)` for each window with results.
    #[allow(dead_code)]
    pub(super) fn poll_completions(&mut self) -> Vec<(u64, Vec<FrameComplete>)> {
        let mut results = Vec::new();
        for (&wid, pipeline) in &mut self.pipelines {
            let completions = pipeline.coordinator.poll_completions();
            if !completions.is_empty() {
                results.push((wid, completions));
            }
        }
        results
    }

    /// Unregister a window and shut down its threads.
    pub(super) fn unregister_window(&mut self, window_id: u64) {
        if let Some(mut pipeline) = self.pipelines.remove(&window_id) {
            let _ = pipeline.coordinator.shutdown();
            if let Some(h) = pipeline.chrome_handle.take() {
                let _ = h.join();
            }
            if let Some(h) = pipeline.content_handle.take() {
                let _ = h.join();
            }
            debug!(window_id, "per-window render threads shut down");
        }
    }

    /// Shut down all per-window pipelines.
    pub(super) fn shutdown_all(&mut self) {
        let ids: Vec<u64> = self.pipelines.keys().copied().collect();
        for id in ids {
            self.unregister_window(id);
        }
    }

    /// Number of active per-window pipelines.
    #[allow(dead_code)]
    pub(super) fn active_count(&self) -> usize {
        self.pipelines.len()
    }
}

impl Drop for WindowRenderManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
