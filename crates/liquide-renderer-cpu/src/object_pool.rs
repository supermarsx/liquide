//! Object pooling for UI elements and rendering resources.
//!
//! Reduces allocation overhead by reusing objects instead of
//! repeatedly allocating and deallocating them.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A generic object pool with configurable capacity.
#[allow(dead_code)]
pub struct ObjectPool<T> {
    /// Pool of available objects ready for reuse.
    pool: VecDeque<T>,
    /// Maximum pool size (prevents unbounded growth).
    max_capacity: usize,
    /// Number of objects currently in use (checked out).
    in_use: usize,
}

impl<T> ObjectPool<T> {
    /// Create a new object pool with specified maximum capacity.
    #[must_use]
    pub fn new(max_capacity: usize) -> Self {
        Self {
            pool: VecDeque::with_capacity(max_capacity.min(64)),
            max_capacity,
            in_use: 0,
        }
    }

    /// Create a pool with default capacity (256 objects).
    #[must_use]
    pub fn default() -> Self {
        Self::new(256)
    }

    /// Acquire an object from the pool, or None if pool is empty.
    /// Caller is responsible for returning the object via `release()`.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(obj) = self.pool.pop_front() {
            self.in_use += 1;
            Some(obj)
        } else {
            None
        }
    }

    /// Acquire an object from the pool, or create a new one using the factory.
    pub fn acquire_or_create<F>(&mut self, factory: F) -> T
    where
        F: FnOnce() -> T,
    {
        if let Some(obj) = self.acquire() {
            obj
        } else {
            self.in_use += 1;
            factory()
        }
    }

    /// Release an object back to the pool for reuse.
    /// If the pool is at capacity, the object is dropped.
    pub fn release(&mut self, obj: T) {
        if self.in_use > 0 {
            self.in_use -= 1;
        }

        if self.pool.len() < self.max_capacity {
            self.pool.push_back(obj);
        }
        // Otherwise drop the object (pool is full)
    }

    /// Get the number of objects available in the pool.
    #[must_use]
    pub fn available(&self) -> usize {
        self.pool.len()
    }

    /// Get the number of objects currently in use.
    #[must_use]
    pub fn in_use(&self) -> usize {
        self.in_use
    }

    /// Get the maximum pool capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.max_capacity
    }

    /// Clear the pool, dropping all available objects.
    pub fn clear(&mut self) {
        self.pool.clear();
        self.in_use = 0;
    }

    /// Preallocate objects into the pool using a factory function.
    pub fn preallocate<F>(&mut self, count: usize, mut factory: F)
    where
        F: FnMut() -> T,
    {
        let to_allocate = count.min(self.max_capacity - self.pool.len());
        for _ in 0..to_allocate {
            self.pool.push_back(factory());
        }
    }

    /// Get statistics about pool usage.
    #[must_use]
    pub fn stats(&self) -> ObjectPoolStats {
        ObjectPoolStats {
            available: self.pool.len(),
            in_use: self.in_use,
            capacity: self.max_capacity,
            utilization: (self.in_use as f64 / self.max_capacity as f64) * 100.0,
        }
    }
}

/// Statistics about object pool usage.
#[derive(Debug, Clone, Copy)]
pub struct ObjectPoolStats {
    pub available: usize,
    pub in_use: usize,
    pub capacity: usize,
    pub utilization: f64,
}

/// A pooled object that automatically returns to the pool when dropped.
pub struct PooledObject<T> {
    object: Option<T>,
    free_list: Arc<Mutex<VecDeque<T>>>,
}

impl<T> PooledObject<T> {
    /// Create a new pooled object backed by a shared free list.
    #[allow(dead_code)]
    pub(crate) fn new(object: T, free_list: Arc<Mutex<VecDeque<T>>>) -> Self {
        Self {
            object: Some(object),
            free_list,
        }
    }

    /// Get a reference to the inner object.
    #[must_use]
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    /// Get a mutable reference to the inner object.
    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            if let Ok(mut list) = self.free_list.lock() {
                list.push_back(object);
            }
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestObject {
        id: u32,
    }

    #[test]
    fn test_pool_basic() {
        let mut pool = ObjectPool::new(10);

        let obj1 = TestObject { id: 1 };
        pool.release(obj1);

        assert_eq!(pool.available(), 1);

        let obj = pool.acquire().unwrap();
        assert_eq!(obj.id, 1);
        assert_eq!(pool.available(), 0);
    }

    #[test]
    fn test_pool_acquire_or_create() {
        let mut pool = ObjectPool::new(10);

        let obj = pool.acquire_or_create(|| TestObject { id: 42 });
        assert_eq!(obj.id, 42);

        pool.release(obj);

        let obj2 = pool.acquire_or_create(|| TestObject { id: 99 });
        assert_eq!(obj2.id, 42); // Should get the pooled object, not create new
    }

    #[test]
    fn test_pool_capacity_limit() {
        let mut pool = ObjectPool::new(2);

        pool.release(TestObject { id: 1 });
        pool.release(TestObject { id: 2 });
        pool.release(TestObject { id: 3 }); // Should be dropped

        assert_eq!(pool.available(), 2);
    }

    #[test]
    fn test_pool_preallocate() {
        let mut pool = ObjectPool::<TestObject>::new(10);

        pool.preallocate(5, || TestObject { id: 0 });

        assert_eq!(pool.available(), 5);
    }

    #[test]
    fn test_pool_stats() {
        let mut pool = ObjectPool::new(10);

        pool.preallocate(5, || TestObject { id: 0 });
        let _obj1 = pool.acquire();
        let _obj2 = pool.acquire();

        let stats = pool.stats();
        assert_eq!(stats.available, 3);
        assert_eq!(stats.in_use, 2);
    }

    #[test]
    fn test_pool_clear() {
        let mut pool = ObjectPool::new(10);

        pool.release(TestObject { id: 1 });
        pool.release(TestObject { id: 2 });

        pool.clear();

        assert_eq!(pool.available(), 0);
        assert_eq!(pool.in_use(), 0);
    }
}
