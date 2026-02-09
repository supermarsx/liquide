//! Server-side listener abstraction.

use std::net::SocketAddr;

/// Configuration for a transport listener.
#[derive(Debug, Clone)]
pub struct ListenerConfig {
    /// The local address to bind to.
    pub bind_addr: SocketAddr,
    /// Maximum number of pending connections in the accept queue.
    pub backlog: u32,
}

/// A transport listener that accepts inbound connections.
pub struct Listener {
    /// The configuration this listener was created with.
    pub config: ListenerConfig,
}

impl Listener {
    /// Create a new listener with the given configuration.
    #[must_use]
    pub fn new(config: ListenerConfig) -> Self {
        Self { config }
    }

    /// Start listening and accept connections.
    ///
    /// This is a stub that will be implemented per transport backend.
    pub async fn accept(&self) -> super::Result<super::connection::Connection> {
        todo!("Listener::accept")
    }
}
