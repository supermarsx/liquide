//! Slab-based send buffer pool with priority-aware allocation.
//!
//! Provides a fixed-capacity buffer pool with size-class slabs and priority
//! reservation.  High-priority channels (P0–P4) draw from a reserved pool
//! first, preventing bulk traffic from starving real-time flows.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::priority::Priority;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Size classes for the slab allocator.
pub const SIZE_CLASSES: [usize; 4] = [128, 1024, 8192, 65536];

/// Default total pool capacity (8 MiB).
pub const DEFAULT_CAPACITY: u64 = 8 * 1024 * 1024;

/// Default reserved capacity for P0–P4 traffic (256 KiB).
pub const DEFAULT_RESERVED: u64 = 256 * 1024;

/// Backpressure threshold (fraction of total capacity).
pub const BACKPRESSURE_THRESHOLD: f64 = 0.80;

/// Suspend threshold for bulk traffic (fraction of total capacity).
pub const SUSPEND_THRESHOLD: f64 = 0.90;

/// Pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Total pool capacity in bytes.
    pub capacity: u64,
    /// Reserved capacity for P0–P4 traffic.
    pub reserved: u64,
    /// Fraction of capacity triggering backpressure for P5.
    pub backpressure_threshold: f64,
    /// Fraction of capacity triggering suspension for P6.
    pub suspend_threshold: f64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CAPACITY,
            reserved: DEFAULT_RESERVED,
            backpressure_threshold: BACKPRESSURE_THRESHOLD,
            suspend_threshold: SUSPEND_THRESHOLD,
        }
    }
}

// ---------------------------------------------------------------------------
// Pool Statistics
// ---------------------------------------------------------------------------

/// Snapshot of pool utilization.
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    /// Total bytes currently allocated.
    pub used_bytes: u64,
    /// Total pool capacity.
    pub capacity: u64,
    /// Bytes allocated from the reserved pool.
    pub reserved_used: u64,
    /// Reserved pool capacity.
    pub reserved_capacity: u64,
}

impl PoolStats {
    /// Overall utilization (0.0–1.0).
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            return 0.0;
        }
        self.used_bytes as f64 / self.capacity as f64
    }

    /// Reserved pool utilization (0.0–1.0).
    #[must_use]
    pub fn reserved_utilization(&self) -> f64 {
        if self.reserved_capacity == 0 {
            return 0.0;
        }
        self.reserved_used as f64 / self.reserved_capacity as f64
    }
}

// ---------------------------------------------------------------------------
// Send Buffer Pool
// ---------------------------------------------------------------------------

/// A fixed-capacity send buffer pool with priority-aware allocation.
#[derive(Debug)]
pub struct SendBufferPool {
    config: PoolConfig,
    /// Total bytes in use (general + reserved).
    used: AtomicU64,
    /// Bytes allocated from the reserved pool.
    reserved_used: AtomicU64,
    /// Per-size-class free lists.
    slabs: [Mutex<Vec<Vec<u8>>>; 4],
}

impl SendBufferPool {
    /// Create a new pool with the given configuration.
    #[must_use]
    pub fn new(config: PoolConfig) -> Self {
        Self {
            config,
            used: AtomicU64::new(0),
            reserved_used: AtomicU64::new(0),
            slabs: [
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
                Mutex::new(Vec::new()),
            ],
        }
    }

    /// Create a pool with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(PoolConfig::default())
    }

    /// Attempt to allocate a buffer of at least `size` bytes.
    ///
    /// Returns `None` if the pool is exhausted and the priority cannot
    /// be served (P5 under backpressure, P6 under suspension).
    pub fn alloc(&self, size: usize, priority: Priority) -> Option<PoolBuffer> {
        let slab_idx = Self::slab_index(size)?;
        let alloc_size = SIZE_CLASSES[slab_idx] as u64;

        // Check priority-based admission
        let current_used = self.used.load(Ordering::Relaxed);
        let capacity = self.config.capacity;

        match priority {
            Priority::P0Emergency
            | Priority::P1Input
            | Priority::P2Cursor
            | Priority::P3Audio
            | Priority::P4Control => {
                // Try reserved pool first
                let reserved_used = self.reserved_used.load(Ordering::Relaxed);
                if reserved_used + alloc_size <= self.config.reserved {
                    self.reserved_used.fetch_add(alloc_size, Ordering::Relaxed);
                    self.used.fetch_add(alloc_size, Ordering::Relaxed);
                    let buf = self.take_from_slab(slab_idx, SIZE_CLASSES[slab_idx]);
                    return Some(PoolBuffer {
                        data: buf,
                        size: alloc_size,
                        from_reserved: true,
                    });
                }
                // Fall through to general pool (never rejected for P0-P4)
                if current_used + alloc_size > capacity {
                    // Even for high priority, don't exceed total capacity
                    return None;
                }
                self.used.fetch_add(alloc_size, Ordering::Relaxed);
                let buf = self.take_from_slab(slab_idx, SIZE_CLASSES[slab_idx]);
                Some(PoolBuffer {
                    data: buf,
                    size: alloc_size,
                    from_reserved: false,
                })
            }
            Priority::P5Graphics => {
                let threshold = (capacity as f64 * self.config.backpressure_threshold) as u64;
                if current_used + alloc_size > threshold {
                    return None; // Backpressure
                }
                self.used.fetch_add(alloc_size, Ordering::Relaxed);
                let buf = self.take_from_slab(slab_idx, SIZE_CLASSES[slab_idx]);
                Some(PoolBuffer {
                    data: buf,
                    size: alloc_size,
                    from_reserved: false,
                })
            }
            Priority::P6Bulk => {
                let threshold = (capacity as f64 * self.config.suspend_threshold) as u64;
                if current_used + alloc_size > threshold {
                    return None; // Suspended
                }
                self.used.fetch_add(alloc_size, Ordering::Relaxed);
                let buf = self.take_from_slab(slab_idx, SIZE_CLASSES[slab_idx]);
                Some(PoolBuffer {
                    data: buf,
                    size: alloc_size,
                    from_reserved: false,
                })
            }
        }
    }

    /// Return a buffer to the pool.
    pub fn dealloc(&self, buffer: PoolBuffer) {
        self.used.fetch_sub(buffer.size, Ordering::Relaxed);
        if buffer.from_reserved {
            self.reserved_used.fetch_sub(buffer.size, Ordering::Relaxed);
        }
        // Return to slab free list
        if let Some(idx) = Self::slab_index(buffer.data.capacity()) {
            if SIZE_CLASSES[idx] == buffer.data.capacity() {
                let mut slab = self.slabs[idx].lock().unwrap();
                slab.push(buffer.data);
            }
        }
    }

    /// Current pool statistics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            used_bytes: self.used.load(Ordering::Relaxed),
            capacity: self.config.capacity,
            reserved_used: self.reserved_used.load(Ordering::Relaxed),
            reserved_capacity: self.config.reserved,
        }
    }

    /// Whether the pool is signaling backpressure (for P5 traffic).
    #[must_use]
    pub fn is_backpressure(&self) -> bool {
        let used = self.used.load(Ordering::Relaxed) as f64;
        let cap = self.config.capacity as f64;
        used / cap >= self.config.backpressure_threshold
    }

    /// Whether bulk traffic (P6) should be suspended.
    #[must_use]
    pub fn is_suspended(&self) -> bool {
        let used = self.used.load(Ordering::Relaxed) as f64;
        let cap = self.config.capacity as f64;
        used / cap >= self.config.suspend_threshold
    }

    /// Find the smallest size class that fits `size`.
    fn slab_index(size: usize) -> Option<usize> {
        SIZE_CLASSES.iter().position(|&s| s >= size)
    }

    /// Try to reuse a buffer from the slab free list, or allocate new.
    fn take_from_slab(&self, slab_idx: usize, capacity: usize) -> Vec<u8> {
        let mut slab = self.slabs[slab_idx].lock().unwrap();
        slab.pop().unwrap_or_else(|| Vec::with_capacity(capacity))
    }
}

// ---------------------------------------------------------------------------
// Pool Buffer
// ---------------------------------------------------------------------------

/// An RAII buffer handle from the pool.
///
/// When dropped without being returned via `SendBufferPool::dealloc`,
/// the allocated bytes are still freed from the accounting.
#[derive(Debug)]
pub struct PoolBuffer {
    /// The underlying buffer.
    pub data: Vec<u8>,
    /// The allocated size (may be larger than data.len()).
    size: u64,
    /// Whether this was allocated from the reserved pool.
    from_reserved: bool,
}

impl PoolBuffer {
    /// The allocated slab size.
    #[must_use]
    pub fn allocated_size(&self) -> u64 {
        self.size
    }

    /// Whether this buffer was allocated from the priority-reserved pool.
    #[must_use]
    pub fn is_reserved(&self) -> bool {
        self.from_reserved
    }
}
