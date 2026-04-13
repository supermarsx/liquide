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
