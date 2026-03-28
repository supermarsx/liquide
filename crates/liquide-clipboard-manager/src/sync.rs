//! Multi-device clipboard synchronisation abstraction.
//!
//! The actual network transport is outside this crate's scope — this module
//! provides the trait and a local stub so upper layers can queue outgoing
//! entries and poll for incoming ones.

use crate::entry::ClipboardEntry;

/// Trait for clipboard sync backends (network, shared-memory, etc.).
pub trait ClipboardSyncBackend {
    /// Queue an entry for outgoing sync to other devices.
    fn queue_outgoing(&mut self, entry: &ClipboardEntry);

    /// Poll for entries received from other devices since the last call.
    /// Returns an empty vec when nothing new is available.
    fn receive_incoming(&mut self) -> Vec<ClipboardEntry>;

    /// Whether the sync channel is currently connected.
    fn is_connected(&self) -> bool;
}

/// Local (in-process) stub implementation of [`ClipboardSyncBackend`].
///
/// Outgoing entries are stored in a ring buffer and can be drained via
/// [`receive_incoming`] on the same instance — useful for testing and
/// single-machine setups.
pub struct LocalSyncStub {
    enabled: bool,
    outgoing: Vec<ClipboardEntry>,
    incoming: Vec<ClipboardEntry>,
}

impl LocalSyncStub {
    /// Create a new stub with sync disabled by default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: false,
            outgoing: Vec::new(),
            incoming: Vec::new(),
        }
    }

    /// Enable or disable sync.
    pub fn set_sync_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether sync is enabled.
    #[must_use]
    pub fn is_sync_enabled(&self) -> bool {
        self.enabled
    }

    /// Simulate receiving entries from a remote device by pushing them into
    /// the incoming buffer.  This is a test helper.
    pub fn inject_incoming(&mut self, entry: ClipboardEntry) {
        self.incoming.push(entry);
    }

    /// Return a reference to all queued outgoing entries (test helper).
    #[must_use]
    pub fn pending_outgoing(&self) -> &[ClipboardEntry] {
        &self.outgoing
    }

    /// Move all outgoing entries into the incoming buffer, simulating a
    /// round-trip.  Returns the number of entries looped back.
    pub fn loopback(&mut self) -> usize {
        let n = self.outgoing.len();
        self.incoming.append(&mut self.outgoing);
        n
    }
}

impl Default for LocalSyncStub {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardSyncBackend for LocalSyncStub {
    fn queue_outgoing(&mut self, entry: &ClipboardEntry) {
        if !self.enabled {
            return;
        }
        self.outgoing.push(entry.clone());
    }

    fn receive_incoming(&mut self) -> Vec<ClipboardEntry> {
        if !self.enabled {
            return Vec::new();
        }
        std::mem::take(&mut self.incoming)
    }

    fn is_connected(&self) -> bool {
        self.enabled
    }
}

/// High-level clipboard sync coordinator that wraps a backend and tracks
/// whether sync is active.
pub struct ClipboardSync<B: ClipboardSyncBackend = LocalSyncStub> {
    backend: B,
}

impl<B: ClipboardSyncBackend> ClipboardSync<B> {
    /// Wrap a sync backend.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Queue an entry for outgoing sync.
    pub fn queue_outgoing(&mut self, entry: &ClipboardEntry) {
        self.backend.queue_outgoing(entry);
    }

    /// Poll for incoming entries.
    pub fn receive_incoming(&mut self) -> Vec<ClipboardEntry> {
        self.backend.receive_incoming()
    }

    /// Whether the backend is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.backend.is_connected()
    }

    /// Mutable access to the underlying backend.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Immutable access to the underlying backend.
    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }
}
