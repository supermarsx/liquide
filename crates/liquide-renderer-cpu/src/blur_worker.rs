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
use rayon::prelude::*;
use tracing::debug;

use crate::blur;

/// Maximum number of cached blur results before eviction.
const MAX_BLUR_CACHE: usize = 128;

/// Minimum number of output ROWS a separable blur pass must have before its
/// rows are split into bands and convolved across cores.
///
/// The backdrop blur is COMPUTE-bound — every output pixel runs a full
/// per-tap FMA accumulation across the kernel — so unlike the bandwidth-bound
/// fills (t76, where more threads only contend for memory) it scales with
/// cores. But below a few hundred rows the rayon dispatch + the V-pass halo
/// copies cost more than the convolution saved, so tiny regions stay on the
/// calling thread. The crossover was measured at 480×320/r=16 (see the
/// `blur_parallel_speedup` bench test): the 160-row half-res buffer of a
/// 320-row region already beats serial, while a 64-row region does not.
const PARALLEL_BLUR_MIN_ROWS: u32 = 96;

// ---------------------------------------------------------------------------
// Types exchanged between renderer and worker
// ---------------------------------------------------------------------------

/// A blur job sent to the worker thread.
///
/// `key` is a STABLE cache key derived render-side from the blur region's
/// pixel-snapped geometry, radius and a hash of the underlying backdrop
/// content — NOT the per-frame-churning scene-node id. This is what lets a
/// steady glass surface hit the cache (see `render_backdrop_blur`).
struct BlurRequest {
    key: NodeId,
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
    /// Most recent blur result per stable blur key, used for compositing.
    cache: HashMap<NodeId, CachedBlur>,
    /// Stable blur keys with pending blur requests (submitted since last poll).
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
                    // the latest request per stable blur key.
                    let mut pending: HashMap<NodeId, BlurRequest> = HashMap::new();
                    pending.insert(req.key, req);

                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            WorkerMsg::Shutdown => return,
                            WorkerMsg::Blur(r) => {
                                pending.insert(r.key, r);
                            }
                        }
                    }

                    // Process each unique key's latest request.
                    for (key, req) in pending {
                        let blurred =
                            Self::compute_blur(req.pixels, req.width, req.height, req.radius);

                        let result = CachedBlur {
                            pixels: blurred,
                            width: req.width,
                            height: req.height,
                        };

                        if tx.send((key, result)).is_err() {
                            return; // receiver dropped
                        }
                    }
                }
            }
        }
    }

    /// Compute a blur **synchronously**, byte-identical to the async worker path
    /// ([`compute_blur`](Self::compute_blur)), and insert it into the cache under
    /// `key`. Used by the deterministic capture path so a glass region's blur is
    /// always present and identical run-to-run, instead of depending on whether
    /// the worker thread happened to finish in time (the source of e2e_temporal
    /// blur-pixel flakiness). Returns a borrow of the cached result.
    pub fn compute_blur_blocking(
        &mut self,
        key: NodeId,
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        radius: u32,
    ) -> &CachedBlur {
        let blurred = Self::compute_blur(pixels, width, height, radius);
        self.pending.remove(&key);
        self.cache.insert(
            key,
            CachedBlur {
                pixels: blurred,
                width,
                height,
            },
        );
        self.cache
            .get(&key)
            .expect("just inserted blur result for key")
    }

    /// Perform the actual Gaussian blur on a pixel buffer.
    ///
    /// Uses the fast downsample path for large radii, same as the
    /// synchronous [`blur::blur_fast`] but operating on a standalone buffer
    /// instead of a framebuffer.
    ///
    /// The two separable passes are parallelised across cores with a
    /// DETERMINISTIC fixed-band row split (see [`blur_horizontal_banded`] /
    /// [`blur_vertical_banded`]); the per-pixel SIMD math is untouched, so the
    /// output is bit-for-bit identical to the single-thread path every run.
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
                blur_horizontal_banded(&small, &mut tmp, dw, dh, &kernel);
                blur_vertical_banded(&tmp, &mut blurred, dw, dh, &kernel);
                return blur::blur_upsample_2x_bilinear(&blurred, dw, dh, width, height);
            }
        }

        // Small radius or downsample not possible — blur the full-res buffer.
        blur_buffer_banded(&mut pixels, width, height, radius);
        pixels
    }

    /// Drain completed blur results into the local cache.
    ///
    /// Call this at the start of each frame before rendering.
    pub fn poll_results(&mut self) {
        self.frame += 1;
        while let Ok((key, result)) = self.result_rx.try_recv() {
            self.pending.remove(&key);
            self.cache.insert(key, result);
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

    /// Check whether a blur request is already pending for this key.
    ///
    /// Used to avoid redundant snapshot allocations when the worker
    /// already has a request in-flight.
    pub fn has_pending(&self, key: NodeId) -> bool {
        self.pending.contains(&key)
    }

    /// Look up a cached blur result for a stable blur key.
    ///
    /// Returns `None` if no result is cached or the cached dimensions
    /// don't match (e.g. after a resize).
    pub fn get_cached(&self, key: NodeId, width: u32, height: u32) -> Option<&CachedBlur> {
        self.cache
            .get(&key)
            .filter(|c| c.width == width && c.height == height)
    }

    /// Submit a blur request.  The result will be available on a subsequent
    /// frame via [`get_cached`].
    pub fn request_blur(
        &mut self,
        key: NodeId,
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        radius: u32,
    ) {
        self.pending.insert(key);
        if self
            .request_tx
            .send(WorkerMsg::Blur(BlurRequest {
                key,
                pixels,
                width,
                height,
                radius,
            }))
            .is_err()
        {
            self.pending.remove(&key);
            tracing::warn!(
                "blur worker channel closed; dropping blur request for key {}",
                key
            );
        }
    }

    /// Remove cached entries whose key is not in `active_keys`.
    ///
    /// The blur cache is keyed on stable content/geometry keys (see
    /// `render_backdrop_blur`), so this is a generic key-retain helper; it is
    /// not driven by per-frame scene-node ids. Stale entries are bounded by the
    /// LRU eviction in [`poll_results`] regardless.
    pub fn retain_nodes(&mut self, active_keys: &[NodeId]) {
        self.cache.retain(|id, _| active_keys.contains(id));
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
    #[allow(dead_code)]
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
// Deterministic multi-core separable blur driver
// ---------------------------------------------------------------------------
//
// The blur is COMPUTE-bound (a full per-tap FMA accumulation per output pixel),
// so it scales with cores — unlike the bandwidth-bound fills (t76). We split
// each separable pass into a FIXED set of contiguous row-bands and convolve the
// bands concurrently. Determinism (byte-identical, every run) holds because:
//
//   * the band partition is a pure function of (rows, available_parallelism) —
//     `band_ranges` produces the SAME contiguous ranges run-to-run, so a given
//     output row is ALWAYS computed by the band that owns it (no work-stealing
//     decides WHICH band, only WHEN — rayon may run bands in any order, but each
//     writes DISJOINT output pixels, so order is irrelevant);
//   * there is NO cross-thread reduction — each band fully owns its output rows,
//     so no floating-point sum's order depends on the schedule;
//   * each band calls the SAME `liquide_simd` AVX2/FMA/SSE2 kernel on the SAME
//     source rows it would read single-threaded (the V pass carries a HALO of
//     `half_width` source rows so interior band edges read real neighbour data
//     instead of clamping — see `blur_vertical_banded`). The per-pixel math is
//     therefore identical; only WHICH thread runs it changes.

/// Number of equal contiguous row-bands to split a pass into, capped at the
/// row count and at available parallelism. Returns 1 (serial) below the
/// crossover threshold or on a single core. Pure function of its inputs, so the
/// partition is identical every run.
fn band_count(rows: u32) -> usize {
    if rows < PARALLEL_BLUR_MIN_ROWS {
        return 1;
    }
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    cores.min(rows as usize).max(1)
}

/// Partition `rows` into `bands` contiguous, equal-as-possible, DISJOINT ranges
/// `[start, end)`. The first `rows % bands` ranges get one extra row. This is a
/// pure function — the same `(rows, bands)` always yields the same partition,
/// which is what makes the parallel result deterministic.
fn band_ranges(rows: u32, bands: usize) -> Vec<(u32, u32)> {
    let bands = bands.max(1) as u32;
    let base = rows / bands;
    let rem = rows % bands;
    let mut ranges = Vec::with_capacity(bands as usize);
    let mut start = 0u32;
    for b in 0..bands {
        let extra = u32::from(b < rem);
        let end = start + base + extra;
        ranges.push((start, end));
        start = end;
    }
    ranges
}

/// Horizontal separable pass, row-banded across cores.
///
/// The H pass is fully ROW-LOCAL — output row `y` reads only source row `y`
/// (the kernel slides in x, clamping x within the row) — so a band is just a
/// contiguous slice of `src`/`dst` rows handed to the unchanged kernel with
/// `height = band_rows`. No halo, no cross-band reads, byte-identical to the
/// single-thread call.
fn blur_horizontal_banded(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    kernel: &blur::GaussianKernel,
) {
    let bands = band_count(height);
    if bands <= 1 {
        blur::blur_horizontal(src, dst, width, height, kernel);
        return;
    }

    let row_bytes = (width as usize) * 4;
    let ranges = band_ranges(height, bands);

    // Disjoint output row-bands; each pairs with the SAME source rows. We zip
    // the dst bands with their (start,end) ranges and run them concurrently.
    let mut dst_bands: Vec<&mut [u8]> = Vec::with_capacity(ranges.len());
    let mut rest = dst;
    let mut consumed = 0u32;
    for &(start, end) in &ranges {
        let take = (end - start) as usize * row_bytes;
        debug_assert_eq!(start, consumed);
        let (head, tail) = rest.split_at_mut(take);
        dst_bands.push(head);
        rest = tail;
        consumed = end;
    }

    dst_bands
        .into_par_iter()
        .zip(ranges.par_iter())
        .for_each(|(dst_band, &(start, end))| {
            let band_h = end - start;
            let src_band = &src[start as usize * row_bytes..end as usize * row_bytes];
            blur::blur_horizontal(src_band, dst_band, width, band_h, kernel);
        });
}

/// Vertical separable pass, row-banded across cores.
///
/// The V pass is NOT row-local — output row `y` reads source rows
/// `clamp(y-half ..= y+half)` — so a naive per-band src sub-slice would clamp at
/// the band's INTERNAL top/bottom and produce a SEAM. Instead each band is given
/// a HALO of `half` extra source rows above and below (truncated only at the real
/// image edge). The kernel runs over the halo-extended source into a halo-sized
/// scratch dst, and we copy back only the band's central rows. For every output
/// row in the band the halo supplies the same real neighbour rows the
/// single-thread kernel would read, and the kernel only clamps at the true image
/// top/bottom (the outer bands' halos coincide with it there) — so the result is
/// bit-for-bit identical to the single-thread V pass.
fn blur_vertical_banded(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    kernel: &blur::GaussianKernel,
) {
    let bands = band_count(height);
    if bands <= 1 {
        blur::blur_vertical(src, dst, width, height, kernel);
        return;
    }

    let row_bytes = (width as usize) * 4;
    let half = kernel.half_width as u32;
    let ranges = band_ranges(height, bands);

    let mut dst_bands: Vec<&mut [u8]> = Vec::with_capacity(ranges.len());
    let mut rest = dst;
    for &(start, end) in &ranges {
        let take = (end - start) as usize * row_bytes;
        let (head, tail) = rest.split_at_mut(take);
        dst_bands.push(head);
        rest = tail;
    }

    dst_bands
        .into_par_iter()
        .zip(ranges.par_iter())
        .for_each(|(dst_band, &(start, end))| {
            // Halo: rows [hi_start, hi_end) of the source, clamped to the image.
            let hi_start = start.saturating_sub(half);
            let hi_end = (end + half).min(height);
            let halo_h = hi_end - hi_start;
            let src_halo = &src[hi_start as usize * row_bytes..hi_end as usize * row_bytes];

            let mut scratch = vec![0u8; halo_h as usize * row_bytes];
            blur::blur_vertical(src_halo, &mut scratch, width, halo_h, kernel);

            // The band's output rows [start, end) sit at offset (start - hi_start)
            // within the halo's coordinate space. Copy that central slice back.
            let inner_off = (start - hi_start) as usize * row_bytes;
            let band_bytes = (end - start) as usize * row_bytes;
            dst_band.copy_from_slice(&scratch[inner_off..inner_off + band_bytes]);
        });
}

/// Parallel, byte-identical equivalent of [`blur::blur_buffer`]: in-place
/// two-pass (H then V) separable blur of a standalone BGRA buffer, with each
/// pass row-banded across cores. The intermediate H result lives in `tmp`; the
/// V pass reads `tmp` and writes back into `buf`. Identical output to
/// `blur::blur_buffer` for the same input.
fn blur_buffer_banded(buf: &mut [u8], width: u32, height: u32, radius: u32) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    let kernel = blur::GaussianKernel::new(radius);
    let size = (width as usize) * (height as usize) * 4;
    let mut tmp = vec![0u8; size];
    blur_horizontal_banded(buf, &mut tmp, width, height, &kernel);
    blur_vertical_banded(&tmp, buf, width, height, &kernel);
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

    // ── Deterministic multi-core blur: byte-identical proofs ─────────────
    //
    // The parallel driver MUST be bit-for-bit identical to the single-thread
    // SIMD path for the same input, EVERY run — the capture path + goldens +
    // e2e_temporal depend on it. These tests run BOTH paths in-process and
    // assert EXACT equality (assert_eq, never ±1) across region sizes (incl.
    // sizes not divisible by the band count, sizes below the threshold) and
    // radii, and repeat to catch any scheduling-dependent nondeterminism.

    /// Deterministic LCG so the "random" pixel inputs are reproducible.
    fn lcg_fill(n: usize, mut seed: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            v.push((seed >> 33) as u8);
        }
        v
    }

    /// SINGLE-THREAD reference for a horizontal pass — the unbanded kernel.
    fn ref_h(src: &[u8], w: u32, h: u32, kernel: &blur::GaussianKernel) -> Vec<u8> {
        let mut dst = vec![0u8; src.len()];
        blur::blur_horizontal(src, &mut dst, w, h, kernel);
        dst
    }

    /// SINGLE-THREAD reference for a vertical pass — the unbanded kernel.
    fn ref_v(src: &[u8], w: u32, h: u32, kernel: &blur::GaussianKernel) -> Vec<u8> {
        let mut dst = vec![0u8; src.len()];
        blur::blur_vertical(src, &mut dst, w, h, kernel);
        dst
    }

    // Sizes chosen to stress the band split: well above the threshold, NOT
    // divisible by typical core counts, and tiny sizes BELOW the threshold
    // (must hit the serial fast path and still match exactly).
    const PARALLEL_TEST_DIMS: [(u32, u32); 7] = [
        (480, 320), // representative glass region
        (97, 211),  // prime-ish, not divisible by core count, just above threshold
        (256, 257), // odd remainder rows
        (33, 96),   // exactly at the threshold
        (64, 64),   // below threshold → serial fallback
        (8, 8),     // tiny → serial fallback
        (200, 150), // larger general
    ];
    const PARALLEL_TEST_RADII: [u32; 8] = [1, 2, 4, 7, 8, 9, 16, 32];

    #[test]
    fn band_ranges_partition_is_contiguous_disjoint_and_total() {
        // The partition is the determinism foundation: contiguous, gap-free,
        // covering exactly [0, rows). Proven for many (rows, bands) pairs.
        for rows in [1u32, 2, 5, 96, 97, 100, 256, 257, 480] {
            for bands in 1usize..=12 {
                let ranges = super::band_ranges(rows, bands);
                assert_eq!(ranges.len(), bands.max(1), "band count");
                assert_eq!(ranges[0].0, 0, "starts at 0");
                assert_eq!(ranges.last().unwrap().1, rows, "ends at rows");
                for w in ranges.windows(2) {
                    assert_eq!(w[0].1, w[1].0, "contiguous (no gap/overlap)");
                }
                // Band sizes differ by at most 1 (balanced).
                let sizes: Vec<u32> = ranges.iter().map(|&(s, e)| e - s).collect();
                let max = *sizes.iter().max().unwrap();
                let min = *sizes.iter().min().unwrap();
                assert!(max - min <= 1, "bands balanced within 1 row");
            }
        }
    }

    #[test]
    fn horizontal_banded_is_bit_identical_to_single_thread() {
        for (w, h) in PARALLEL_TEST_DIMS {
            let src = lcg_fill((w * h * 4) as usize, 0x1111 ^ (w as u64) << 16 ^ h as u64);
            for &r in &PARALLEL_TEST_RADII {
                let kernel = blur::GaussianKernel::new(r);
                let reference = ref_h(&src, w, h, &kernel);
                // Repeat so any scheduling-dependent divergence is caught.
                for run in 0..4 {
                    let mut got = vec![0u8; src.len()];
                    super::blur_horizontal_banded(&src, &mut got, w, h, &kernel);
                    assert_eq!(
                        got, reference,
                        "H pass diverged: w={w} h={h} r={r} run={run} (must be bit-identical)"
                    );
                }
            }
        }
    }

    #[test]
    fn vertical_banded_is_bit_identical_to_single_thread() {
        for (w, h) in PARALLEL_TEST_DIMS {
            let src = lcg_fill((w * h * 4) as usize, 0x2222 ^ (w as u64) << 16 ^ h as u64);
            for &r in &PARALLEL_TEST_RADII {
                let kernel = blur::GaussianKernel::new(r);
                let reference = ref_v(&src, w, h, &kernel);
                for run in 0..4 {
                    let mut got = vec![0u8; src.len()];
                    super::blur_vertical_banded(&src, &mut got, w, h, &kernel);
                    assert_eq!(
                        got, reference,
                        "V pass diverged: w={w} h={h} r={r} run={run} (must be bit-identical)"
                    );
                }
            }
        }
    }

    #[test]
    fn full_compute_blur_is_bit_identical_across_runs() {
        // End-to-end: the worker's compute_blur (incl. the r>=8 downsample
        // regime whose half-res H/V passes are banded) must produce the SAME
        // bytes every run. We compare each run to the first.
        for (w, h) in PARALLEL_TEST_DIMS {
            for &r in &PARALLEL_TEST_RADII {
                let src = lcg_fill((w * h * 4) as usize, 0x3333 ^ r as u64);
                let first = BlurWorker::compute_blur(src.clone(), w, h, r);
                for run in 1..5 {
                    let again = BlurWorker::compute_blur(src.clone(), w, h, r);
                    assert_eq!(
                        again, first,
                        "compute_blur nondeterministic: w={w} h={h} r={r} run={run}"
                    );
                }
            }
        }
    }

    #[test]
    fn blocking_path_matches_async_compute() {
        // The capture path uses compute_blur_blocking; it must yield exactly
        // what the async worker's compute_blur produces for the same input.
        let (w, h, r) = (480u32, 320u32, 16u32);
        let src = lcg_fill((w * h * 4) as usize, 0x4444);
        let expected = BlurWorker::compute_blur(src.clone(), w, h, r);

        let mut worker = BlurWorker::new();
        let cached = worker.compute_blur_blocking(7, src, w, h, r);
        assert_eq!(cached.pixels, expected, "blocking path diverged from async");
    }

    #[test]
    fn parallel_blur_actually_runs_multiple_bands() {
        // Guard the premise: at a realistic glass size the driver MUST split
        // into more than one band when the machine has more than one core.
        // (If it silently stayed serial, the "byte-identical" tests would pass
        // trivially and the parallelism would be fake-green.)
        let cores = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        if cores > 1 {
            assert!(
                super::band_count(320) > 1,
                "320-row region should split across {cores} cores"
            );
        }
        // Below the threshold it MUST stay serial regardless of core count.
        assert_eq!(super::band_count(64), 1, "64 rows is below threshold");
        assert_eq!(super::band_count(8), 1, "8 rows is below threshold");
    }

    // ── Teeth: an off-by-one halo creates a SEAM → RED ──────────────────
    //
    // The whole correctness of the V-pass band split rests on the HALO: a band
    // reads `half` source rows beyond its own range so interior edges read real
    // neighbour data instead of clamping. This test reimplements the V pass with
    // a DELIBERATELY BROKEN halo (one row too short) and asserts it DIVERGES from
    // the single-thread reference — proving the test would catch a seam and that
    // the correct halo is load-bearing, not decorative.

    /// Same structure as `blur_vertical_banded` but with the halo short by one
    /// row on each side — a realistic off-by-one. Forced to >1 band.
    fn broken_vertical_short_halo(
        src: &[u8],
        dst: &mut [u8],
        width: u32,
        height: u32,
        kernel: &blur::GaussianKernel,
    ) {
        let row_bytes = (width as usize) * 4;
        // half_minus_one: the bug. (half saturates to 0, so use a kernel r>=2.)
        let half = (kernel.half_width as u32).saturating_sub(1);
        let bands = 4.min(height as usize).max(1);
        let ranges = super::band_ranges(height, bands);
        for &(start, end) in &ranges {
            let hi_start = start.saturating_sub(half);
            let hi_end = (end + half).min(height);
            let halo_h = hi_end - hi_start;
            let src_halo = &src[hi_start as usize * row_bytes..hi_end as usize * row_bytes];
            let mut scratch = vec![0u8; halo_h as usize * row_bytes];
            blur::blur_vertical(src_halo, &mut scratch, width, halo_h, kernel);
            let inner_off = (start - hi_start) as usize * row_bytes;
            let band_bytes = (end - start) as usize * row_bytes;
            dst[start as usize * row_bytes..end as usize * row_bytes]
                .copy_from_slice(&scratch[inner_off..inner_off + band_bytes]);
        }
    }

    #[test]
    fn teeth_short_halo_produces_a_seam_and_diverges() {
        // A radius large enough that an interior band boundary lands inside the
        // kernel reach, and a height that forces multiple bands.
        let (w, h, r) = (32u32, 200u32, 6u32);
        let src = lcg_fill((w * h * 4) as usize, 0x5A5A);
        let kernel = blur::GaussianKernel::new(r);

        let reference = ref_v(&src, w, h, &kernel);

        // Correct banded path matches exactly...
        let mut good = vec![0u8; src.len()];
        super::blur_vertical_banded(&src, &mut good, w, h, &kernel);
        assert_eq!(good, reference, "correct halo must match single-thread");

        // ...the off-by-one halo must NOT (it clamps at interior band edges,
        // leaving a seam). If this ever matched, the byte-identical test above
        // would be toothless.
        let mut bad = vec![0u8; src.len()];
        broken_vertical_short_halo(&src, &mut bad, w, h, &kernel);
        assert_ne!(
            bad, reference,
            "short halo should create a seam — the test must have teeth"
        );
    }

    /// Hand benchmark: single-thread vs banded parallel separable blur over a
    /// realistic glass region and a larger one. Ignored by default; run with:
    ///   cargo test -p liquide-renderer-cpu --offline --release \
    ///     blur_worker::tests::blur_parallel_speedup -- --ignored --nocapture
    #[test]
    #[ignore]
    fn blur_parallel_speedup() {
        use std::time::Instant;
        let cores = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        eprintln!("available_parallelism = {cores}");

        for (w, h) in [(480u32, 320u32), (1280, 800)] {
            let src = lcg_fill((w * h * 4) as usize, 0x9E37_79B9);
            for &r in &[8u32, 16, 32] {
                let kernel = blur::GaussianKernel::new(r);
                let size = src.len();
                let iters = 50;

                // Single-thread reference: direct kernel calls (serial).
                let mut tmp = vec![0u8; size];
                let mut out = vec![0u8; size];
                let t = Instant::now();
                for _ in 0..iters {
                    blur::blur_horizontal(&src, &mut tmp, w, h, &kernel);
                    blur::blur_vertical(&tmp, &mut out, w, h, &kernel);
                }
                let serial_ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;

                // Banded parallel.
                let t = Instant::now();
                for _ in 0..iters {
                    super::blur_horizontal_banded(&src, &mut tmp, w, h, &kernel);
                    super::blur_vertical_banded(&tmp, &mut out, w, h, &kernel);
                }
                let par_ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;

                eprintln!(
                    "blur {w}x{h} r={r:>2}: serial={serial_ms:7.3}ms  parallel={par_ms:7.3}ms  \
                     speedup={:.2}x  (bands={})",
                    serial_ms / par_ms,
                    super::band_count(h),
                );
            }
        }
    }
}
