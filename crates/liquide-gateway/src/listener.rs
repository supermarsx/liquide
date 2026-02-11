//! Transport listener management.

use crate::config::ListenConfig;

/// Lifecycle state of a transport listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenerState {
    /// Socket is bound but not yet accepting.
    Bound,
    /// Actively accepting connections.
    Listening,
    /// Temporarily paused (e.g. during reload).
    Paused,
    /// Permanently closed.
    Closed,
}

impl std::fmt::Display for ListenerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bound => write!(f, "bound"),
            Self::Listening => write!(f, "listening"),
            Self::Paused => write!(f, "paused"),
            Self::Closed => write!(f, "closed"),
        }
    }
}

/// A single transport listener endpoint.
pub struct TransportListener {
    id: String,
    config: ListenConfig,
    state: ListenerState,
    connections_accepted: u64,
    connections_rejected: u64,
}

impl TransportListener {
    /// Create a new transport listener in `Bound` state.
    #[must_use]
    pub fn new(id: String, config: ListenConfig) -> Self {
        Self {
            id,
            config,
            state: ListenerState::Bound,
            connections_accepted: 0,
            connections_rejected: 0,
        }
    }

    /// Listener identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Listener configuration.
    #[must_use]
    pub fn config(&self) -> &ListenConfig {
        &self.config
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> ListenerState {
        self.state
    }

    /// Total connections accepted.
    #[must_use]
    pub fn accept_count(&self) -> u64 {
        self.connections_accepted
    }

    /// Total connections rejected.
    #[must_use]
    pub fn reject_count(&self) -> u64 {
        self.connections_rejected
    }

    /// Record an accepted connection.
    pub fn record_accept(&mut self) {
        self.connections_accepted += 1;
    }

    /// Record a rejected connection.
    pub fn record_reject(&mut self) {
        self.connections_rejected += 1;
    }

    /// Pause accepting new connections.
    pub fn pause(&mut self) {
        if self.state == ListenerState::Listening {
            self.state = ListenerState::Paused;
        }
    }

    /// Resume accepting connections.
    pub fn resume(&mut self) {
        if self.state == ListenerState::Paused || self.state == ListenerState::Bound {
            self.state = ListenerState::Listening;
        }
    }
}

/// Manages all transport listeners for the gateway.
pub struct ListenerManager {
    listeners: Vec<TransportListener>,
}

impl ListenerManager {
    /// Create an empty listener manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    /// Add a listener from configuration. Returns its assigned ID.
    pub fn add_listener(&mut self, config: ListenConfig) -> String {
        let id = format!("listener-{}", self.listeners.len() + 1);
        let listener = TransportListener::new(id.clone(), config);
        self.listeners.push(listener);
        id
    }

    /// Remove a listener by ID.
    pub fn remove_listener(&mut self, id: &str) -> bool {
        let before = self.listeners.len();
        self.listeners.retain(|l| l.id() != id);
        self.listeners.len() < before
    }

    /// Total accepted connections across all listeners.
    #[must_use]
    pub fn total_accepts(&self) -> u64 {
        self.listeners.iter().map(|l| l.accept_count()).sum()
    }

    /// Total rejected connections across all listeners.
    #[must_use]
    pub fn total_rejects(&self) -> u64 {
        self.listeners.iter().map(|l| l.reject_count()).sum()
    }

    /// Get a reference to a listener by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&TransportListener> {
        self.listeners.iter().find(|l| l.id() == id)
    }

    /// Get a mutable reference to a listener by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut TransportListener> {
        self.listeners.iter_mut().find(|l| l.id() == id)
    }

    /// List all listeners.
    #[must_use]
    pub fn all(&self) -> &[TransportListener] {
        &self.listeners
    }
}

impl Default for ListenerManager {
    fn default() -> Self {
        Self::new()
    }
}
