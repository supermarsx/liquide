//! Helpers for recovering from poisoned standard-library locks.
//!
//! When a thread panics while holding a `Mutex` or `RwLock`, the lock
//! becomes "poisoned".  Rather than propagating the panic to every
//! subsequent caller (which cascades a single crash into a full
//! process failure), these helpers log a warning and recover the
//! inner data.

/// Lock a [`std::sync::Mutex`], recovering from poison.
pub fn lock_or_recover<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| {
        tracing::warn!("recovering from poisoned mutex");
        poisoned.into_inner()
    })
}

/// Acquire a read guard on a [`std::sync::RwLock`], recovering from poison.
pub fn read_or_recover<T>(rw: &std::sync::RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    rw.read().unwrap_or_else(|poisoned| {
        tracing::warn!("recovering from poisoned rwlock");
        poisoned.into_inner()
    })
}

/// Acquire a write guard on a [`std::sync::RwLock`], recovering from poison.
pub fn write_or_recover<T>(rw: &std::sync::RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    rw.write().unwrap_or_else(|poisoned| {
        tracing::warn!("recovering from poisoned rwlock");
        poisoned.into_inner()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, RwLock};

    #[test]
    fn test_lock_or_recover_normal() {
        let m = Mutex::new(42);
        let guard = lock_or_recover(&m);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_read_or_recover_normal() {
        let rw = RwLock::new("hello");
        let guard = read_or_recover(&rw);
        assert_eq!(*guard, "hello");
    }

    #[test]
    fn test_write_or_recover_normal() {
        let rw = RwLock::new(10u32);
        {
            let mut guard = write_or_recover(&rw);
            *guard = 20;
        }
        let guard = read_or_recover(&rw);
        assert_eq!(*guard, 20);
    }

    #[test]
    fn test_lock_or_recover_poisoned() {
        let m = Arc::new(Mutex::new(42));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _guard = m2.lock().unwrap();
            panic!("intentional poison");
        })
        .join();
        // Lock is poisoned now, but lock_or_recover should recover.
        assert!(m.lock().is_err());
        let guard = lock_or_recover(&m);
        assert_eq!(*guard, 42);
    }
}
