//! TCP transport backend.

use std::net::SocketAddr;

use bytes::Bytes;

/// Plain TCP transport (TLS is layered on top externally).
pub struct TcpTransport {
    /// Remote address, if connected.
    remote: Option<SocketAddr>,
}

impl TcpTransport {
    /// Create a new, unconnected TCP transport.
    #[must_use]
    pub fn new() -> Self {
        Self { remote: None }
    }
}

impl Default for TcpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Transport for TcpTransport {
    async fn connect(&mut self, addr: SocketAddr) -> super::Result<()> {
        self.remote = Some(addr);
        todo!("TCP connect")
    }

    async fn accept(&self) -> super::Result<Box<dyn super::Transport>> {
        todo!("TCP accept")
    }

    async fn send(&self, _data: Bytes) -> super::Result<()> {
        todo!("TCP send")
    }

    async fn recv(&self) -> super::Result<Bytes> {
        todo!("TCP recv")
    }

    async fn close(&mut self) -> super::Result<()> {
        self.remote = None;
        Ok(())
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        self.remote
    }
}
