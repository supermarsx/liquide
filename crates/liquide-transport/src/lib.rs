#![doc = "Transport layer abstractions for the Liquide protocol."]
#![doc = ""]
#![doc = "Provides a unified `Transport` trait with pluggable backends for QUIC,"]
#![doc = "TCP, UDP, and WebSocket.  The transport layer sits between the protocol"]
#![doc = "framing (`liquide-protocol`) and the session management layer above."]

pub mod connection;
pub mod listener;
pub mod quic;
pub mod tcp;
pub mod udp;
pub mod websocket;

use bytes::Bytes;
use std::net::SocketAddr;
use thiserror::Error;

/// Errors produced by the transport layer.
#[derive(Debug, Error)]
pub enum TransportError {
    /// The underlying I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TLS or cryptographic failure.
    #[error("TLS error: {0}")]
    Tls(String),

    /// The connection was reset by the peer.
    #[error("connection reset by peer")]
    ConnectionReset,

    /// A timeout expired before the operation completed.
    #[error("operation timed out")]
    Timeout,

    /// The transport is not connected.
    #[error("not connected")]
    NotConnected,
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, TransportError>;

/// A transport-agnostic connection handle.
///
/// Implementors wrap a concrete transport (QUIC stream, TCP socket, etc.) and
/// expose a uniform async API.
#[allow(async_fn_in_trait)]
pub trait Transport: Send + Sync + 'static {
    /// Establish a connection to the given remote address.
    async fn connect(&mut self, addr: SocketAddr) -> Result<()>;

    /// Send a frame payload to the connected peer.
    async fn send(&self, data: Bytes) -> Result<()>;

    /// Receive a frame payload from the connected peer.
    async fn recv(&self) -> Result<Bytes>;

    /// Gracefully close the transport.
    async fn close(&mut self) -> Result<()>;

    /// Return the remote peer address, if connected.
    fn peer_addr(&self) -> Option<SocketAddr>;
}
