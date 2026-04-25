//! Lock-free MPSC (multi-producer, single-consumer) queue and coalescing slot.
//!
//! Designed for the desktop compositor's render message pipeline, where the
//! main thread and potentially multiple input-handling threads push messages
//! to be consumed by a single render thread (see [`super::render_thread::RenderMsg`]).
//!
//! # Design
//!
//! The queue uses an intrusive singly-linked list with atomic CAS (compare-and-swap)
//! on the head pointer for lock-free insertion, inspired by the NT DPC queue pattern
//! (InterlockedCompareExchangePointer for lock-free insertion).  The single consumer
//! atomically swaps the head to null and processes all accumulated nodes, reversing
//! the list to restore FIFO order.
//!
//! The [`CoalescingSlot`] provides "latest-wins" semantics for values where only the
//! most recent matters (e.g., cursor position updates).  It uses a single `AtomicPtr`
//! that producers overwrite and the consumer takes.
//!
//! # Memory Ordering
//!
//! - **Producers** use `AcqRel` on CAS (and `Acquire` on the CAS failure load) to
//!   ensure the node's `next` pointer is visible before it becomes reachable via `head`.
//! - **Consumer** uses `Acquire` on the swap to see all writes made by producers.
//! - **Wake flag** uses `Release` on set (producer) and `Acquire` on load (consumer)
//!   to establish a happens-before relationship with the enqueued data.
//!
//! # Safety
//!
//! All heap allocations are owned via `Box` and converted to/from raw pointers at
//! the atomic boundary.  Ownership is transferred exactly once: `push` allocates
//! and inserts, `drain` extracts and reconstitutes `Box`es.  `Drop` drains any
//! remaining items to prevent leaks.

use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Lock-free MPSC queue
// ---------------------------------------------------------------------------

/// A node in the lock-free linked list.
struct Node<T> {
    value: T,
    next: *mut Node<T>,
}

/// Lock-free multi-producer, single-consumer queue.
///
/// Producers call [`push`](Self::push) from any thread.  A single consumer
/// calls [`drain`](Self::drain) to atomically collect all enqueued items in
/// FIFO order.
///
/// The implementation is a Treiber-style lock-free stack (LIFO insertion)
/// with reversal on drain to restore FIFO ordering.  This gives O(1) lock-free
/// push and O(N) drain — ideal for batched consumption patterns like the
/// render thread's message loop.
pub struct LockFreeQueue<T> {
    /// Head of the intrusive linked list.  Producers CAS new nodes onto this.
    /// The consumer swaps it to null to drain all items at once.
    head: AtomicPtr<Node<T>>,

    /// Wake flag — set by producers after enqueue so the consumer can poll
    /// without OS-level synchronization (e.g., condvar or event).
    wake: AtomicBool,

    /// Total number of items successfully enqueued (monotonically increasing).
    enqueue_count: AtomicU64,

    /// Total number of items returned by `drain` (monotonically increasing).
    drain_count: AtomicU64,
}

// SAFETY: The queue is designed for multi-producer use.  All shared state
// is accessed through atomics.  `T` must be `Send` since values cross thread
// boundaries.
unsafe impl<T: Send> Send for LockFreeQueue<T> {}
unsafe impl<T: Send> Sync for LockFreeQueue<T> {}

impl<T> LockFreeQueue<T> {
    /// Create a new empty queue.
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
            wake: AtomicBool::new(false),
            enqueue_count: AtomicU64::new(0),
            drain_count: AtomicU64::new(0),
        }
    }

    /// Enqueue an item.  Lock-free, wait-free for a single CAS retry loop.
    ///
    /// This is safe to call from multiple threads simultaneously.  The item
    /// is heap-allocated into a [`Node`] and CAS-linked onto the head of the
    /// intrusive list.
    pub fn push(&self, item: T) {
        let node = Box::into_raw(Box::new(Node {
            value: item,
            next: ptr::null_mut(),
        }));

        loop {
            // SAFETY: `head` is always either null or a valid `Node` pointer
            // that was inserted by a prior `push`.
            let current_head = self.head.load(Ordering::Acquire);

            // Point our new node's `next` to the current head.
            //
            // SAFETY: `node` is a valid pointer we just allocated and have
            // exclusive access to (it is not yet reachable via `head`).
            unsafe {
                (*node).next = current_head;
            }

            // CAS: try to make our node the new head.
            //
            // `AcqRel` ensures:
            //   - Release: our write to `(*node).next` is visible to the
            //     consumer before the node becomes reachable via `head`.
            //   - Acquire: if the CAS fails, we see the latest `head` value
            //     written by a concurrent producer.
            match self.head.compare_exchange_weak(
                current_head,
                node,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Successfully linked.  Bump the enqueue counter and set
                    // the wake flag so the consumer knows there is work.
                    self.enqueue_count.fetch_add(1, Ordering::Relaxed);
                    self.wake.store(true, Ordering::Release);
                    return;
                }
                Err(_) => {
                    // Another producer won the race.  Retry with the updated
                    // head (loaded by the failed CAS via Acquire ordering).
                    continue;
                }
            }
        }
    }

    /// Atomically drain all enqueued items, returning them in FIFO order.
    ///
    /// This must be called from a single consumer thread.  It swaps the head
    /// pointer to null, walks the extracted list, and reverses it to restore
    /// insertion order.
    ///
    /// Returns an empty `Vec` if no items are pending.
    pub fn drain(&self) -> Vec<T> {
        // Atomically take the entire list.
        //
        // `Acquire` ensures we see all writes (node allocations, `next`
        // pointer stores) made by producers before their successful CAS.
        let head = self.head.swap(ptr::null_mut(), Ordering::Acquire);

        // Clear the wake flag now that we've taken everything.
        self.wake.store(false, Ordering::Release);

        if head.is_null() {
            return Vec::new();
        }

        // Walk the list and collect items.  The list is in LIFO order
        // (most recently pushed node is at `head`), so we collect into a
        // Vec and then reverse for FIFO.
        let mut items = Vec::new();
        let mut current = head;
        while !current.is_null() {
            // SAFETY: `current` was allocated by `push` via `Box::into_raw`.
            // We have exclusive ownership after the atomic swap (no other
            // thread can reach these nodes).  We reconstitute the `Box` to
            // reclaim the heap allocation.
            let node = unsafe { Box::from_raw(current) };
            current = node.next;
            items.push(node.value);
        }

        // Reverse to restore FIFO order (head was the most-recently-pushed).
        items.reverse();

        let count = items.len() as u64;
        self.drain_count.fetch_add(count, Ordering::Relaxed);

        items
    }

    /// Check if the queue appears empty.
    ///
    /// This is a relaxed check — by the time this returns, a concurrent
    /// producer may have already enqueued an item.  Useful for fast-path
    /// polling before committing to a drain.
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed).is_null()
    }

    /// Check the wake flag.
    ///
    /// Returns `true` if at least one `push` has occurred since the last
    /// `drain`.  This is cheaper than `is_empty` for the consumer's poll
    /// loop when the flag is enough to decide whether to wake.
    pub fn is_signaled(&self) -> bool {
        self.wake.load(Ordering::Acquire)
    }

    /// Return a snapshot of queue statistics.
    pub fn stats(&self) -> QueueStats {
        QueueStats {
            enqueue_count: self.enqueue_count.load(Ordering::Relaxed),
            drain_count: self.drain_count.load(Ordering::Relaxed),
            coalesce_overwrites: 0,
        }
    }
}

impl<T> Default for LockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for LockFreeQueue<T> {
    fn drop(&mut self) {
        // Drain any remaining items to prevent memory leaks.
        //
        // During drop we have exclusive access (`&mut self`), so no
        // synchronization is needed — but we go through `drain` anyway
        // since it handles the pointer walk correctly.
        let _ = self.drain();
    }
}

// ---------------------------------------------------------------------------
// Coalescing slot
// ---------------------------------------------------------------------------

/// A single-value slot with "latest wins" semantics.
///
/// Designed for values where only the most recent matters — for example,
/// cursor position updates during a frame.  Producers [`store`](Self::store)
/// a new value (overwriting any previous), and the consumer
/// [`take`](Self::take)s the current value atomically.
///
/// Internally this is a single `AtomicPtr` that holds a heap-allocated `T`
/// (or null if empty).  Store overwrites the previous value (dropping it).
/// Take swaps in null and returns the value.
pub struct CoalescingSlot<T> {
    /// The current value, or null if empty.
    ptr: AtomicPtr<T>,

    /// Number of times a `store` overwrote a previous value before the
    /// consumer could `take` it.
    overwrite_count: AtomicU64,
}

// SAFETY: Same reasoning as `LockFreeQueue` — all access is through atomics,
// and `T: Send` is required since values move between threads.
unsafe impl<T: Send> Send for CoalescingSlot<T> {}
unsafe impl<T: Send> Sync for CoalescingSlot<T> {}

impl<T> CoalescingSlot<T> {
    /// Create an empty coalescing slot.
    pub fn new() -> Self {
        Self {
            ptr: AtomicPtr::new(ptr::null_mut()),
            overwrite_count: AtomicU64::new(0),
        }
    }

    /// Store a value, overwriting any previous value that hasn't been taken.
    ///
    /// If a previous value was present, it is dropped and the overwrite
    /// counter is incremented.
    ///
    /// Lock-free: uses a single atomic swap.
    pub fn store(&self, value: T) {
        let new_ptr = Box::into_raw(Box::new(value));

        // Swap in the new value, getting back whatever was there before.
        //
        // `AcqRel` ensures:
        //   - Release: the write to `*new_ptr` is visible before the pointer
        //     becomes reachable via `self.ptr`.
        //   - Acquire: if there was a previous value, we see its full
        //     initialization before we drop it.
        let old = self.ptr.swap(new_ptr, Ordering::AcqRel);

        if !old.is_null() {
            // SAFETY: `old` was allocated by a previous `store` via
            // `Box::into_raw`.  We now have exclusive ownership after the
            // atomic swap — no other thread can access it.
            let _drop = unsafe { Box::from_raw(old) };
            self.overwrite_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Take the current value, leaving the slot empty.
    ///
    /// Returns `None` if no value has been stored since the last take.
    ///
    /// Lock-free: uses a single atomic swap.
    pub fn take(&self) -> Option<T> {
        // Swap the pointer to null, taking ownership of whatever was there.
        //
        // `Acquire` ensures we see the full initialization of `*ptr` that
        // was performed by the producer before their `Release` store.
        let ptr = self.ptr.swap(ptr::null_mut(), Ordering::Acquire);

        if ptr.is_null() {
            None
        } else {
            // SAFETY: `ptr` was allocated by `store` via `Box::into_raw`.
            // The atomic swap gives us exclusive ownership.
            Some(*unsafe { Box::from_raw(ptr) })
        }
    }

    /// Check if the slot currently holds a value.
    ///
    /// Relaxed check — may be stale by the time the caller acts on it.
    pub fn is_empty(&self) -> bool {
        self.ptr.load(Ordering::Relaxed).is_null()
    }

    /// Number of times a `store` overwrote a previous un-taken value.
    pub fn overwrite_count(&self) -> u64 {
        self.overwrite_count.load(Ordering::Relaxed)
    }
}

impl<T> Default for CoalescingSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for CoalescingSlot<T> {
    fn drop(&mut self) {
        // Drop any remaining value to prevent memory leaks.
        let ptr = *self.ptr.get_mut();
        if !ptr.is_null() {
            // SAFETY: We have `&mut self` so no concurrent access.
            // The pointer was allocated by `store`.
            let _drop = unsafe { Box::from_raw(ptr) };
        }
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Snapshot of queue and coalescing slot statistics.
///
/// All counters are monotonically increasing over the lifetime of the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueStats {
    /// Total number of items successfully enqueued via `push`.
    pub enqueue_count: u64,

    /// Total number of items returned by `drain`.
    pub drain_count: u64,

    /// Total number of coalescing overwrites (a `store` that replaced
    /// a value before `take` was called).
    pub coalesce_overwrites: u64,
}

impl QueueStats {
    /// Number of items currently in-flight (enqueued but not yet drained).
    ///
    /// This is an approximation — the counters are read non-atomically
    /// relative to each other.
    pub fn pending(&self) -> u64 {
        self.enqueue_count.saturating_sub(self.drain_count)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn single_thread_push_drain() {
        let q = LockFreeQueue::new();
        assert!(q.is_empty());

        q.push(1);
        q.push(2);
        q.push(3);
        assert!(!q.is_empty());

        let items = q.drain();
        assert_eq!(items, vec![1, 2, 3]);
        assert!(q.is_empty());

        let stats = q.stats();
        assert_eq!(stats.enqueue_count, 3);
        assert_eq!(stats.drain_count, 3);
    }

    #[test]
    fn drain_empty_returns_empty_vec() {
        let q = LockFreeQueue::<i32>::new();
        assert_eq!(q.drain(), Vec::<i32>::new());
    }

    #[test]
    fn multiple_drains() {
        let q = LockFreeQueue::new();

        q.push(10);
        assert_eq!(q.drain(), vec![10]);

        q.push(20);
        q.push(30);
        assert_eq!(q.drain(), vec![20, 30]);

        assert_eq!(q.drain(), Vec::<i32>::new());

        let stats = q.stats();
        assert_eq!(stats.enqueue_count, 3);
        assert_eq!(stats.drain_count, 3);
    }

    #[test]
    fn fifo_order_preserved() {
        let q = LockFreeQueue::new();
        for i in 0..100 {
            q.push(i);
        }
        let items = q.drain();
        let expected: Vec<i32> = (0..100).collect();
        assert_eq!(items, expected);
    }

    #[test]
    fn wake_flag() {
        let q = LockFreeQueue::new();
        assert!(!q.is_signaled());

        q.push(42);
        assert!(q.is_signaled());

        let _ = q.drain();
        assert!(!q.is_signaled());
    }

    #[test]
    fn concurrent_producers_single_consumer() {
        let q = Arc::new(LockFreeQueue::new());
        let num_producers = 8;
        let items_per_producer = 1_000;

        let mut handles = Vec::new();
        for producer_id in 0..num_producers {
            let q = Arc::clone(&q);
            handles.push(thread::spawn(move || {
                for i in 0..items_per_producer {
                    q.push(producer_id * items_per_producer + i);
                }
            }));
        }

        // Wait for all producers to finish.
        for h in handles {
            h.join().unwrap();
        }

        // Drain and verify we got everything.
        let items = q.drain();
        assert_eq!(items.len(), num_producers * items_per_producer);

        // Every value should appear exactly once.
        let mut sorted = items.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), num_producers * items_per_producer);

        let stats = q.stats();
        assert_eq!(
            stats.enqueue_count,
            (num_producers * items_per_producer) as u64
        );
        assert_eq!(
            stats.drain_count,
            (num_producers * items_per_producer) as u64
        );
    }

    #[test]
    fn concurrent_push_and_drain() {
        let q = Arc::new(LockFreeQueue::new());
        let total_items = 10_000;

        // Producer thread pushes items as fast as possible.
        let q_producer = Arc::clone(&q);
        let producer = thread::spawn(move || {
            for i in 0..total_items {
                q_producer.push(i);
            }
        });

        // Consumer thread drains in a loop, collecting all items.
        let q_consumer = Arc::clone(&q);
        let consumer = thread::spawn(move || {
            let mut collected = Vec::new();
            loop {
                let batch = q_consumer.drain();
                collected.extend(batch);
                if collected.len() >= total_items {
                    break;
                }
                // Yield to let producer make progress.
                thread::yield_now();
            }
            collected
        });

        producer.join().unwrap();
        let collected = consumer.join().unwrap();

        assert_eq!(collected.len(), total_items);

        // Values should all be present (though order between drain batches
        // might interleave — within each batch, FIFO is guaranteed).
        let mut sorted = collected;
        sorted.sort();
        let expected: Vec<usize> = (0..total_items).collect();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn drop_cleans_up_remaining_items() {
        // Use a counter to verify destructors run.
        use std::sync::atomic::AtomicUsize;

        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Tracked(#[allow(dead_code)] i32);
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROP_COUNT.store(0, Ordering::Relaxed);

        {
            let q = LockFreeQueue::new();
            q.push(Tracked(1));
            q.push(Tracked(2));
            q.push(Tracked(3));
            // Dropped here without draining.
        }

        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn coalescing_slot_basic() {
        let slot = CoalescingSlot::new();
        assert!(slot.is_empty());
        assert_eq!(slot.take(), None);

        slot.store(42);
        assert!(!slot.is_empty());
        assert_eq!(slot.take(), Some(42));
        assert!(slot.is_empty());
        assert_eq!(slot.take(), None);
    }

    #[test]
    fn coalescing_slot_overwrite() {
        let slot = CoalescingSlot::new();

        slot.store(1);
        slot.store(2);
        slot.store(3);

        // Only the latest value survives.
        assert_eq!(slot.take(), Some(3));
        assert_eq!(slot.overwrite_count(), 2);
    }

    #[test]
    fn coalescing_slot_concurrent() {
        let slot = Arc::new(CoalescingSlot::new());
        let num_writers = 4;
        let writes_per_thread = 1_000;

        let mut handles = Vec::new();
        for thread_id in 0..num_writers {
            let slot = Arc::clone(&slot);
            handles.push(thread::spawn(move || {
                for i in 0..writes_per_thread {
                    slot.store(thread_id * writes_per_thread + i);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // We should get *some* value (the last one written, though which
        // thread "wins" is nondeterministic).
        let val = slot.take();
        assert!(val.is_some());

        // Overwrite count should be total_writes - 1 at most
        // (first store doesn't overwrite), but at least some overwrites
        // happened with 4 concurrent writers doing 1000 each.
        let overwrites = slot.overwrite_count();
        assert!(overwrites > 0, "expected some overwrites, got 0");
    }

    #[test]
    fn coalescing_slot_drop_cleans_up() {
        use std::sync::atomic::AtomicUsize;

        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Tracked(#[allow(dead_code)] i32);
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROP_COUNT.store(0, Ordering::Relaxed);

        {
            let slot = CoalescingSlot::new();
            slot.store(Tracked(1));
            // Dropped without taking.
        }

        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn coalescing_slot_store_drops_previous() {
        use std::sync::atomic::AtomicUsize;

        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Tracked(#[allow(dead_code)] i32);
        impl Drop for Tracked {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROP_COUNT.store(0, Ordering::Relaxed);

        let slot = CoalescingSlot::new();
        slot.store(Tracked(1));
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0); // Still held
        slot.store(Tracked(2));
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 1); // First was dropped
        slot.store(Tracked(3));
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 2); // Second was dropped

        let _val = slot.take();
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 2); // Third now owned by _val
        drop(_val);
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 3); // Third dropped
    }

    #[test]
    fn queue_stats_pending() {
        let q = LockFreeQueue::new();
        q.push(1);
        q.push(2);

        let stats = q.stats();
        assert_eq!(stats.pending(), 2);

        let _ = q.drain();
        let stats = q.stats();
        assert_eq!(stats.pending(), 0);
    }

    #[test]
    fn queue_with_zero_sized_type() {
        let q = LockFreeQueue::new();
        q.push(());
        q.push(());
        let items = q.drain();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn queue_with_string_values() {
        let q = LockFreeQueue::new();
        q.push(String::from("hello"));
        q.push(String::from("world"));
        let items = q.drain();
        assert_eq!(items, vec!["hello", "world"]);
    }

    #[test]
    fn coalescing_slot_with_struct() {
        #[derive(Debug, PartialEq)]
        struct CursorPos {
            x: f32,
            y: f32,
        }

        let slot = CoalescingSlot::new();
        slot.store(CursorPos { x: 1.0, y: 2.0 });
        slot.store(CursorPos { x: 3.0, y: 4.0 });

        assert_eq!(slot.take(), Some(CursorPos { x: 3.0, y: 4.0 }));
        assert_eq!(slot.overwrite_count(), 1);
    }

    #[test]
    fn high_contention_stress_test() {
        let q = Arc::new(LockFreeQueue::new());
        let num_threads = 16;
        let items_per_thread = 500;

        let mut handles = Vec::new();
        for tid in 0..num_threads {
            let q = Arc::clone(&q);
            handles.push(thread::spawn(move || {
                for i in 0..items_per_thread {
                    q.push((tid, i));
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let items = q.drain();
        assert_eq!(items.len(), num_threads * items_per_thread);

        // Verify per-thread sequences are in order (FIFO within each
        // producer's contributions, though interleaved across producers).
        let mut per_thread: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for (tid, seq) in &items {
            per_thread.entry(*tid).or_default().push(*seq);
        }

        for (tid, seq) in &per_thread {
            // Each thread's items should appear in order (not necessarily
            // contiguous, but monotonically increasing).
            for window in seq.windows(2) {
                assert!(
                    window[0] < window[1],
                    "thread {tid}: expected {}<{}, FIFO violated",
                    window[0],
                    window[1]
                );
            }
        }
    }
}
