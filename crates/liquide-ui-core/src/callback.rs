//! Callback and signal mechanism (Qt-style signals/slots pattern).

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CB: AtomicU64 = AtomicU64::new(1);

/// A unique callback identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallbackId(u64);

impl CallbackId {
    pub fn new() -> Self {
        Self(NEXT_CB.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for CallbackId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CallbackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CB{}", self.0)
    }
}

/// A type-erased callback that can be stored and invoked.
pub struct Callback<T: 'static = ()> {
    pub id: CallbackId,
    func: Box<dyn FnMut(T) + Send>,
}

impl<T: 'static> Callback<T> {
    /// Create a new callback wrapping a closure.
    pub fn new(f: impl FnMut(T) + Send + 'static) -> Self {
        Self {
            id: CallbackId::new(),
            func: Box::new(f),
        }
    }

    /// Invoke the callback with a value.
    pub fn call(&mut self, value: T) {
        (self.func)(value);
    }
}

impl<T: 'static> fmt::Debug for Callback<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Callback({})", self.id)
    }
}
