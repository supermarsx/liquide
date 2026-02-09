//! WebSocket transport backend (for browser-based clients and firewall traversal).

use std::net::SocketAddr;

use bytes::Bytes;

/// WebSocket transport for browser clients.
pub struct WebSocketTransport {
    /// Remote address, if connected.
    remote: Option<SocketAddr>,
}

impl WebSocketTransport {
    /// Create a new, unconnected WebSocket transport.
    #[must_use]
    pub fn new() -> Self {
        Self { remote: None }
    }
}

impl Default for WebSocketTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Transport for WebSocketTransport {
    async fn connect(&mut self, addr: SocketAddr) -> super::Result<()> {
        self.remote = Some(addr);
        todo!("WebSocket connect")
    }

    async fn accept(&self) -> super::Result<Box<dyn super::Transport>> {
        todo!("WebSocket accept")
    }

    async fn send(&self, _data: Bytes) -> super::Result<()> {
        todo!("WebSocket send")
    }

    async fn recv(&self) -> super::Result<Bytes> {
        todo!("WebSocket recv")
    }

    async fn close(&mut self) -> super::Result<()> {
        self.remote = None;
        Ok(())
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        self.remote
    }
}
