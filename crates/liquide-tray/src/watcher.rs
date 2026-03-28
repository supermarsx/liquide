//! Status notifier watcher — tracks registered hosts and items.
//!
//! The watcher acts as the central registry for the StatusNotifierWatcher
//! D-Bus service. Hosts register themselves so that items can discover whether
//! a tray exists; items register so that hosts can be notified of new arrivals.

use std::collections::HashSet;

/// Unique identifier for a host (typically a D-Bus bus name).
pub type HostId = String;

/// Unique identifier for an item (typically a D-Bus bus name + object path).
pub type WatchedItemId = String;

/// Signals emitted by the watcher when hosts or items change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusNotifierWatcherSignal {
    /// A new host registered itself.
    HostRegistered(HostId),
    /// A host unregistered.
    HostUnregistered(HostId),
    /// A new item was registered.
    ItemRegistered(WatchedItemId),
    /// An item was unregistered.
    ItemUnregistered(WatchedItemId),
}

impl StatusNotifierWatcherSignal {
    /// Returns `true` if this signal concerns a host.
    pub fn is_host_signal(&self) -> bool {
        matches!(
            self,
            Self::HostRegistered(_) | Self::HostUnregistered(_)
        )
    }

    /// Returns `true` if this signal concerns an item.
    pub fn is_item_signal(&self) -> bool {
        matches!(
            self,
            Self::ItemRegistered(_) | Self::ItemUnregistered(_)
        )
    }

    /// Returns the identifier (host or item) associated with this signal.
    pub fn id(&self) -> &str {
        match self {
            Self::HostRegistered(id)
            | Self::HostUnregistered(id)
            | Self::ItemRegistered(id)
            | Self::ItemUnregistered(id) => id,
        }
    }
}

/// The status notifier watcher — central registry tracking which hosts and
/// items are alive on the bus.
pub struct TrayWatcher {
    /// Set of registered host identifiers.
    registered_hosts: HashSet<HostId>,
    /// Set of registered item identifiers.
    registered_items: HashSet<WatchedItemId>,
    /// Accumulated signals since last drain.
    signals: Vec<StatusNotifierWatcherSignal>,
}

impl TrayWatcher {
    /// Create a new, empty watcher.
    pub fn new() -> Self {
        Self {
            registered_hosts: HashSet::new(),
            registered_items: HashSet::new(),
            signals: Vec::new(),
        }
    }

    // ── Host management ────────────────────────────────────────────────

    /// Register a host. Returns `true` if the host was newly registered,
    /// `false` if it was already known.
    pub fn register_host(&mut self, host_id: impl Into<HostId>) -> bool {
        let id = host_id.into();
        if self.registered_hosts.insert(id.clone()) {
            self.signals
                .push(StatusNotifierWatcherSignal::HostRegistered(id));
            true
        } else {
            false
        }
    }

    /// Unregister a host. Returns `true` if it was previously registered.
    pub fn unregister_host(&mut self, host_id: &str) -> bool {
        if self.registered_hosts.remove(host_id) {
            self.signals
                .push(StatusNotifierWatcherSignal::HostUnregistered(
                    host_id.to_string(),
                ));
            true
        } else {
            false
        }
    }

    /// Returns `true` if at least one host is registered, meaning applications
    /// should create tray items because a tray exists to display them.
    pub fn is_host_registered(&self) -> bool {
        !self.registered_hosts.is_empty()
    }

    /// Returns the number of registered hosts.
    pub fn host_count(&self) -> usize {
        self.registered_hosts.len()
    }

    /// Returns all registered host IDs.
    pub fn registered_hosts(&self) -> Vec<&str> {
        self.registered_hosts.iter().map(|s| s.as_str()).collect()
    }

    // ── Item management ────────────────────────────────────────────────

    /// Register an item. Returns `true` if the item was newly registered,
    /// `false` if it was already known.
    pub fn register_item(&mut self, item_id: impl Into<WatchedItemId>) -> bool {
        let id = item_id.into();
        if self.registered_items.insert(id.clone()) {
            self.signals
                .push(StatusNotifierWatcherSignal::ItemRegistered(id));
            true
        } else {
            false
        }
    }

    /// Unregister an item. Returns `true` if it was previously registered.
    pub fn unregister_item(&mut self, item_id: &str) -> bool {
        if self.registered_items.remove(item_id) {
            self.signals
                .push(StatusNotifierWatcherSignal::ItemUnregistered(
                    item_id.to_string(),
                ));
            true
        } else {
            false
        }
    }

    /// Returns `true` if a specific item is currently registered.
    pub fn is_item_registered(&self, item_id: &str) -> bool {
        self.registered_items.contains(item_id)
    }

    /// Returns the number of registered items.
    pub fn item_count(&self) -> usize {
        self.registered_items.len()
    }

    /// Returns all registered item IDs.
    pub fn registered_items(&self) -> Vec<&str> {
        self.registered_items.iter().map(|s| s.as_str()).collect()
    }

    // ── Signal management ──────────────────────────────────────────────

    /// Drain all accumulated signals since the last call.
    pub fn drain_signals(&mut self) -> Vec<StatusNotifierWatcherSignal> {
        std::mem::take(&mut self.signals)
    }

    /// Peek at pending signals without draining.
    pub fn pending_signals(&self) -> &[StatusNotifierWatcherSignal] {
        &self.signals
    }

    /// Clear all state (hosts, items, signals).
    pub fn clear(&mut self) {
        self.registered_hosts.clear();
        self.registered_items.clear();
        self.signals.clear();
    }
}

impl Default for TrayWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrayWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TrayWatcher({} hosts, {} items)",
            self.host_count(),
            self.item_count()
        )
    }
}
