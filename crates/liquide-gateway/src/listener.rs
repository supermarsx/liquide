//! Transport listener management.

use tokio::net::TcpListener;

use crate::config::ListenConfig;
use crate::GatewayError;

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
    /// The actual bound TCP listener (populated after `start()`).
    tcp_listener: Option<TcpListener>,
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
            tcp_listener: None,
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

    /// Whether the listener is in the `Listening` state.
    #[must_use]
    pub fn is_listening(&self) -> bool {
        self.state == ListenerState::Listening
    }

    /// Bind the TCP socket and transition to `Listening`.
    pub async fn start(&mut self) -> crate::Result<()> {
        let addr = &self.config.address;
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            GatewayError::ListenerBindFailed {
                addr: addr.clone(),
                reason: e.to_string(),
            }
        })?;
        tracing::info!(addr = %addr, id = %self.id, "listener bound");
        self.tcp_listener = Some(listener);
        self.state = ListenerState::Listening;
        Ok(())
    }

    /// Accept the next inbound TCP connection.
    ///
    /// Returns the stream and increments the accept counter.
    /// Returns an error if the listener has not been started.
    pub async fn accept(
        &mut self,
    ) -> crate::Result<(tokio::net::TcpStream, std::net::SocketAddr)> {
        let listener = self.tcp_listener.as_ref().ok_or_else(|| {
            GatewayError::ListenerBindFailed {
                addr: self.config.address.clone(),
                reason: "listener not started".into(),
            }
        })?;
        let (stream, peer_addr) = listener.accept().await.map_err(|e| {
            GatewayError::ListenerBindFailed {
                addr: self.config.address.clone(),
                reason: format!("accept: {e}"),
            }
        })?;
        self.connections_accepted += 1;
        tracing::debug!(peer = %peer_addr, id = %self.id, "accepted connection");
        Ok((stream, peer_addr))
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
