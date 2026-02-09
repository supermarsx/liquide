//! QUIC transport backend.

use std::net::SocketAddr;

use bytes::Bytes;

/// QUIC-based transport using the QUIC protocol for multiplexed, encrypted streams.
pub struct QuicTransport {
    /// Remote address, if connected.
    remote: Option<SocketAddr>,
}

impl QuicTransport {
    /// Create a new, unconnected QUIC transport.
    #[must_use]
    pub fn new() -> Self {
        Self { remote: None }
    }
}

impl Default for QuicTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Transport for QuicTransport {
    async fn connect(&mut self, addr: SocketAddr) -> super::Result<()> {
        self.remote = Some(addr);
        todo!("QUIC connect")
    }

    async fn send(&self, _data: Bytes) -> super::Result<()> {
        todo!("QUIC send")
    }

    async fn recv(&self) -> super::Result<Bytes> {
        todo!("QUIC recv")
    }

    async fn close(&mut self) -> super::Result<()> {
        self.remote = None;
        Ok(())
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        self.remote
    }
}
