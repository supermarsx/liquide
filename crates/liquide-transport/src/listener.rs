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

// ---------------------------------------------------------------------------
// TLS Listener (requires `tls` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "tls")]
pub mod tls {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use tokio::net::TcpListener as TokioTcpListener;
    use tokio_rustls::TlsAcceptor;

    use crate::tls::TlsTcpTransport;

    /// A TLS listener that accepts TCP connections and upgrades them with TLS.
    pub struct TlsListener {
        inner: TokioTcpListener,
        acceptor: TlsAcceptor,
        local_addr: SocketAddr,
    }

    impl TlsListener {
        /// Bind a TLS listener to the given address with the given server
        /// configuration.
        pub async fn bind(
            addr: SocketAddr,
            server_config: Arc<rustls::ServerConfig>,
        ) -> crate::Result<Self> {
            let inner = TokioTcpListener::bind(addr).await?;
            let local_addr = inner.local_addr()?;
            let acceptor = TlsAcceptor::from(server_config);
            tracing::info!(%local_addr, "TLS listener bound");
            Ok(Self {
                inner,
                acceptor,
                local_addr,
            })
        }

        /// Accept one inbound TLS connection.
        ///
        /// The TCP connection is accepted and then upgraded via the TLS
        /// handshake. Returns a [`TlsTcpTransport`] wrapping the server-side
        /// TLS stream.
        pub async fn accept(&self) -> crate::Result<(TlsTcpTransport, SocketAddr)> {
            let (stream, peer) = self.inner.accept().await?;
            stream.set_nodelay(true)?;
            let tls_stream = self
                .acceptor
                .accept(stream)
                .await
                .map_err(|e| crate::TransportError::Tls(e.to_string()))?;
            let transport = TlsTcpTransport::from_server_stream(tls_stream, peer)?;
            tracing::debug!(%peer, "TLS accepted");
            Ok((transport, peer))
        }

        /// Accept and return the raw server-side TLS stream.
        pub async fn accept_raw(
            &self,
        ) -> crate::Result<(tokio_rustls::server::TlsStream<tokio::net::TcpStream>, SocketAddr)>
        {
            let (stream, peer) = self.inner.accept().await?;
            stream.set_nodelay(true)?;
            let tls_stream = self
                .acceptor
                .accept(stream)
                .await
                .map_err(|e| crate::TransportError::Tls(e.to_string()))?;
            Ok((tls_stream, peer))
        }

        /// The local address this listener is bound to.
        #[must_use]
        pub fn local_addr(&self) -> SocketAddr {
            self.local_addr
        }
    }
}

// ---------------------------------------------------------------------------
// QUIC Listener (requires `quic` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "quic")]
pub mod quic {
    use std::net::SocketAddr;

    use crate::quic::QuicTransport;

    /// A QUIC listener that accepts inbound QUIC connections.
    ///
    /// Each accepted connection upgrades to a [`QuicTransport`] by accepting
    /// the first bidirectional stream from the remote peer.
    pub struct QuicListener {
        endpoint: quinn::Endpoint,
        local_addr: SocketAddr,
    }

    impl QuicListener {
        /// Bind a QUIC listener with the given server configuration.
        pub async fn bind(
            addr: SocketAddr,
            server_config: quinn::ServerConfig,
        ) -> crate::Result<Self> {
            let endpoint = quinn::Endpoint::server(server_config, addr)?;
            let local_addr = endpoint.local_addr()?;
            tracing::info!(%local_addr, "QUIC listener bound");
            Ok(Self {
                endpoint,
                local_addr,
            })
        }

        /// Accept one inbound QUIC connection.
        ///
        /// Waits for a new QUIC connection and accepts the first bidirectional
        /// stream opened by the peer.
        pub async fn accept(&self) -> crate::Result<(QuicTransport, SocketAddr)> {
            let incoming = self
                .endpoint
                .accept()
                .await
                .ok_or_else(|| crate::TransportError::Protocol("QUIC endpoint closed".into()))?;
            let connection = incoming.await.map_err(|e| {
                crate::TransportError::Protocol(format!("QUIC accept: {e}"))
            })?;
            let remote = connection.remote_address();
            let transport = QuicTransport::from_connection(connection).await?;
            tracing::debug!(%remote, "QUIC accepted");
            Ok((transport, remote))
        }

        /// The local address this listener is bound to.
        #[must_use]
        pub fn local_addr(&self) -> SocketAddr {
            self.local_addr
        }

        /// Access the underlying [`quinn::Endpoint`].
        #[must_use]
        pub fn endpoint(&self) -> &quinn::Endpoint {
            &self.endpoint
        }
    }
}
