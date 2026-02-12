//! UDP transport backend for latency-sensitive, loss-tolerant data.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio::net::UdpSocket;

/// Maximum UDP payload we will accept (64 KiB minus IP/UDP headers).
const MAX_DATAGRAM: usize = 65_507;

/// UDP-based transport.
///
/// Uses a "connected" UDP socket so that [`send`](crate::Transport::send) and
/// [`recv`](crate::Transport::recv) operate without specifying the peer each
/// time.  Each datagram is one complete message — no length-prefix framing is
/// applied.
pub struct UdpTransport {
    remote: Option<SocketAddr>,
    local: Option<SocketAddr>,
    socket: Option<Arc<UdpSocket>>,
}

impl UdpTransport {
    /// Create a new, unconnected UDP transport.
    #[must_use]
    pub fn new() -> Self {
        Self {
            remote: None,
            local: None,
            socket: None,
        }
    }

    /// Wrap an already-connected [`UdpSocket`].
    pub fn from_socket(socket: UdpSocket, remote: SocketAddr) -> crate::Result<Self> {
        let local = socket.local_addr().ok();
        Ok(Self {
            remote: Some(remote),
            local,
            socket: Some(Arc::new(socket)),
        })
    }
}

impl Default for UdpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::Transport for UdpTransport {
    async fn connect(&mut self, addr: SocketAddr) -> crate::Result<()> {
        // Bind to an ephemeral port matching the address family.
        let bind_addr: SocketAddr = if addr.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let socket = UdpSocket::bind(bind_addr).await?;
        socket.connect(addr).await?;
        self.local = socket.local_addr().ok();
        self.remote = Some(addr);
        self.socket = Some(Arc::new(socket));
        tracing::debug!(%addr, "UDP connected");
        Ok(())
    }

    async fn send(&self, data: Bytes) -> crate::Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        if data.len() > MAX_DATAGRAM {
            return Err(crate::TransportError::MessageTooLarge {
                size: data.len(),
                max: MAX_DATAGRAM,
            });
        }
        socket.send(&data).await?;
        Ok(())
    }

    async fn recv(&self) -> crate::Result<Bytes> {
        let socket = self
            .socket
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        let mut buf = vec![0u8; MAX_DATAGRAM];
        let n = socket.recv(&mut buf).await?;
        buf.truncate(n);
        Ok(Bytes::from(buf))
    }

    async fn close(&mut self) -> crate::Result<()> {
        self.socket = None;
        self.remote = None;
        self.local = None;
        Ok(())
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        self.remote
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.local
    }
}
