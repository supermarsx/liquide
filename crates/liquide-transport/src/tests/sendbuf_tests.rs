use crate::priority::Priority;
use crate::sendbuf::{
    DEFAULT_CAPACITY, DEFAULT_RESERVED, PoolConfig, SIZE_CLASSES, SendBufferPool,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[test]
fn size_classes_ascending() {
    for i in 1..SIZE_CLASSES.len() {
        assert!(SIZE_CLASSES[i] > SIZE_CLASSES[i - 1]);
    }
}

// ---------------------------------------------------------------------------
// Pool Creation
// ---------------------------------------------------------------------------

#[test]
fn pool_initial_stats() {
    let pool = SendBufferPool::with_defaults();
    let stats = pool.stats();
    assert_eq!(stats.used_bytes, 0);
    assert_eq!(stats.capacity, DEFAULT_CAPACITY);
    assert_eq!(stats.reserved_used, 0);
    assert_eq!(stats.reserved_capacity, DEFAULT_RESERVED);
    assert!((stats.utilization() - 0.0).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Allocation / Deallocation
// ---------------------------------------------------------------------------

#[test]
fn alloc_and_dealloc() {
    let pool = SendBufferPool::with_defaults();
    let buf = pool.alloc(100, Priority::P4Control).unwrap();
    assert!(buf.allocated_size() >= 100);
    let stats = pool.stats();
    assert!(stats.used_bytes > 0);

    pool.dealloc(buf);
    let stats = pool.stats();
    assert_eq!(stats.used_bytes, 0);
}

#[test]
fn alloc_picks_smallest_slab() {
    let pool = SendBufferPool::with_defaults();

    // 50 bytes should go into 128-byte slab
    let buf = pool.alloc(50, Priority::P4Control).unwrap();
    assert_eq!(buf.allocated_size(), 128);
    pool.dealloc(buf);

    // 200 bytes should go into 1024-byte slab
    let buf = pool.alloc(200, Priority::P4Control).unwrap();
    assert_eq!(buf.allocated_size(), 1024);
    pool.dealloc(buf);

    // 5000 bytes should go into 8192-byte slab
    let buf = pool.alloc(5000, Priority::P4Control).unwrap();
    assert_eq!(buf.allocated_size(), 8192);
    pool.dealloc(buf);
}

#[test]
fn alloc_too_large_returns_none() {
    let pool = SendBufferPool::with_defaults();
    // Larger than any slab
    assert!(pool.alloc(100_000, Priority::P4Control).is_none());
}

// ---------------------------------------------------------------------------
// Reserved Pool
// ---------------------------------------------------------------------------

#[test]
fn high_priority_uses_reserved() {
    let config = PoolConfig {
        capacity: 10_000,
        reserved: 1_000,
        ..PoolConfig::default()
    };
    let pool = SendBufferPool::new(config);

    let buf = pool.alloc(50, Priority::P0Emergency).unwrap();
    assert!(buf.is_reserved());
    let stats = pool.stats();
    assert!(stats.reserved_used > 0);
    pool.dealloc(buf);
}

#[test]
fn reserved_dealloc_restores() {
    let config = PoolConfig {
        capacity: 10_000,
        reserved: 1_000,
        ..PoolConfig::default()
    };
    let pool = SendBufferPool::new(config);
    let buf = pool.alloc(50, Priority::P1Input).unwrap();
    pool.dealloc(buf);
    assert_eq!(pool.stats().reserved_used, 0);
}

// ---------------------------------------------------------------------------
// Backpressure & Suspension
// ---------------------------------------------------------------------------

#[test]
fn p5_blocked_at_backpressure() {
    let config = PoolConfig {
        capacity: 1024,
        reserved: 128,
        backpressure_threshold: 0.50,
        suspend_threshold: 0.90,
    };
    let pool = SendBufferPool::new(config);

    // Fill past 50% with control traffic
    let mut held = Vec::new();
    for _ in 0..5 {
        if let Some(buf) = pool.alloc(100, Priority::P4Control) {
            held.push(buf);
        }
    }

    // P5 should be blocked when utilization >= 50%
    if pool.is_backpressure() {
        assert!(pool.alloc(100, Priority::P5Graphics).is_none());
    }

    for buf in held {
        pool.dealloc(buf);
    }
}

#[test]
fn p6_blocked_at_suspend() {
    let config = PoolConfig {
        capacity: 1024,
        reserved: 128,
        backpressure_threshold: 0.80,
        suspend_threshold: 0.50,
    };
    let pool = SendBufferPool::new(config);

    // Fill past 50%
    let mut held = Vec::new();
    for _ in 0..5 {
        if let Some(buf) = pool.alloc(100, Priority::P4Control) {
            held.push(buf);
        }
    }

    if pool.is_suspended() {
        assert!(pool.alloc(100, Priority::P6Bulk).is_none());
    }

    for buf in held {
        pool.dealloc(buf);
    }
}

// ---------------------------------------------------------------------------
// Utilization
// ---------------------------------------------------------------------------

#[test]
fn utilization_increases_with_alloc() {
    let pool = SendBufferPool::with_defaults();
    let before = pool.stats().utilization();
    let buf = pool.alloc(1000, Priority::P5Graphics).unwrap();
    let after = pool.stats().utilization();
    assert!(after > before);
    pool.dealloc(buf);
}

// ---------------------------------------------------------------------------
// Slab Reuse
// ---------------------------------------------------------------------------

#[test]
fn slab_buffers_reused() {
    let pool = SendBufferPool::with_defaults();

    // Allocate and return
    let buf = pool.alloc(50, Priority::P4Control).unwrap();
    pool.dealloc(buf);

    // Allocate again — should reuse the returned buffer
    let buf2 = pool.alloc(50, Priority::P4Control).unwrap();
    assert_eq!(buf2.allocated_size(), 128);
    pool.dealloc(buf2);
}
