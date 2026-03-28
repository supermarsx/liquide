//! SurfacePool — reusable pixel buffer pool to reduce allocation pressure.
//!
//! Layers are frequently created and destroyed (e.g., popup menus, tooltips).
//! Rather than allocating and freeing pixel buffers every time, the pool
//! maintains buckets of pre-sized buffers that can be reused.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique handle identifying a surface allocation from the pool.
#[derive(Debug, Clone)]
pub struct SurfaceHandle {
    /// Unique ID of this allocation.
    pub id: u64,
    /// Pixel data (RGBA, 4 bytes per pixel).
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per row (width * 4 for RGBA).
    pub stride: u32,
}

impl SurfaceHandle {
    /// Total number of bytes in the pixel data.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }
}

/// Statistics about pool usage.
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Number of buffers currently allocated (in use by callers).
    pub allocated: u64,
    /// Number of buffers waiting in the pool for reuse.
    pub pooled: u64,
    /// Number of allocations that were satisfied by reusing a pooled buffer.
    pub reused: u64,
    /// Number of allocations that required a fresh Vec allocation.
    pub fresh: u64,
    /// Total bytes across all pooled (free) buffers.
    pub pooled_bytes: usize,
    /// Total bytes across all allocated (in-use) buffers.
    pub allocated_bytes: usize,
}

impl PoolStats {
    /// Total bytes managed by the pool (in-use + free).
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.pooled_bytes + self.allocated_bytes
    }
}

/// Size buckets for the pool. Dimensions are rounded up to the nearest
/// bucket to increase reuse. The bucket sizes are chosen to be powers of
/// two or common UI element sizes.
const BUCKET_SIZES: &[u32] = &[64, 128, 256, 512, 1024, 2048, 4096];

/// Round a dimension up to the nearest bucket size.
fn round_to_bucket(dim: u32) -> u32 {
    for &bucket in BUCKET_SIZES {
        if dim <= bucket {
            return bucket;
        }
    }
    // For sizes larger than the biggest bucket, round up to the next
    // multiple of the largest bucket.
    let largest = *BUCKET_SIZES.last().unwrap();
    ((dim + largest - 1) / largest) * largest
}

/// Global counter for surface handle IDs.
static NEXT_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

/// A pool of reusable pixel buffers, bucketed by size.
///
/// Each bucket key is `(bucket_width, bucket_height)`. When a buffer is
/// released, it goes back into the bucket matching its dimensions so a
/// future allocation of similar size can reuse it without `Vec::alloc`.
#[derive(Debug)]
pub struct SurfacePool {
    /// Free buffers organized by `(bucket_w, bucket_h)`.
    buckets: HashMap<(u32, u32), Vec<Vec<u8>>>,
    /// Running count of allocations served from the pool.
    reuse_count: u64,
    /// Running count of fresh allocations.
    fresh_count: u64,
    /// Number of buffers currently checked out.
    outstanding: u64,
    /// Maximum number of buffers to keep in any single bucket.
    max_per_bucket: usize,
}

impl SurfacePool {
    /// Create a new, empty pool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
            reuse_count: 0,
            fresh_count: 0,
            outstanding: 0,
            max_per_bucket: 8,
        }
    }

    /// Create a pool with a custom per-bucket retention limit.
    #[must_use]
    pub fn with_max_per_bucket(max: usize) -> Self {
        Self {
            max_per_bucket: max,
            ..Self::new()
        }
    }

    /// Allocate a surface of at least the given dimensions.
    ///
    /// The returned `SurfaceHandle` may be slightly larger than requested
    /// (rounded to the nearest bucket) but `width`/`height` fields reflect
    /// the requested size.
    pub fn allocate(&mut self, width: u32, height: u32) -> SurfaceHandle {
        let bucket_w = round_to_bucket(width);
        let bucket_h = round_to_bucket(height);
        let bucket_key = (bucket_w, bucket_h);
        let needed_bytes = (bucket_w as usize) * (bucket_h as usize) * 4;

        let data = if let Some(bucket) = self.buckets.get_mut(&bucket_key) {
            if let Some(mut buf) = bucket.pop() {
                // Reuse: zero the buffer to avoid stale pixel data.
                for byte in buf.iter_mut() {
                    *byte = 0;
                }
                self.reuse_count += 1;
                buf
            } else {
                self.fresh_count += 1;
                vec![0u8; needed_bytes]
            }
        } else {
            self.fresh_count += 1;
            vec![0u8; needed_bytes]
        };

        self.outstanding += 1;

        SurfaceHandle {
            id: NEXT_SURFACE_ID.fetch_add(1, Ordering::Relaxed),
            data,
            width,
            height,
            stride: width * 4,
        }
    }

    /// Return a surface to the pool for future reuse.
    ///
    /// The buffer is kept in the matching size bucket. If the bucket is
    /// already at the retention limit, the buffer is dropped.
    pub fn release(&mut self, handle: SurfaceHandle) {
        self.outstanding = self.outstanding.saturating_sub(1);

        let bucket_w = round_to_bucket(handle.width);
        let bucket_h = round_to_bucket(handle.height);
        let bucket_key = (bucket_w, bucket_h);

        let bucket = self.buckets.entry(bucket_key).or_insert_with(Vec::new);
        if bucket.len() < self.max_per_bucket {
            bucket.push(handle.data);
        }
        // else: drop the buffer — the bucket is full
    }

    /// Get current pool statistics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let pooled: u64 = self.buckets.values().map(|b| b.len() as u64).sum();
        let pooled_bytes: usize = self.buckets.values().flat_map(|b| b.iter()).map(|v| v.len()).sum();

        PoolStats {
            allocated: self.outstanding,
            pooled,
            reused: self.reuse_count,
            fresh: self.fresh_count,
            pooled_bytes,
            // We don't track in-use bytes exactly; approximate from outstanding count
            // using average allocation size.
            allocated_bytes: 0,
        }
    }

    /// Remove all pooled buffers, freeing their memory. Does not affect
    /// buffers that are currently checked out.
    pub fn clear(&mut self) {
        self.buckets.clear();
    }

    /// Remove pooled buffers from a specific size bucket.
    pub fn clear_bucket(&mut self, width: u32, height: u32) {
        let bucket_w = round_to_bucket(width);
        let bucket_h = round_to_bucket(height);
        self.buckets.remove(&(bucket_w, bucket_h));
    }

    /// Total number of buffers currently in the pool (free, waiting for reuse).
    #[must_use]
    pub fn pooled_count(&self) -> usize {
        self.buckets.values().map(|b| b.len()).sum()
    }
}

impl Default for SurfacePool {
    fn default() -> Self {
        Self::new()
    }
}
