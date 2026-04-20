//! Background-thread blur computation.
//!
//! [`BlurWorker`] offloads expensive Gaussian blur to a dedicated thread so
//! that the render loop is never blocked by blur convolution.  The renderer
//! snapshots the backdrop pixels, sends them to the worker, and uses the
//! most recent cached result (at most one frame old) for compositing.
//!
//! When no cached result is available (first frame, or region changed size),
//! the renderer falls through to a tint-only fill until the worker delivers
//! the blurred pixels.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use liquide_compositor::scene::NodeId;
use tracing::debug;

use crate::blur;

/// Maximum number of cached blur results before eviction.
const MAX_BLUR_CACHE: usize = 128;

// ---------------------------------------------------------------------------
// Types exchanged between renderer and worker
// ---------------------------------------------------------------------------

/// A blur job sent to the worker thread.
struct BlurRequest {
    node_id: NodeId,
    /// BGRA backdrop pixels extracted from the framebuffer.
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    radius: u32,
}

/// A completed blurred region cached for compositing.
pub(crate) struct CachedBlur {
    /// Blurred BGRA pixel data (same dimensions as the request).
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Messages sent to the worker thread.
enum WorkerMsg {
    Blur(BlurRequest),
    Shutdown,
}

// ---------------------------------------------------------------------------
// BlurWorker
// ---------------------------------------------------------------------------

/// Manages a background thread that computes Gaussian blur asynchronously.
///
/// The renderer calls [`request_blur`] to submit a backdrop snapshot and
/// [`get_cached`] to retrieve the most recent result.  [`poll_results`]
/// should be called at the start of each frame to drain completed work.
pub(crate) struct BlurWorker {
    /// Channel to send requests to the worker thread.
    request_tx: mpsc::Sender<WorkerMsg>,
    /// Channel to receive completed blurs from the worker thread.
    result_rx: mpsc::Receiver<(NodeId, CachedBlur)>,
    /// Worker thread handle — joined on drop.
    handle: Option<JoinHandle<()>>,
    /// Most recent blur result per node, used for compositing.
    cache: HashMap<NodeId, CachedBlur>,
    /// Node IDs with pending blur requests (submitted since last poll).
    pending: HashSet<NodeId>,
    /// Monotonically increasing frame counter for staleness tracking.
    frame: u64,
}

impl BlurWorker {
    /// Spawn the background blur worker thread.
    pub fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<WorkerMsg>();
        let (res_tx, res_rx) = mpsc::channel::<(NodeId, CachedBlur)>();

        let handle = match thread::Builder::new()
            .name("blur-worker".into())
            .spawn(move || Self::worker_loop(req_rx, res_tx))
        {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::error!("failed to spawn blur worker thread: {e}; blur effects disabled");
                None
            }
        };

        Self {
            request_tx: req_tx,
            result_rx: res_rx,
            handle,
            cache: HashMap::new(),
            pending: HashSet::new(),
            frame: 0,
        }
    }

    /// The worker thread's main loop.
    ///
    /// Blocks until at least one message arrives, then drains all pending
    /// messages and processes only the latest request per node ID (discards
    /// stale intermediate snapshots).
    fn worker_loop(rx: mpsc::Receiver<WorkerMsg>, tx: mpsc::Sender<(NodeId, CachedBlur)>) {
        loop {
            // Block for the first message.
            let first = match rx.recv() {
                Ok(msg) => msg,
                Err(_) => break, // sender dropped
            };

            match first {
                WorkerMsg::Shutdown => break,
                WorkerMsg::Blur(req) => {
                    // Drain all additional pending messages and keep only
                    // the latest request per node ID.
                    let mut pending: HashMap<NodeId, BlurRequest> = HashMap::new();
                    pending.insert(req.node_id, req);

                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            WorkerMsg::Shutdown => return,
                            WorkerMsg::Blur(r) => {
                                pending.insert(r.node_id, r);
                            }
                        }
                    }

                    // Process each unique node's latest request.
                    for (node_id, req) in pending {
                        let blurred =
                            Self::compute_blur(req.pixels, req.width, req.height, req.radius);

                        let result = CachedBlur {
                            pixels: blurred,
                            width: req.width,
                            height: req.height,
                        };

                        if tx.send((node_id, result)).is_err() {
                            return; // receiver dropped
                        }
                    }
                }
            }
        }
    }

    /// Perform the actual Gaussian blur on a pixel buffer.
    ///
    /// Uses the fast downsample path for large radii, same as the
    /// synchronous [`blur::blur_fast`] but operating on a standalone buffer
    /// instead of a framebuffer.
    fn compute_blur(mut pixels: Vec<u8>, width: u32, height: u32, radius: u32) -> Vec<u8> {
        if radius == 0 || width == 0 || height == 0 {
            return pixels;
        }

        if radius >= 8 {
            // Downsample → blur at half res → upsample (much faster)
            let (small, dw, dh) = blur::blur_downsample_2x(&pixels, width, height);
            if dw > 0 && dh > 0 {
                let kernel = blur::GaussianKernel::new(radius / 2);
                let small_size = (dw * dh * 4) as usize;
                let mut tmp = vec![0u8; small_size];
                let mut blurred = vec![0u8; small_size];
                blur::blur_horizontal(&small, &mut tmp, dw, dh, &kernel);
                blur::blur_vertical(&tmp, &mut blurred, dw, dh, &kernel);
                return blur::blur_upsample_2x_bilinear(&blurred, dw, dh, width, height);
            }
        }

        // Small radius or downsample not possible — blur in-place
        blur::blur_buffer(&mut pixels, width, height, radius);
        pixels
    }

    /// Drain completed blur results into the local cache.
    ///
    /// Call this at the start of each frame before rendering.
    pub fn poll_results(&mut self) {
        self.frame += 1;
        while let Ok((node_id, result)) = self.result_rx.try_recv() {
            self.pending.remove(&node_id);
            self.cache.insert(node_id, result);
        }
        // Evict oldest half when cache exceeds capacity.
        if self.cache.len() > MAX_BLUR_CACHE {
            let to_remove: Vec<NodeId> = self
                .cache
                .keys()
                .take(self.cache.len() / 2)
                .copied()
                .collect();
            for id in to_remove {
                self.cache.remove(&id);
            }
        }
    }

    /// Check whether a blur request is already pending for this node.
    ///
    /// Used to avoid redundant snapshot allocations when the worker
    /// already has a request in-flight.
    pub fn has_pending(&self, node_id: NodeId) -> bool {
        self.pending.contains(&node_id)
    }

    /// Look up a cached blur result for a node.
    ///
    /// Returns `None` if no result is cached or the cached dimensions
    /// don't match (e.g. after a resize).
    pub fn get_cached(&self, node_id: NodeId, width: u32, height: u32) -> Option<&CachedBlur> {
        self.cache
            .get(&node_id)
            .filter(|c| c.width == width && c.height == height)
    }

    /// Submit a blur request.  The result will be available on a subsequent
    /// frame via [`get_cached`].
    pub fn request_blur(
        &mut self,
        node_id: NodeId,
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        radius: u32,
    ) {
        self.pending.insert(node_id);
        if self.request_tx.send(WorkerMsg::Blur(BlurRequest {
            node_id,
            pixels,
            width,
            height,
            radius,
        })).is_err() {
            self.pending.remove(&node_id);
            tracing::warn!("blur worker channel closed; dropping blur request for node {}", node_id);
        }
    }

    /// Remove cached entries for nodes no longer in the scene.
    pub fn retain_nodes(&mut self, active_ids: &[NodeId]) {
        self.cache.retain(|id, _| active_ids.contains(id));
    }

    /// Clear the entire blur cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Trim the blur cache to half capacity.
    pub fn trim_cache(&mut self) {
        if self.cache.len() > MAX_BLUR_CACHE / 2 {
            let to_remove: Vec<NodeId> = self
                .cache
                .keys()
                .take(self.cache.len() / 2)
                .copied()
                .collect();
            for id in to_remove {
                self.cache.remove(&id);
            }
        }
    }

    /// Current frame number (for diagnostics).
    #[cfg(test)]
    #[must_use]
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Number of cached blur results.
    #[cfg(test)]
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }
}

impl Drop for BlurWorker {
    fn drop(&mut self) {
        // Signal the worker to shut down and wait for it.
        let _ = self.request_tx.send(WorkerMsg::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        debug!("blur worker shut down");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// Create a solid-colour BGRA buffer.
    fn solid_rgba(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let mut buf = vec![0u8; (w * h * 4) as usize];
        for pixel in buf.chunks_exact_mut(4) {
            pixel[0] = b;
            pixel[1] = g;
            pixel[2] = r;
            pixel[3] = a;
        }
        buf
    }

    #[test]
    fn worker_produces_result() {
        let mut worker = BlurWorker::new();

        let pixels = solid_rgba(64, 64, 255, 0, 0, 255);
        worker.request_blur(42, pixels, 64, 64, 4);

        // Give the worker time to process.
        thread::sleep(Duration::from_millis(100));
        worker.poll_results();

        let cached = worker.get_cached(42, 64, 64);
        assert!(cached.is_some(), "expected cached blur result");
        let c = cached.unwrap();
        assert_eq!(c.width, 64);
        assert_eq!(c.height, 64);
        assert_eq!(c.pixels.len(), 64 * 64 * 4);
    }

    #[test]
    fn cache_miss_on_wrong_dimensions() {
        let mut worker = BlurWorker::new();

        let pixels = solid_rgba(32, 32, 0, 255, 0, 255);
        worker.request_blur(10, pixels, 32, 32, 2);

        thread::sleep(Duration::from_millis(100));
        worker.poll_results();

        // Correct dims → hit
        assert!(worker.get_cached(10, 32, 32).is_some());
        // Wrong dims → miss
        assert!(worker.get_cached(10, 64, 64).is_none());
        // Wrong node → miss
        assert!(worker.get_cached(99, 32, 32).is_none());
    }

    #[test]
    fn latest_request_wins() {
        let mut worker = BlurWorker::new();

        // Send multiple requests for the same node rapidly.
        for size in [16, 32, 48, 64] {
            let pixels = solid_rgba(size, size, 128, 128, 128, 255);
            worker.request_blur(1, pixels, size, size, 4);
        }

        thread::sleep(Duration::from_millis(200));
        worker.poll_results();

        // The cache should have the latest dimensions (64×64).
        let cached = worker.get_cached(1, 64, 64);
        assert!(cached.is_some(), "expected 64x64 result");
    }

    #[test]
    fn retain_removes_stale_nodes() {
        let mut worker = BlurWorker::new();

        for id in 1..=3 {
            let pixels = solid_rgba(16, 16, 0, 0, 0, 255);
            worker.request_blur(id, pixels, 16, 16, 2);
        }

        thread::sleep(Duration::from_millis(100));
        worker.poll_results();
        assert_eq!(worker.cache_len(), 3);

        worker.retain_nodes(&[1, 3]);
        assert_eq!(worker.cache_len(), 2);
        assert!(worker.get_cached(1, 16, 16).is_some());
        assert!(worker.get_cached(2, 16, 16).is_none());
        assert!(worker.get_cached(3, 16, 16).is_some());
    }

    #[test]
    fn clear_cache_empties_all() {
        let mut worker = BlurWorker::new();

        let pixels = solid_rgba(16, 16, 0, 0, 0, 255);
        worker.request_blur(1, pixels, 16, 16, 2);

        thread::sleep(Duration::from_millis(100));
        worker.poll_results();
        assert_eq!(worker.cache_len(), 1);

        worker.clear_cache();
        assert_eq!(worker.cache_len(), 0);
    }

    #[test]
    fn drop_joins_worker() {
        // Just ensure drop doesn't hang.
        let worker = BlurWorker::new();
        drop(worker);
    }

    #[test]
    fn fast_path_large_radius() {
        let mut worker = BlurWorker::new();

        let pixels = solid_rgba(128, 128, 100, 150, 200, 255);
        worker.request_blur(5, pixels, 128, 128, 16);

        thread::sleep(Duration::from_millis(200));
        worker.poll_results();

        let cached = worker.get_cached(5, 128, 128);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().pixels.len(), 128 * 128 * 4);
    }

    #[test]
    fn cache_eviction_at_capacity() {
        let mut worker = BlurWorker::new();

        // Insert more entries than MAX_BLUR_CACHE by sending requests
        // for many distinct node IDs.
        for id in 0..(super::MAX_BLUR_CACHE as u64 + 10) {
            let pixels = solid_rgba(4, 4, 0, 0, 0, 255);
            worker.request_blur(id, pixels, 4, 4, 1);
        }

        thread::sleep(Duration::from_millis(500));
        worker.poll_results();

        // After eviction, cache should be at most MAX_BLUR_CACHE.
        assert!(
            worker.cache_len() <= super::MAX_BLUR_CACHE,
            "cache should be bounded: {} > {}",
            worker.cache_len(),
            super::MAX_BLUR_CACHE,
        );
    }

    #[test]
    fn trim_cache_reduces_size() {
        let mut worker = BlurWorker::new();

        for id in 0..10u64 {
            let pixels = solid_rgba(4, 4, 0, 0, 0, 255);
            worker.request_blur(id, pixels, 4, 4, 1);
        }
        thread::sleep(Duration::from_millis(200));
        worker.poll_results();
        assert_eq!(worker.cache_len(), 10);

        worker.trim_cache();
        // trim_cache only acts when above MAX_BLUR_CACHE/2, so 10 < 64 means no change.
        assert_eq!(worker.cache_len(), 10);
    }
}
