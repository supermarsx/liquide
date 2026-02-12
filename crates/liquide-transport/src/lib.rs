#![doc = "Transport layer abstractions for the LiquiDE protocol."]
#![doc = ""]
#![doc = "Provides a unified `Transport` trait with pluggable backends for TCP,"]
#![doc = "UDP, QUIC (feature `quic`), and WebSocket (feature `websocket`)."]
#![doc = "The transport layer sits between protocol framing (`liquide-protocol`)"]
#![doc = "and the session management layer above."]

pub mod backoff;
pub mod codec;
pub mod connection;
pub mod listener;
pub mod pool;
pub mod stats;
pub mod tcp;
#[cfg(feature = "tls")]
pub mod tls;
pub mod udp;

#[cfg(feature = "quic")]
pub mod quic;
#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(test)]
mod tests;

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

    /// A protocol-level framing or encoding error.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// The local address is already in use.
    #[error("address in use: {0}")]
    AddressInUse(SocketAddr),

    /// The message exceeds the maximum allowed size.
    #[error("message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, TransportError>;

/// Maximum size of a single transport message (16 MiB).
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// A transport-agnostic connection handle.
///
/// Implementors wrap a concrete transport (TCP socket, QUIC stream, etc.) and
/// expose a uniform async API for sending and receiving length-delimited byte
/// messages.
#[allow(async_fn_in_trait)]
pub trait Transport: Send + Sync + 'static {
    /// Establish a connection to the given remote address.
    async fn connect(&mut self, addr: SocketAddr) -> Result<()>;

    /// Send a message to the connected peer.
    ///
    /// For stream transports (TCP, QUIC) the message is length-prefixed
    /// automatically.  For datagram transports (UDP) and message transports
    /// (WebSocket) the natural message boundary is used.
    async fn send(&self, data: Bytes) -> Result<()>;

    /// Receive a message from the connected peer.
    async fn recv(&self) -> Result<Bytes>;

    /// Gracefully close the transport.
    async fn close(&mut self) -> Result<()>;

    /// Return the remote peer address, if connected.
    fn peer_addr(&self) -> Option<SocketAddr>;

    /// Return the local bound address, if available.
    fn local_addr(&self) -> Option<SocketAddr> {
        None
    }

    /// Check whether the transport is currently connected.
    fn is_connected(&self) -> bool {
        self.peer_addr().is_some()
    }
}
