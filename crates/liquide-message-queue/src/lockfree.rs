//! Lock-free data structures for high-performance message passing.
//!
//! Inspired by NT kernel patterns:
//! - **LockFreeQueue**: Treiber stack with FIFO drain (NT's DPC queue uses CAS insertion)
//! - **CasSlot**: Latest-value slot for coalescing (NT's input coalescing)
//! - **SlabAllocator**: Pre-allocated pool (NT's PAGED_LOOKASIDE_LIST for LPC messages)

use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

// ─── Lock-Free MPSC Queue ───────────────────────────────────────────────

struct LfNode<T> {
    value: Option<T>,
    next: AtomicPtr<LfNode<T>>,
}

/// Lock-free multi-producer single-consumer queue.
///
/// Producers push onto a Treiber stack via CAS on `head`.
/// Consumer atomically swaps `head` to null, then reverses the list for FIFO.
/// No mutex, no spinlock. O(1) enqueue, O(N) batch dequeue.
pub struct LockFreeQueue<T> {
    head: AtomicPtr<LfNode<T>>,
    len: AtomicU64,
}

unsafe impl<T: Send> Send for LockFreeQueue<T> {}
unsafe impl<T: Send> Sync for LockFreeQueue<T> {}

impl<T> LockFreeQueue<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
            len: AtomicU64::new(0),
        }
    }

    /// Lock-free enqueue. Multiple threads can call concurrently.
    pub fn push(&self, value: T) {
        let node = Box::into_raw(Box::new(LfNode {
            value: Some(value),
            next: AtomicPtr::new(ptr::null_mut()),
        }));
        loop {
            let old_head = self.head.load(Ordering::Acquire);
            unsafe { (*node).next.store(old_head, Ordering::Relaxed) };
            if self
                .head
                .compare_exchange_weak(old_head, node, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.len.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }

    /// Drain all pending items in FIFO order. Only one thread should call this.
    /// Atomically swaps head to null, reverses the collected stack.
    pub fn drain(&self) -> Vec<T> {
        let head = self.head.swap(ptr::null_mut(), Ordering::AcqRel);
        if head.is_null() {
            return Vec::new();
        }

        // Collect nodes from the stack (LIFO order)
        let mut nodes = Vec::new();
        let mut current = head;
        while !current.is_null() {
            let node = unsafe { Box::from_raw(current) };
            current = node.next.load(Ordering::Relaxed);
            if let Some(val) = node.value {
                nodes.push(val);
            }
        }

        let count = nodes.len() as u64;
        self.len.fetch_sub(count, Ordering::Relaxed);

        // Reverse for FIFO order
        nodes.reverse();
        nodes
    }

    /// Drain into an existing Vec (avoids allocation if caller reuses buffer).
    pub fn drain_into(&self, out: &mut Vec<T>) {
        let head = self.head.swap(ptr::null_mut(), Ordering::AcqRel);
        if head.is_null() {
            return;
        }

        let start_len = out.len();
        let mut current = head;
        while !current.is_null() {
            let node = unsafe { Box::from_raw(current) };
            current = node.next.load(Ordering::Relaxed);
            if let Some(val) = node.value {
                out.push(val);
            }
        }

        let added = (out.len() - start_len) as u64;
        self.len.fetch_sub(added, Ordering::Relaxed);

        // Reverse the newly added portion for FIFO
        out[start_len..].reverse();
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire).is_null()
    }

    pub fn len(&self) -> u64 {
        self.len.load(Ordering::Relaxed)
    }
}

impl<T> Drop for LockFreeQueue<T> {
    fn drop(&mut self) {
        // Drain remaining items to free memory
        let _ = self.drain();
    }
}

impl<T> Default for LockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─── CAS Slot (Latest-Value) ────────────────────────────────────────────

/// Atomic slot holding the latest value. Writers overwrite; reader takes.
/// Perfect for mouse position, cursor shape — only the most recent matters.
///
/// Inspired by NT's raw input thread coalescing: multiple WM_MOUSEMOVE
/// messages are collapsed into one with the latest coordinates.
pub struct CasSlot<T> {
    slot: AtomicPtr<T>,
}

unsafe impl<T: Send> Send for CasSlot<T> {}
unsafe impl<T: Send> Sync for CasSlot<T> {}

impl<T> CasSlot<T> {
    pub fn new() -> Self {
        Self {
            slot: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Store a new value, replacing any previous one.
    /// The old value (if any) is dropped.
    pub fn store(&self, value: T) {
        let new_ptr = Box::into_raw(Box::new(value));
        let old = self.slot.swap(new_ptr, Ordering::AcqRel);
        if !old.is_null() {
            unsafe { drop(Box::from_raw(old)) };
        }
    }

    /// Take the current value, leaving the slot empty.
    /// Returns None if the slot is empty.
    pub fn take(&self) -> Option<T> {
        let ptr = self.slot.swap(ptr::null_mut(), Ordering::AcqRel);
        if ptr.is_null() {
            None
        } else {
            Some(*unsafe { Box::from_raw(ptr) })
        }
    }

    /// Check if a value is present (approximate, may race).
    pub fn has_value(&self) -> bool {
        !self.slot.load(Ordering::Acquire).is_null()
    }
}

impl<T> Drop for CasSlot<T> {
    fn drop(&mut self) {
        let ptr = self.slot.swap(ptr::null_mut(), Ordering::AcqRel);
        if !ptr.is_null() {
            unsafe { drop(Box::from_raw(ptr)) };
        }
    }
}

impl<T> Default for CasSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Slab Allocator ─────────────────────────────────────────────────────

/// Pre-allocated pool of fixed-size objects.
/// Inspired by NT's PAGED_LOOKASIDE_LIST for LPC messages.
///
/// Acquires from free list first (zero alloc), falls back to Box::new.
/// Released objects return to the free list for reuse.
pub struct SlabAllocator<T> {
    free_list: Vec<Box<T>>,
    capacity: usize,
    reuse_count: u64,
    fallback_count: u64,
    release_count: u64,
}

/// Handle to a slab-allocated object. Returns to pool on drop if pool provided.
#[allow(dead_code)]
pub struct SlabHandle<T> {
    value: Option<Box<T>>,
}

impl<T> SlabAllocator<T> {
    /// Create a slab with the given capacity. Does NOT pre-allocate.
    pub fn new(capacity: usize) -> Self {
        Self {
            free_list: Vec::with_capacity(capacity),
            capacity,
            reuse_count: 0,
            fallback_count: 0,
            release_count: 0,
        }
    }

    /// Pre-populate the slab with objects created by the factory.
    pub fn prefill(&mut self, factory: impl Fn() -> T) {
        while self.free_list.len() < self.capacity {
            self.free_list.push(Box::new(factory()));
        }
    }

    /// Acquire an object from the slab, or create one via factory.
    pub fn acquire(&mut self, factory: impl FnOnce() -> T) -> Box<T> {
        if let Some(obj) = self.free_list.pop() {
            self.reuse_count += 1;
            obj
        } else {
            self.fallback_count += 1;
            Box::new(factory())
        }
    }

    /// Return an object to the slab for reuse.
    /// If the slab is full, the object is dropped.
    pub fn release(&mut self, obj: Box<T>) {
        self.release_count += 1;
        if self.free_list.len() < self.capacity {
            self.free_list.push(obj);
        }
        // else: drop obj (slab full)
    }

    /// Number of objects currently available in the pool.
    pub fn available(&self) -> usize {
        self.free_list.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Stats: how many acquires hit the slab vs fell back to allocation.
    pub fn stats(&self) -> SlabStats {
        let total = self.reuse_count + self.fallback_count;
        SlabStats {
            reuse_count: self.reuse_count,
            fallback_count: self.fallback_count,
            release_count: self.release_count,
            available: self.free_list.len(),
            capacity: self.capacity,
            hit_ratio: if total > 0 {
                self.reuse_count as f64 / total as f64
            } else {
                1.0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlabStats {
    pub reuse_count: u64,
    pub fallback_count: u64,
    pub release_count: u64,
    pub available: usize,
    pub capacity: usize,
    pub hit_ratio: f64,
}

// ─── Dedup Guard ────────────────────────────────────────────────────────

/// Duplicate-prevention guard inspired by NT's DPC Lock field.
/// The Lock field doubles as an "is-queued" flag — if non-null, the DPC
/// is already in a queue and re-insertion is skipped.
///
/// Use this to prevent the same work item from being queued twice.
pub struct DedupGuard {
    queued: AtomicBool,
}

impl DedupGuard {
    pub fn new() -> Self {
        Self {
            queued: AtomicBool::new(false),
        }
    }

    /// Try to mark as queued. Returns true if successfully marked
    /// (was not queued), false if already queued.
    /// Uses CAS like NT's InterlockedCompareExchangePointer.
    pub fn try_enqueue(&self) -> bool {
        self.queued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Mark as dequeued (after processing).
    pub fn mark_dequeued(&self) {
        self.queued.store(false, Ordering::Release);
    }

    /// Check if currently queued.
    pub fn is_queued(&self) -> bool {
        self.queued.load(Ordering::Acquire)
    }
}

impl Default for DedupGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── LockFreeQueue tests ──

    #[test]
    fn queue_new_is_empty() {
        let q: LockFreeQueue<i32> = LockFreeQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn queue_push_makes_non_empty() {
        let q = LockFreeQueue::new();
        q.push(42);
        assert!(!q.is_empty());
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn queue_drain_returns_fifo_order() {
        let q = LockFreeQueue::new();
        q.push(1);
        q.push(2);
        q.push(3);
        let items = q.drain();
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[test]
    fn queue_drain_empties() {
        let q = LockFreeQueue::new();
        q.push(10);
        q.push(20);
        let _ = q.drain();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn queue_drain_empty_returns_empty() {
        let q: LockFreeQueue<i32> = LockFreeQueue::new();
        let items = q.drain();
        assert!(items.is_empty());
    }

    #[test]
    fn queue_multiple_drain_cycles() {
        let q = LockFreeQueue::new();
        q.push(1);
        q.push(2);
        assert_eq!(q.drain(), vec![1, 2]);

        q.push(3);
        q.push(4);
        q.push(5);
        assert_eq!(q.drain(), vec![3, 4, 5]);
    }

    #[test]
    fn queue_drain_into_appends() {
        let q = LockFreeQueue::new();
        q.push(1);
        q.push(2);
        let mut buf = vec![0];
        q.drain_into(&mut buf);
        assert_eq!(buf, vec![0, 1, 2]);
        assert!(q.is_empty());
    }

    #[test]
    fn queue_large_batch() {
        let q = LockFreeQueue::new();
        for i in 0..1000 {
            q.push(i);
        }
        assert_eq!(q.len(), 1000);
        let items = q.drain();
        assert_eq!(items.len(), 1000);
        for (i, &v) in items.iter().enumerate() {
            assert_eq!(v, i as i32);
        }
    }

    #[test]
    fn queue_drop_cleans_up() {
        let q = LockFreeQueue::new();
        q.push(String::from("hello"));
        q.push(String::from("world"));
        drop(q); // Should not leak
    }

    // ── CasSlot tests ──

    #[test]
    fn slot_new_is_empty() {
        let s: CasSlot<i32> = CasSlot::new();
        assert!(!s.has_value());
        assert!(s.take().is_none());
    }

    #[test]
    fn slot_store_then_take() {
        let s = CasSlot::new();
        s.store(42);
        assert!(s.has_value());
        assert_eq!(s.take(), Some(42));
        assert!(!s.has_value());
    }

    #[test]
    fn slot_overwrite_keeps_latest() {
        let s = CasSlot::new();
        s.store(1);
        s.store(2);
        s.store(3);
        assert_eq!(s.take(), Some(3));
    }

    #[test]
    fn slot_take_twice_returns_none() {
        let s = CasSlot::new();
        s.store(99);
        assert_eq!(s.take(), Some(99));
        assert_eq!(s.take(), None);
    }

    #[test]
    fn slot_drop_cleans_up() {
        let s = CasSlot::new();
        s.store(String::from("test"));
        drop(s);
    }

    // ── SlabAllocator tests ──

    #[test]
    fn slab_acquire_from_empty_uses_factory() {
        let mut slab: SlabAllocator<Vec<u8>> = SlabAllocator::new(4);
        let obj = slab.acquire(|| vec![0u8; 64]);
        assert_eq!(obj.len(), 64);
        assert_eq!(slab.stats().fallback_count, 1);
        assert_eq!(slab.stats().reuse_count, 0);
    }

    #[test]
    fn slab_release_and_reuse() {
        let mut slab: SlabAllocator<Vec<u8>> = SlabAllocator::new(4);
        let obj = slab.acquire(|| vec![0u8; 64]);
        slab.release(obj);
        assert_eq!(slab.available(), 1);

        let obj2 = slab.acquire(|| vec![0u8; 64]);
        assert_eq!(slab.stats().reuse_count, 1);
        assert_eq!(obj2.len(), 64);
    }

    #[test]
    fn slab_prefill() {
        let mut slab: SlabAllocator<i32> = SlabAllocator::new(8);
        slab.prefill(|| 0);
        assert_eq!(slab.available(), 8);

        let obj = slab.acquire(|| 99);
        assert_eq!(*obj, 0); // Got prefilled value, not factory
        assert_eq!(slab.stats().reuse_count, 1);
        assert_eq!(slab.stats().fallback_count, 0);
    }

    #[test]
    fn slab_full_drops_excess() {
        let mut slab: SlabAllocator<i32> = SlabAllocator::new(2);
        slab.release(Box::new(1));
        slab.release(Box::new(2));
        slab.release(Box::new(3)); // Should be dropped, slab full
        assert_eq!(slab.available(), 2);
    }

    #[test]
    fn slab_hit_ratio() {
        let mut slab: SlabAllocator<i32> = SlabAllocator::new(4);
        slab.prefill(|| 0);
        let _ = slab.acquire(|| 0); // reuse
        let _ = slab.acquire(|| 0); // reuse
        let _ = slab.acquire(|| 0); // reuse
        let _ = slab.acquire(|| 0); // reuse
        let _ = slab.acquire(|| 0); // fallback
        let stats = slab.stats();
        assert_eq!(stats.reuse_count, 4);
        assert_eq!(stats.fallback_count, 1);
        assert!((stats.hit_ratio - 0.8).abs() < 0.01);
    }

    // ── DedupGuard tests ──

    #[test]
    fn dedup_initially_not_queued() {
        let g = DedupGuard::new();
        assert!(!g.is_queued());
    }

    #[test]
    fn dedup_try_enqueue_succeeds_once() {
        let g = DedupGuard::new();
        assert!(g.try_enqueue());
        assert!(!g.try_enqueue()); // Already queued
    }

    #[test]
    fn dedup_mark_dequeued_allows_re_enqueue() {
        let g = DedupGuard::new();
        assert!(g.try_enqueue());
        g.mark_dequeued();
        assert!(g.try_enqueue()); // Can enqueue again
    }

    #[test]
    fn dedup_is_queued_reflects_state() {
        let g = DedupGuard::new();
        assert!(!g.is_queued());
        g.try_enqueue();
        assert!(g.is_queued());
        g.mark_dequeued();
        assert!(!g.is_queued());
    }
}
