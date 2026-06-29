//! Per-owner cached pixel surfaces for the live compositor (t2 Phase P1).
//!
//! This module is the **retained PIXEL half** of the compositor. liquide already
//! has a retained *scene* (`WindowSceneCache`, keyed on a position-independent
//! content signature) but no per-window / per-layer cached *bitmap*: on any
//! damaged frame the renderer re-rasters every window's Decoration + Gradient +
//! Glass from scene nodes. A real OS compositor caches each window as a bitmap
//! and re-composites only the regions that changed.
//!
//! [`SurfaceCache`] is that store. It is a **pure data structure**: it holds one
//! cached [`SurfaceBuffer`] per [`SurfaceOwner`] (the wallpaper, each window,
//! each isolated chrome layer), keyed by a [`SurfaceKey`] (content signature,
//! physical size, DPI, and — for glass — a backdrop signature). It performs no
//! rendering and never changes rendered output by itself; the render thread
//! (E3) blits a cached surface on a HIT and re-rasters + [`insert`](SurfaceCache::insert)s
//! on a MISS.
//!
//! ## Invalidation truth (single-sourced)
//!
//! - A **content change** bumps `content_sig` (folded from the
//!   `WindowContentSignature` the scene cache already trusts) → the key no longer
//!   matches → automatic MISS, with no separate dirty-tracking.
//! - A **move** (x/y only) changes neither `content_sig`, `size`, nor `dpi_scale`
//!   (position is *not* in the key — that is the whole point of the
//!   position-independent content signature) → HIT → the surface is reused and
//!   re-blitted at the new position.
//! - A **resize or DPI change** changes `size` / `dpi_scale` → MISS → a fresh
//!   surface is rastered and the old one is dropped (its bytes reclaimed). A
//!   surface is never silently stretched.
//! - A **glass backdrop change** flips `backdrop_sig` → MISS → re-blur.
//!
//! ## Memory
//!
//! A `~256 MB` LRU budget (override with `LIQUIDE_SURFACE_CACHE_BUDGET_MB`).
//! Eviction is LRU by `last_used_frame`, evicting offscreen / occluded surfaces
//! first (they carry an old `last_used_frame` because the composite loop skipped
//! them) and **never** evicting a surface composited in the current frame.
//! Eviction is leak-free: the evicted surface's backing `Vec<u8>` is returned to
//! the shared [`FrameMemoryPool`] for reuse by the next same-size raster.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::framebuffer::{FrameBuffer, FrameMemory, FrameMemoryPool};
use crate::scene::SurfaceBuffer;

/// Default surface-cache memory budget when the env override is unset (256 MB).
pub const DEFAULT_SURFACE_CACHE_BUDGET_MB: usize = 256;

/// Environment variable that overrides the cache budget (in whole megabytes).
pub const SURFACE_CACHE_BUDGET_ENV: &str = "LIQUIDE_SURFACE_CACHE_BUDGET_MB";

/// Identifies *which* compositor element a cached surface belongs to.
///
/// The store is keyed on this (one cached surface per owner) so a key MISMATCH
/// (sig / size / dpi / backdrop changed) is detected as a miss and the surface
/// is re-rastered in place for the same owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceOwner {
    /// The desktop wallpaper / background layer — one screen-sized surface.
    Wallpaper,
    /// A toplevel window (content + its decoration / border / shadow chrome).
    Window(u64),
    /// An isolated chrome layer (statusbar, dock, fixed/overlay glass band).
    Layer(u64),
}

/// The per-surface cache key (invalidation signature).
///
/// Two keys are equal iff the cached pixels are reusable verbatim. Position is
/// deliberately **absent** — a pure move keeps the key (the surface is reused and
/// re-blitted at the new position), exactly the position-independent property the
/// content signature already guarantees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceKey {
    /// Hash of the owner's `WindowContentSignature` (or layer chrome signature).
    /// Position-independent: a content change flips it, a move does not.
    pub content_sig: u64,
    /// Footprint size in PHYSICAL pixels (post-DPI). A resize → new surface.
    pub size: (u32, u32),
    /// Bit pattern of the DPI scale (`f32::to_bits`). A DPI change → new surface.
    pub dpi_scale: u32,
    /// `None` for opaque owners; `Some(crc)` of the live backdrop for glass.
    /// A change behind a glass surface flips this → re-blur.
    pub backdrop_sig: Option<u64>,
}

impl SurfaceKey {
    /// Construct a key for an **opaque** owner (no backdrop dependency).
    #[must_use]
    pub fn opaque(content_sig: u64, size: (u32, u32), dpi_scale: f32) -> Self {
        Self {
            content_sig,
            size,
            dpi_scale: dpi_scale.to_bits(),
            backdrop_sig: None,
        }
    }

    /// Construct a key for a **glass** owner whose pixels depend on `backdrop_sig`.
    #[must_use]
    pub fn glass(content_sig: u64, size: (u32, u32), dpi_scale: f32, backdrop_sig: u64) -> Self {
        Self {
            content_sig,
            size,
            dpi_scale: dpi_scale.to_bits(),
            backdrop_sig: Some(backdrop_sig),
        }
    }
}

/// A cached surface plus its key and last-composited frame (LRU stamp).
#[derive(Debug, Clone)]
pub struct CachedSurface {
    /// The key these pixels were captured under.
    pub key: SurfaceKey,
    /// The cached pixels (`Arc` → cloning for a blit is an atomic increment).
    pub buffer: SurfaceBuffer,
    /// Frame index when this surface was last composited (the LRU stamp).
    pub last_used_frame: u64,
    /// Cached `buffer.pixels.len()` so budget math never re-walks the `Arc`.
    pub bytes: usize,
}

/// Cumulative store statistics (for tests / perf profiling — not load-bearing).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfaceCacheStats {
    /// `get` calls that found a matching key (a reuse).
    pub hits: u64,
    /// `get` calls that missed (absent owner or key mismatch).
    pub misses: u64,
    /// Surfaces dropped by budget eviction or `invalidate`.
    pub evictions: u64,
    /// Backing buffers successfully returned to the `FrameMemoryPool`.
    pub pooled: u64,
}

/// Number of bytes a [`SurfaceBuffer`] occupies (its backing pixel vector).
#[must_use]
fn surface_bytes(buf: &SurfaceBuffer) -> usize {
    buf.pixels.len()
}

/// Per-owner cached pixel-surface store with an LRU memory budget.
///
/// Pure: no rendering. The render thread (E3) drives it as:
///
/// ```ignore
/// cache.begin_frame();
/// for owner in surfaces_back_to_front {
///     let key = key_for(owner, job);            // sig + size + dpi (+ backdrop for glass)
///     match cache.get(owner, key) {
///         Some(buf) => blit(buf, owner.pos),    // HIT — reuse cached pixels
///         None => {                             // MISS — raster then store
///             let surface = raster_owner_to_surface(owner); // E2 path
///             cache.insert(owner, key, surface);
///             blit(cache.get(owner, key).unwrap(), owner.pos);
///         }
///     }
/// }
/// cache.end_frame();                            // LRU-evict to budget
/// ```
pub struct SurfaceCache {
    /// One cached surface per owner; key mismatch ⇒ stale ⇒ miss.
    store: HashMap<SurfaceOwner, CachedSurface>,
    /// Recycles evicted backing memory so a same-size re-raster never re-allocates.
    pool: FrameMemoryPool,
    /// Soft memory ceiling in bytes (LRU eviction targets this).
    budget_bytes: usize,
    /// Sum of `bytes` over all stored surfaces (kept in sync incrementally).
    total_bytes: usize,
    /// Monotonic frame counter; bumped by `begin_frame`.
    current_frame: u64,
    /// Owners composited during the current frame — NEVER evicted this frame.
    composited_this_frame: HashSet<SurfaceOwner>,
    /// Whether we are between `begin_frame` and `end_frame`.
    in_frame: bool,
    /// Cumulative stats.
    stats: SurfaceCacheStats,
}

impl Default for SurfaceCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SurfaceCache {
    /// Create a cache with the budget from `LIQUIDE_SURFACE_CACHE_BUDGET_MB`
    /// (default 256 MB). A non-parseable or zero value falls back to the default.
    #[must_use]
    pub fn new() -> Self {
        let mb = std::env::var(SURFACE_CACHE_BUDGET_ENV)
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_SURFACE_CACHE_BUDGET_MB);
        Self::with_budget_bytes(mb.saturating_mul(1024 * 1024))
    }

    /// Create a cache with an explicit byte budget (bypasses the env var).
    #[must_use]
    pub fn with_budget_bytes(budget_bytes: usize) -> Self {
        Self {
            store: HashMap::new(),
            pool: FrameMemoryPool::new(),
            budget_bytes,
            total_bytes: 0,
            current_frame: 0,
            composited_this_frame: HashSet::new(),
            in_frame: false,
            stats: SurfaceCacheStats::default(),
        }
    }

    /// The configured byte budget.
    #[must_use]
    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// Current total bytes held across all cached surfaces.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Number of owners with a cached surface.
    #[must_use]
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Whether the store holds no surfaces.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// The current frame index (bumped by [`begin_frame`](Self::begin_frame)).
    #[must_use]
    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// Cumulative store statistics.
    #[must_use]
    pub fn stats(&self) -> SurfaceCacheStats {
        self.stats
    }

    /// Whether `owner` has a cached surface (regardless of key freshness).
    #[must_use]
    pub fn contains(&self, owner: SurfaceOwner) -> bool {
        self.store.contains_key(&owner)
    }

    /// Borrow the [`FrameMemoryPool`] (for the offscreen raster targets E3 draws
    /// from, so re-rastering a surface never allocates a fresh megabyte buffer).
    pub fn pool_mut(&mut self) -> &mut FrameMemoryPool {
        &mut self.pool
    }

    /// Begin a composite frame: bump the frame counter and clear the
    /// "composited this frame" set. Surfaces reused or inserted before the
    /// matching [`end_frame`](Self::end_frame) are protected from eviction.
    pub fn begin_frame(&mut self) {
        self.current_frame = self.current_frame.wrapping_add(1);
        self.composited_this_frame.clear();
        self.in_frame = true;
    }

    /// End a composite frame and LRU-evict down to the budget.
    pub fn end_frame(&mut self) {
        self.in_frame = false;
        self.evict_to_budget();
    }

    /// Look up a reusable surface for `owner` under `want_key`.
    ///
    /// Returns `Some(&buffer)` only when the stored key matches *exactly*
    /// (content_sig + size + dpi + backdrop) — a HIT. On a hit the surface's LRU
    /// stamp is refreshed and the owner is marked composited-this-frame (so the
    /// following [`end_frame`](Self::end_frame) will not evict it). An absent
    /// owner, or a stored key that differs in any field, is a MISS (`None`).
    pub fn get(&mut self, owner: SurfaceOwner, want_key: SurfaceKey) -> Option<&SurfaceBuffer> {
        let frame = self.current_frame;
        match self.store.get_mut(&owner) {
            Some(cached) if cached.key == want_key => {
                cached.last_used_frame = frame;
                self.composited_this_frame.insert(owner);
                self.stats.hits += 1;
                Some(&cached.buffer)
            }
            _ => {
                self.stats.misses += 1;
                None
            }
        }
    }

    /// Borrow the cached surface for `owner` without affecting LRU / stats.
    ///
    /// Returns the entry only if its key matches `want_key`. Read-only probe for
    /// tests and diagnostics — prefer [`get`](Self::get) on the composite path.
    #[must_use]
    pub fn peek(&self, owner: SurfaceOwner, want_key: SurfaceKey) -> Option<&SurfaceBuffer> {
        self.store
            .get(&owner)
            .filter(|c| c.key == want_key)
            .map(|c| &c.buffer)
    }

    /// Borrow the raw cached entry for `owner` (any key). Diagnostics only.
    #[must_use]
    pub fn entry(&self, owner: SurfaceOwner) -> Option<&CachedSurface> {
        self.store.get(&owner)
    }

    /// Store `surface` for `owner` under `key`, replacing any prior surface for
    /// that owner (its bytes are reclaimed to the pool). The owner is marked
    /// composited-this-frame: a just-rastered surface was just composited and
    /// must not be evicted by this frame's `end_frame`.
    pub fn insert(&mut self, owner: SurfaceOwner, key: SurfaceKey, surface: SurfaceBuffer) {
        let bytes = surface_bytes(&surface);
        let cached = CachedSurface {
            key,
            buffer: surface,
            last_used_frame: self.current_frame,
            bytes,
        };
        // Replace any prior surface for this owner, reclaiming its memory.
        if let Some(old) = self.store.insert(owner, cached) {
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
            self.reclaim(old.buffer);
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.composited_this_frame.insert(owner);
    }

    /// Explicitly mark `owner` as composited this frame (eviction-protected),
    /// e.g. when E3 reuses a surface via a path other than [`get`](Self::get)
    /// (such as a blit-move translate of a previously-fetched surface).
    pub fn mark_composited(&mut self, owner: SurfaceOwner) {
        if self.store.contains_key(&owner) {
            self.composited_this_frame.insert(owner);
        }
    }

    /// Drop the cached surface for `owner` (e.g. window closed / wallpaper
    /// changed), reclaiming its backing memory to the pool. No-op if absent.
    pub fn invalidate(&mut self, owner: SurfaceOwner) {
        if let Some(old) = self.store.remove(&owner) {
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
            self.composited_this_frame.remove(&owner);
            self.stats.evictions += 1;
            self.reclaim(old.buffer);
        }
    }

    /// Drop every cached surface, reclaiming all backing memory to the pool.
    pub fn clear(&mut self) {
        let owners: Vec<SurfaceOwner> = self.store.keys().copied().collect();
        for owner in owners {
            if let Some(old) = self.store.remove(&owner) {
                self.reclaim(old.buffer);
            }
        }
        self.total_bytes = 0;
        self.composited_this_frame.clear();
    }

    /// Evict least-recently-used surfaces until `total_bytes <= budget_bytes`.
    ///
    /// Eviction order is ascending `last_used_frame` (offscreen / occluded
    /// surfaces sort first because the composite loop skipped them, leaving an
    /// old stamp). Surfaces composited THIS frame are never evicted — if only
    /// live surfaces remain, eviction stops even if still over budget (a live
    /// surface must not be dropped mid-frame).
    pub fn evict_to_budget(&mut self) {
        if self.total_bytes <= self.budget_bytes {
            return;
        }
        // Candidates: everything NOT composited this frame, oldest first.
        let mut candidates: Vec<(SurfaceOwner, u64)> = self
            .store
            .iter()
            .filter(|(owner, _)| !self.composited_this_frame.contains(owner))
            .map(|(owner, c)| (*owner, c.last_used_frame))
            .collect();
        // Sort by last_used_frame ascending (LRU first). Tie-break on the owner's
        // ordinal so eviction is deterministic regardless of HashMap iteration.
        candidates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| owner_ord(a.0).cmp(&owner_ord(b.0))));

        for (owner, _) in candidates {
            if self.total_bytes <= self.budget_bytes {
                break;
            }
            if let Some(old) = self.store.remove(&owner) {
                self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
                self.stats.evictions += 1;
                self.reclaim(old.buffer);
            }
        }
    }

    /// Return a surface's backing `Vec<u8>` to the pool if we hold the only
    /// strong reference (the common case for a pure store — `get` hands out
    /// borrows, not clones). If the buffer is still shared (a blit clone is in
    /// flight) or GPU-shaped, it is simply dropped — never leaked.
    fn reclaim(&mut self, surface: SurfaceBuffer) {
        let SurfaceBuffer {
            pixels,
            width,
            height,
            stride,
            format,
        } = surface;
        // Only reclaim a tightly-packed CPU buffer whose size the pool can key
        // by (width, height, format); the pool assumes stride == width * bpp.
        if stride != width * format.bytes_per_pixel() {
            return;
        }
        match Arc::try_unwrap(pixels) {
            Ok(mem) => {
                let fb = FrameBuffer {
                    memory: FrameMemory::Cpu(mem),
                    width,
                    height,
                    stride,
                    format,
                };
                self.pool.release(fb);
                self.stats.pooled += 1;
            }
            Err(_shared) => { /* still referenced elsewhere — drop, no leak */ }
        }
    }
}

/// Stable ordinal for deterministic eviction tie-breaking.
#[inline]
fn owner_ord(owner: SurfaceOwner) -> (u8, u64) {
    match owner {
        SurfaceOwner::Wallpaper => (0, 0),
        SurfaceOwner::Window(id) => (1, id),
        SurfaceOwner::Layer(id) => (2, id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pixel::PixelFormat;

    /// Build a tightly-packed BGRA surface of `w * h` pixels (the pool-reclaimable
    /// shape: `stride == width * 4`). Filled with `fill` so distinct surfaces are
    /// distinguishable, though the cache never inspects pixel values.
    fn surface(w: u32, h: u32, fill: u8) -> SurfaceBuffer {
        SurfaceBuffer {
            pixels: Arc::new(vec![fill; (w * h * 4) as usize]),
            width: w,
            height: h,
            stride: w * 4,
            format: PixelFormat::Bgra8,
        }
    }

    fn key(content_sig: u64, size: (u32, u32)) -> SurfaceKey {
        SurfaceKey::opaque(content_sig, size, 1.0)
    }

    // ── get-after-insert HIT ─────────────────────────────────────────────────

    #[test]
    fn get_after_insert_is_hit() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        let k = key(0xABCD, (100, 80));
        cache.insert(SurfaceOwner::Window(1), k, surface(100, 80, 7));

        let got = cache.get(SurfaceOwner::Window(1), k);
        assert!(got.is_some(), "get after insert must HIT");
        assert_eq!(got.unwrap().width, 100);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 0);
    }

    #[test]
    fn get_missing_owner_is_miss() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        assert!(cache.get(SurfaceOwner::Window(99), key(1, (10, 10))).is_none());
        assert_eq!(cache.stats().misses, 1);
    }

    // ── content_sig change ⇒ MISS ────────────────────────────────────────────

    #[test]
    fn content_sig_change_invalidates_surface() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        cache.insert(SurfaceOwner::Window(1), key(0x1111, (100, 80)), surface(100, 80, 1));

        // Same owner, same size/dpi, DIFFERENT content_sig ⇒ stale ⇒ miss.
        let stale = cache.get(SurfaceOwner::Window(1), key(0x2222, (100, 80)));
        assert!(stale.is_none(), "content change must MISS");
        assert_eq!(cache.stats().misses, 1);
    }

    // ── size change ⇒ MISS, new surface, old evicted ─────────────────────────

    #[test]
    fn size_change_allocates_new_and_evicts_old() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        cache.insert(SurfaceOwner::Window(1), key(0x1111, (100, 80)), surface(100, 80, 1));
        let bytes_before = cache.total_bytes();
        assert_eq!(bytes_before, 100 * 80 * 4);

        // Resize: new size in key ⇒ miss.
        let new_key = key(0x1111, (200, 80));
        assert!(cache.get(SurfaceOwner::Window(1), new_key).is_none(), "resize must MISS");

        // E3 re-rasters and re-inserts at the new size; old surface dropped.
        cache.insert(SurfaceOwner::Window(1), new_key, surface(200, 80, 2));
        assert_eq!(cache.len(), 1, "still one owner, old surface replaced not duplicated");
        assert_eq!(cache.total_bytes(), 200 * 80 * 4, "bytes reflect the NEW size only");
        // Old buffer's memory was reclaimed to the pool.
        assert_eq!(cache.stats().pooled, 1, "old surface returned to pool, no leak");
    }

    // ── DPI change ⇒ MISS ────────────────────────────────────────────────────

    #[test]
    fn dpi_change_invalidates_surface() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        let k1 = SurfaceKey::opaque(0x1111, (100, 80), 1.0);
        cache.insert(SurfaceOwner::Window(1), k1, surface(100, 80, 1));

        let k2 = SurfaceKey::opaque(0x1111, (100, 80), 2.0); // DPI bump only
        assert!(cache.get(SurfaceOwner::Window(1), k2).is_none(), "DPI change must MISS");
    }

    // ── MOVE keeps the key ⇒ HIT, zero re-raster ─────────────────────────────

    #[test]
    fn move_reuses_same_entry_zero_reraster() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        // The key is position-independent. A move changes neither content_sig,
        // size, nor dpi — so the SAME key is queried next frame.
        let k = key(0x1111, (100, 80));
        cache.insert(SurfaceOwner::Window(1), k, surface(100, 80, 1));
        cache.end_frame();

        // Next frame: window moved (only x/y changed) — same key.
        cache.begin_frame();
        let hit = cache.get(SurfaceOwner::Window(1), k);
        assert!(hit.is_some(), "a move (same key) must HIT — reuse + reblit at new pos");
        // No re-raster occurred: the only insert was the original one.
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.len(), 1);
        cache.end_frame();
    }

    // ── glass backdrop change ⇒ MISS ─────────────────────────────────────────

    #[test]
    fn glass_backdrop_change_invalidates() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        let g1 = SurfaceKey::glass(0x1111, (100, 80), 1.0, 0xAAAA);
        cache.insert(SurfaceOwner::Layer(1), g1, surface(100, 80, 1));

        // Own content unchanged, but the backdrop crc changed ⇒ stale ⇒ miss.
        let g2 = SurfaceKey::glass(0x1111, (100, 80), 1.0, 0xBBBB);
        assert!(cache.get(SurfaceOwner::Layer(1), g2).is_none(), "backdrop change must MISS");

        // Identical backdrop ⇒ hit (blit, no re-blur).
        assert!(cache.get(SurfaceOwner::Layer(1), g1).is_some(), "same backdrop must HIT");
    }

    // ── LRU eviction: least-recently-used dropped first, budget bounded ───────

    #[test]
    fn lru_evicts_least_recently_used_and_bounds_memory() {
        // Budget for exactly two 100x80 BGRA surfaces (= 64000 bytes).
        let one = (100 * 80 * 4) as usize;
        let mut cache = SurfaceCache::with_budget_bytes(2 * one);

        cache.begin_frame(); // frame 1
        cache.insert(SurfaceOwner::Window(1), key(1, (100, 80)), surface(100, 80, 1));
        cache.end_frame();

        cache.begin_frame(); // frame 2
        cache.insert(SurfaceOwner::Window(2), key(2, (100, 80)), surface(100, 80, 2));
        cache.end_frame();
        assert_eq!(cache.len(), 2, "two surfaces fit the budget");
        assert!(cache.total_bytes() <= cache.budget_bytes());

        cache.begin_frame(); // frame 3
        // Touch W2 so it is the more-recently-used of the two old surfaces.
        assert!(cache.get(SurfaceOwner::Window(2), key(2, (100, 80))).is_some());
        // Insert a third surface — now over budget.
        cache.insert(SurfaceOwner::Window(3), key(3, (100, 80)), surface(100, 80, 3));
        cache.end_frame();

        // W1 (oldest, not touched this frame) is evicted; W2 + W3 survive.
        assert!(!cache.contains(SurfaceOwner::Window(1)), "LRU victim W1 evicted");
        assert!(cache.contains(SurfaceOwner::Window(2)));
        assert!(cache.contains(SurfaceOwner::Window(3)));
        assert!(cache.total_bytes() <= cache.budget_bytes(), "budget respected");
        assert!(cache.stats().evictions >= 1);
    }

    // ── a surface composited THIS frame is never evicted ─────────────────────

    #[test]
    fn surface_composited_this_frame_is_never_evicted() {
        let one = (100 * 80 * 4) as usize;
        // Budget for two; we will hold three live this frame to force the
        // protection rule (eviction must NOT touch live surfaces).
        let mut cache = SurfaceCache::with_budget_bytes(2 * one);

        cache.begin_frame(); // frame 1: seed an OLD offscreen surface
        cache.insert(SurfaceOwner::Window(1), key(1, (100, 80)), surface(100, 80, 1));
        cache.end_frame();

        cache.begin_frame(); // frame 2
        // Composite (reuse) the live window we want protected.
        let live_key = key(1, (100, 80));
        assert!(cache.get(SurfaceOwner::Window(1), live_key).is_some());
        // Add two more live surfaces → 3 surfaces, way over the 2-surface budget.
        cache.insert(SurfaceOwner::Window(2), key(2, (100, 80)), surface(100, 80, 2));
        cache.insert(SurfaceOwner::Window(3), key(3, (100, 80)), surface(100, 80, 3));
        cache.end_frame();

        // All three were composited this frame → none may be evicted, even
        // though we are over budget (correctness over budget for live pixels).
        assert!(cache.contains(SurfaceOwner::Window(1)), "live W1 must survive eviction");
        assert!(cache.contains(SurfaceOwner::Window(2)));
        assert!(cache.contains(SurfaceOwner::Window(3)));
        assert!(cache.total_bytes() > cache.budget_bytes(), "over budget but no live drop");
    }

    #[test]
    fn live_surface_protected_offscreen_evicted_first() {
        let one = (100 * 80 * 4) as usize;
        let mut cache = SurfaceCache::with_budget_bytes(2 * one);

        // Seed two offscreen surfaces over two frames (W1 older than W2).
        cache.begin_frame();
        cache.insert(SurfaceOwner::Window(1), key(1, (100, 80)), surface(100, 80, 1));
        cache.end_frame();
        cache.begin_frame();
        cache.insert(SurfaceOwner::Window(2), key(2, (100, 80)), surface(100, 80, 2));
        cache.end_frame();

        cache.begin_frame(); // frame 3
        // Composite ONLY W2 (it is live/onscreen); W1 stays offscreen (untouched).
        assert!(cache.get(SurfaceOwner::Window(2), key(2, (100, 80))).is_some());
        // A new live surface pushes us over budget.
        cache.insert(SurfaceOwner::Window(3), key(3, (100, 80)), surface(100, 80, 3));
        cache.end_frame();

        // Offscreen W1 evicted first; live W2 + W3 survive.
        assert!(!cache.contains(SurfaceOwner::Window(1)), "offscreen W1 evicted");
        assert!(cache.contains(SurfaceOwner::Window(2)), "live W2 protected");
        assert!(cache.contains(SurfaceOwner::Window(3)));
    }

    // ── pool reclaim / no-leak ───────────────────────────────────────────────

    #[test]
    fn evicted_buffer_returns_to_pool() {
        let one = (100 * 80 * 4) as usize;
        let mut cache = SurfaceCache::with_budget_bytes(2 * one);

        cache.begin_frame();
        cache.insert(SurfaceOwner::Window(1), key(1, (100, 80)), surface(100, 80, 1));
        cache.end_frame();
        cache.begin_frame();
        cache.insert(SurfaceOwner::Window(2), key(2, (100, 80)), surface(100, 80, 2));
        cache.end_frame();

        let pooled_before = cache.stats().pooled;
        let reuses_before = cache.pool_mut().reuses();

        cache.begin_frame();
        // Force eviction of W1 by inserting a third live surface over budget.
        cache.insert(SurfaceOwner::Window(3), key(3, (100, 80)), surface(100, 80, 3));
        cache.end_frame();
        assert!(!cache.contains(SurfaceOwner::Window(1)));
        assert_eq!(cache.stats().pooled, pooled_before + 1, "evicted W1 buffer pooled");

        // The pooled memory is reused by a same-size acquire (proves no leak +
        // real recycle, not just a drop).
        let fb = cache.pool_mut().acquire(100, 80, PixelFormat::Bgra8);
        assert_eq!(fb.pixels().len(), one);
        assert_eq!(cache.pool_mut().reuses(), reuses_before + 1, "pool recycled the buffer");
    }

    #[test]
    fn invalidate_drops_and_reclaims() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        cache.insert(SurfaceOwner::Window(1), key(1, (100, 80)), surface(100, 80, 1));
        assert_eq!(cache.total_bytes(), 100 * 80 * 4);

        cache.invalidate(SurfaceOwner::Window(1));
        assert!(!cache.contains(SurfaceOwner::Window(1)));
        assert_eq!(cache.total_bytes(), 0, "bytes returned to zero");
        assert_eq!(cache.stats().pooled, 1, "invalidated buffer pooled");
        assert_eq!(cache.stats().evictions, 1);
    }

    #[test]
    fn shared_buffer_is_not_pooled_but_not_leaked() {
        // If a blit clone is still in flight, reclaim must drop (no panic, no
        // double-free); it simply is not returned to the pool.
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        let s = surface(10, 10, 1);
        let _outstanding = s.clone(); // hold a second strong Arc reference
        cache.insert(SurfaceOwner::Window(1), key(1, (10, 10)), s);
        cache.invalidate(SurfaceOwner::Window(1));
        assert_eq!(cache.stats().pooled, 0, "shared buffer not pooled");
        assert!(!cache.contains(SurfaceOwner::Window(1)), "still removed from store");
    }

    // ── idle frame: every owner reused, nothing inserted ─────────────────────

    #[test]
    fn idle_frame_all_reuse_no_eviction() {
        let mut cache = SurfaceCache::with_budget_bytes(64 * 1024 * 1024);
        cache.begin_frame();
        let ka = key(1, (50, 50));
        let kb = key(2, (50, 50));
        cache.insert(SurfaceOwner::Window(1), ka, surface(50, 50, 1));
        cache.insert(SurfaceOwner::Window(2), kb, surface(50, 50, 2));
        cache.end_frame();

        cache.begin_frame();
        assert!(cache.get(SurfaceOwner::Window(1), ka).is_some());
        assert!(cache.get(SurfaceOwner::Window(2), kb).is_some());
        cache.end_frame();
        assert_eq!(cache.stats().evictions, 0, "idle reuse never evicts");
        assert_eq!(cache.len(), 2);
    }
}
