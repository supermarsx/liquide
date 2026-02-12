//! High-level connection wrapper with frame-level I/O and statistics.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use liquide_protocol::FrameHeader;

use crate::codec;
use crate::stats::TransportStats;
use crate::Transport;

/// A framed connection that wraps a [`Transport`] and adds protocol-level
/// frame encoding/decoding plus statistics tracking.
pub struct Connection<T: Transport> {
    transport: T,
    stats: Arc<TransportStats>,
}

impl<T: Transport> Connection<T> {
    /// Create a connection by wrapping an existing transport.
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            stats: Arc::new(TransportStats::new()),
        }
    }

    /// Create a connection and connect to `addr`.
    pub async fn connect(mut transport: T, addr: SocketAddr) -> crate::Result<Self> {
        transport.connect(addr).await?;
        Ok(Self {
            transport,
            stats: Arc::new(TransportStats::new()),
        })
    }

    /// Send a protocol frame (header + payload) over the transport.
    ///
    /// The header and payload are serialised into a single transport message.
    pub async fn send_frame(
        &self,
        header: &FrameHeader,
        payload: &[u8],
    ) -> crate::Result<()> {
        let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE + payload.len());
        codec::encode_frame(header, payload, &mut buf);
        let data = buf.freeze();
        self.stats.record_send(data.len() as u64);
        self.transport.send(data).await
    }

    /// Receive a protocol frame from the transport.
    pub async fn recv_frame(&self) -> crate::Result<(FrameHeader, Bytes)> {
        let data = self.transport.recv().await?;
        self.stats.record_recv(data.len() as u64);
        codec::decode_frame(&data)
    }

    /// Send raw bytes (without frame encoding).
    pub async fn send_raw(&self, data: Bytes) -> crate::Result<()> {
        self.stats.record_send(data.len() as u64);
        self.transport.send(data).await
    }

    /// Receive raw bytes (without frame decoding).
    pub async fn recv_raw(&self) -> crate::Result<Bytes> {
        let data = self.transport.recv().await?;
        self.stats.record_recv(data.len() as u64);
        Ok(data)
    }

    /// Close the underlying transport.
    pub async fn close(&mut self) -> crate::Result<()> {
        self.transport.close().await
    }

    /// Get a reference to the transport statistics.
    #[must_use]
    pub fn stats(&self) -> &TransportStats {
        &self.stats
    }

    /// Get a clone of the stats [`Arc`] for sharing.
    #[must_use]
    pub fn stats_shared(&self) -> Arc<TransportStats> {
        Arc::clone(&self.stats)
    }

    /// Remote peer address, if connected.
    #[must_use]
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.transport.peer_addr()
    }

    /// Whether the transport is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    /// Access the inner transport.
    #[must_use]
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Consume the connection and return the inner transport.
    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }
}
