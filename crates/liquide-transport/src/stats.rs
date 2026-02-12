//! Transport-level statistics (bytes, messages, errors).

use std::sync::atomic::{AtomicU64, Ordering};

/// Tracks transport-level counters.
///
/// All operations use relaxed atomic ordering — these are advisory metrics,
/// not synchronisation primitives.
#[derive(Debug, Default)]
pub struct TransportStats {
    bytes_sent: AtomicU64,
    bytes_recv: AtomicU64,
    messages_sent: AtomicU64,
    messages_recv: AtomicU64,
    errors: AtomicU64,
}

impl TransportStats {
    /// Create a zeroed stats instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record bytes sent.
    pub fn record_send(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record bytes received.
    pub fn record_recv(&self, bytes: u64) {
        self.bytes_recv.fetch_add(bytes, Ordering::Relaxed);
        self.messages_recv.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an error.
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Total bytes sent.
    #[must_use]
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    /// Total bytes received.
    #[must_use]
    pub fn bytes_recv(&self) -> u64 {
        self.bytes_recv.load(Ordering::Relaxed)
    }

    /// Total messages sent.
    #[must_use]
    pub fn messages_sent(&self) -> u64 {
        self.messages_sent.load(Ordering::Relaxed)
    }

    /// Total messages received.
    #[must_use]
    pub fn messages_recv(&self) -> u64 {
        self.messages_recv.load(Ordering::Relaxed)
    }

    /// Total errors observed.
    #[must_use]
    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    /// Take a snapshot of all counters.
    #[must_use]
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            bytes_sent: self.bytes_sent(),
            bytes_recv: self.bytes_recv(),
            messages_sent: self.messages_sent(),
            messages_recv: self.messages_recv(),
            errors: self.errors(),
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_recv.store(0, Ordering::Relaxed);
        self.messages_sent.store(0, Ordering::Relaxed);
        self.messages_recv.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
    }
}

/// A point-in-time copy of [`TransportStats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StatsSnapshot {
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub messages_sent: u64,
    pub messages_recv: u64,
    pub errors: u64,
}
