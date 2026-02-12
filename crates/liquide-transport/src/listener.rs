//! Server-side listener abstractions for accepting inbound connections.

use std::net::SocketAddr;

use tokio::net::{TcpListener as TokioTcpListener, TcpStream};

use crate::tcp::TcpTransport;

/// Configuration for a transport listener.
#[derive(Debug, Clone)]
pub struct ListenerConfig {
    /// The local address to bind to.
    pub bind_addr: SocketAddr,
    /// Maximum number of pending connections in the accept queue.
    pub backlog: u32,
}

// ---------------------------------------------------------------------------
// TCP Listener
// ---------------------------------------------------------------------------

/// A TCP listener that accepts inbound connections and wraps each in a
/// [`TcpTransport`].
pub struct TcpListener {
    inner: TokioTcpListener,
    local_addr: SocketAddr,
}

impl TcpListener {
    /// Bind a TCP listener to the given address.
    pub async fn bind(addr: SocketAddr) -> crate::Result<Self> {
        let inner = TokioTcpListener::bind(addr).await?;
        let local_addr = inner.local_addr()?;
        tracing::info!(%local_addr, "TCP listener bound");
        Ok(Self { inner, local_addr })
    }

    /// Bind with a full [`ListenerConfig`].
    pub async fn bind_config(config: &ListenerConfig) -> crate::Result<Self> {
        Self::bind(config.bind_addr).await
    }

    /// Accept one inbound connection.
    ///
    /// Returns the [`TcpTransport`] and the peer's address.
    pub async fn accept(&self) -> crate::Result<(TcpTransport, SocketAddr)> {
        let (stream, peer) = self.inner.accept().await?;
        let transport = TcpTransport::from_stream(stream)?;
        tracing::debug!(%peer, "TCP accepted");
        Ok((transport, peer))
    }

    /// Accept and return the raw [`TcpStream`] (useful for TLS or WebSocket
    /// upgrade before wrapping).
    pub async fn accept_raw(&self) -> crate::Result<(TcpStream, SocketAddr)> {
        let (stream, peer) = self.inner.accept().await?;
        Ok((stream, peer))
    }

    /// The local address this listener is bound to.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

// ---------------------------------------------------------------------------
// WebSocket Listener (requires `websocket` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "websocket")]
pub mod ws {
    use std::net::SocketAddr;

    use tokio::net::TcpListener as TokioTcpListener;
    use tokio_tungstenite::accept_async;

    use crate::websocket::WebSocketTransport;

    /// A WebSocket listener that upgrades accepted TCP connections.
    pub struct WebSocketListener {
        inner: TokioTcpListener,
        local_addr: SocketAddr,
    }

    impl WebSocketListener {
        /// Bind a WebSocket listener to the given address.
        pub async fn bind(addr: SocketAddr) -> crate::Result<Self> {
            let inner = TokioTcpListener::bind(addr).await?;
            let local_addr = inner.local_addr()?;
            tracing::info!(%local_addr, "WebSocket listener bound");
            Ok(Self { inner, local_addr })
        }

        /// Accept and upgrade one inbound WebSocket connection.
        pub async fn accept(&self) -> crate::Result<(WebSocketTransport, SocketAddr)> {
            let (stream, peer) = self.inner.accept().await?;
            let ws_stream = accept_async(tokio_tungstenite::MaybeTlsStream::Plain(stream))
                .await
                .map_err(|e| {
                    crate::TransportError::Protocol(format!("WS upgrade failed: {e}"))
                })?;
            let transport = WebSocketTransport::from_server_stream(ws_stream, peer);
            tracing::debug!(%peer, "WebSocket accepted");
            Ok((transport, peer))
        }

        /// The local address this listener is bound to.
        #[must_use]
        pub fn local_addr(&self) -> SocketAddr {
            self.local_addr
        }
    }
}
