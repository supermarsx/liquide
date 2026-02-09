//! High-level connection abstraction that wraps a concrete transport.

use std::net::SocketAddr;

/// Represents an established connection to a remote peer.
///
/// This is the primary handle given to upper layers after the transport
/// has completed its handshake.
pub struct Connection {
    /// The peer's socket address.
    pub peer: SocketAddr,
    /// Whether the connection is still alive.
    pub alive: bool,
}

impl Connection {
    /// Create a new connection handle.
    #[must_use]
    pub fn new(peer: SocketAddr) -> Self {
        Self { peer, alive: true }
    }

    /// Mark the connection as closed.
    pub fn close(&mut self) {
        self.alive = false;
    }
}
