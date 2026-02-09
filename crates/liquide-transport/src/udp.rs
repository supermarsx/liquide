//! UDP transport backend (unreliable datagrams for latency-sensitive channels).

use std::net::SocketAddr;

use bytes::Bytes;

/// UDP-based transport for latency-sensitive, loss-tolerant data.
pub struct UdpTransport {
    /// Remote address, if connected.
    remote: Option<SocketAddr>,
}

impl UdpTransport {
    /// Create a new, unconnected UDP transport.
    #[must_use]
    pub fn new() -> Self {
        Self { remote: None }
    }
}

impl Default for UdpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Transport for UdpTransport {
    async fn connect(&mut self, addr: SocketAddr) -> super::Result<()> {
        self.remote = Some(addr);
        todo!("UDP connect")
    }

    async fn send(&self, _data: Bytes) -> super::Result<()> {
        todo!("UDP send")
    }

    async fn recv(&self) -> super::Result<Bytes> {
        todo!("UDP recv")
    }

    async fn close(&mut self) -> super::Result<()> {
        self.remote = None;
        Ok(())
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        self.remote
    }
}
