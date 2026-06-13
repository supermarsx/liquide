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
    ///
    /// The listener carries a security mode chosen at bind time.  The default,
    /// secure path is [`bind_tls`](WebSocketListener::bind_tls), which performs
    /// a TLS handshake (`wss://`) before the WebSocket upgrade.  Plaintext
    /// (`ws://`) is only reachable through the explicit, loudly-named
    /// [`bind_plaintext_insecure`](WebSocketListener::bind_plaintext_insecure).
    pub struct WebSocketListener {
        inner: TokioTcpListener,
        local_addr: SocketAddr,
        #[cfg(feature = "tls")]
        acceptor: Option<tokio_rustls::TlsAcceptor>,
        #[cfg(not(feature = "tls"))]
        acceptor: Option<()>,
    }

    impl WebSocketListener {
        /// Bind a **secure** (`wss://`) WebSocket listener.
        ///
        /// Accepted TCP connections are upgraded with TLS using `server_config`
        /// before the WebSocket handshake.  This is the default constructor;
        /// prefer it everywhere.
        #[cfg(feature = "tls")]
        pub async fn bind_tls(
            addr: SocketAddr,
            server_config: std::sync::Arc<rustls::ServerConfig>,
        ) -> crate::Result<Self> {
            let inner = TokioTcpListener::bind(addr).await?;
            let local_addr = inner.local_addr()?;
            let acceptor = Some(tokio_rustls::TlsAcceptor::from(server_config));
            tracing::info!(%local_addr, "WebSocket (wss) listener bound");
            Ok(Self {
                inner,
                local_addr,
                acceptor,
            })
        }

        /// Bind a **plaintext** (`ws://`) WebSocket listener.
        ///
        /// # Security
        ///
        /// Accepted connections carry session traffic in cleartext.  This is an
        /// **insecure** dev/test opt-in (e.g. loopback) and must never be used
        /// for traffic that leaves the host.  Production code must use
        /// [`bind_tls`](Self::bind_tls).
        pub async fn bind_plaintext_insecure(addr: SocketAddr) -> crate::Result<Self> {
            let inner = TokioTcpListener::bind(addr).await?;
            let local_addr = inner.local_addr()?;
            tracing::warn!(
                %local_addr,
                "WebSocket plaintext (ws) listener bound (INSECURE — dev/test opt-in only)"
            );
            Ok(Self {
                inner,
                local_addr,
                acceptor: None,
            })
        }

        /// Accept and upgrade one inbound WebSocket connection.
        ///
        /// If this listener was bound via [`bind_tls`](Self::bind_tls), the TCP
        /// connection is first upgraded to TLS; otherwise it is handled in
        /// plaintext.
        pub async fn accept(&self) -> crate::Result<(WebSocketTransport, SocketAddr)> {
            let (stream, peer) = self.inner.accept().await?;

            #[cfg(feature = "tls")]
            if let Some(acceptor) = &self.acceptor {
                stream.set_nodelay(true)?;
                let tls_stream = acceptor
                    .accept(stream)
                    .await
                    .map_err(|e| crate::TransportError::Tls(e.to_string()))?;
                let ws_stream = accept_async(tls_stream).await.map_err(|e| {
                    crate::TransportError::Protocol(format!("WS upgrade failed: {e}"))
                })?;
                let transport = WebSocketTransport::from_server_stream(ws_stream, peer);
                tracing::debug!(%peer, "WebSocket (wss) accepted");
                return Ok((transport, peer));
            }

            let ws_stream = accept_async(tokio_tungstenite::MaybeTlsStream::Plain(stream))
                .await
                .map_err(|e| crate::TransportError::Protocol(format!("WS upgrade failed: {e}")))?;
            let transport = WebSocketTransport::from_server_stream(ws_stream, peer);
            tracing::debug!(%peer, "WebSocket (plaintext) accepted");
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
        ) -> crate::Result<(
            tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
            SocketAddr,
        )> {
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
            let incoming =
                self.endpoint.accept().await.ok_or_else(|| {
                    crate::TransportError::Protocol("QUIC endpoint closed".into())
                })?;
            let connection = incoming
                .await
                .map_err(|e| crate::TransportError::Protocol(format!("QUIC accept: {e}")))?;
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
