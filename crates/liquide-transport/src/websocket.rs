//! WebSocket transport backend for browser clients and firewall traversal.
//!
//! Uses `tokio-tungstenite` for the WebSocket protocol.  Each LiquiDE message
//! is sent as a single WebSocket binary frame — no additional length-prefix
//! framing is applied.
//!
//! # Security posture
//!
//! "websocket" is a gateway-negotiable transport that carries session data
//! (input, screen tiles, clipboard).  By default it MUST be encrypted: the
//! client connects over `wss://` and verifies the server certificate using a
//! caller-supplied `rustls` client config, exactly like [`TlsTcpTransport`].
//!
//! Plaintext `ws://` is a fail-open downgrade and is only reachable through the
//! explicit, loudly-named [`WebSocketTransport::new_plaintext_insecure`]
//! constructor (and the matching plaintext listener).  There is no code path
//! that silently selects plaintext.
//!
//! [`TlsTcpTransport`]: crate::tls::TlsTcpTransport

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Error as WsError,
    tungstenite::protocol::Message,
};

/// Boxed write half of a split WebSocket stream.
///
/// Boxing as a trait object lets the transport hold either a client-side
/// (`MaybeTlsStream`) or a server-side (`tokio_rustls::server::TlsStream`)
/// connection uniformly — mirroring how [`TlsTcpTransport`](crate::tls::TlsTcpTransport)
/// boxes its reader/writer halves.
type WsSink = Box<dyn Sink<Message, Error = WsError> + Send + Unpin>;
/// Boxed read half of a split WebSocket stream.
type WsStream = Box<dyn Stream<Item = Result<Message, WsError>> + Send + Unpin>;

/// Split a concrete WebSocket stream into boxed sink/stream halves.
fn split_boxed<S>(ws: WebSocketStream<S>) -> (WsSink, WsStream)
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (sink, stream) = ws.split();
    (Box::new(sink), Box::new(stream))
}

/// How a not-yet-connected [`WebSocketTransport`] will dial its peer.
///
/// The default (and only secure) variant is [`Security::Tls`].  Plaintext is
/// an explicit opt-in held in [`Security::PlaintextInsecure`].
enum Security {
    /// Encrypted `wss://` using the given `rustls` client config and the
    /// expected server name (used for SNI / certificate verification).
    #[cfg(feature = "tls")]
    Tls {
        client_config: Arc<rustls::ClientConfig>,
        server_name: String,
    },
    /// Plaintext `ws://`.  **Insecure** — explicit dev/test opt-in only.
    PlaintextInsecure,
}

/// WebSocket transport for browser-based clients.
pub struct WebSocketTransport {
    remote: Option<SocketAddr>,
    local: Option<SocketAddr>,
    sink: Option<Arc<Mutex<WsSink>>>,
    stream: Option<Arc<Mutex<WsStream>>>,
    /// Dial security for an outbound [`connect`](crate::Transport::connect).
    /// `None` for transports created from an already-accepted server stream.
    security: Option<Security>,
}

impl WebSocketTransport {
    /// Create a new, unconnected **secure** (`wss://`) WebSocket transport.
    ///
    /// Mirrors [`TlsTcpTransport::new`](crate::tls::TlsTcpTransport::new): the
    /// `client_config` supplies the trust roots and the `server_name` is used
    /// for SNI and certificate verification.  This is the default, secure
    /// constructor — prefer it everywhere.
    #[cfg(feature = "tls")]
    #[must_use]
    pub fn new(client_config: Arc<rustls::ClientConfig>, server_name: String) -> Self {
        Self {
            remote: None,
            local: None,
            sink: None,
            stream: None,
            security: Some(Security::Tls {
                client_config,
                server_name,
            }),
        }
    }

    /// Create a new, unconnected **plaintext** (`ws://`) WebSocket transport.
    ///
    /// # Security
    ///
    /// This is an **insecure** transport: all carried session traffic travels
    /// in cleartext with no confidentiality or integrity.  It exists solely for
    /// local development and tests (e.g. loopback).  Never use it for traffic
    /// that leaves the host.  Production code must use [`new`](Self::new).
    #[must_use]
    pub fn new_plaintext_insecure() -> Self {
        tracing::warn!(
            "WebSocketTransport: plaintext ws:// selected (INSECURE — dev/test opt-in only)"
        );
        Self {
            remote: None,
            local: None,
            sink: None,
            stream: None,
            security: Some(Security::PlaintextInsecure),
        }
    }

    /// Wrap an already-connected server-side WebSocket stream.
    ///
    /// The stream may be plaintext or TLS-wrapped (`MaybeTlsStream`); the
    /// security of an accepted connection is decided by the listener that
    /// produced it (see [`crate::listener::ws`]).
    pub fn from_server_stream<S>(ws: WebSocketStream<S>, peer: SocketAddr) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let (sink, stream) = split_boxed(ws);
        Self {
            remote: Some(peer),
            local: None,
            sink: Some(Arc::new(Mutex::new(sink))),
            stream: Some(Arc::new(Mutex::new(stream))),
            security: None,
        }
    }

    /// Whether this transport will dial (or was accepted) over plaintext.
    ///
    /// Returns `true` only for transports created via
    /// [`new_plaintext_insecure`](Self::new_plaintext_insecure).
    #[must_use]
    pub fn is_plaintext(&self) -> bool {
        matches!(self.security, Some(Security::PlaintextInsecure))
    }
}

impl crate::Transport for WebSocketTransport {
    async fn connect(&mut self, addr: SocketAddr) -> crate::Result<()> {
        let ws_stream = match self.security.as_ref() {
            #[cfg(feature = "tls")]
            Some(Security::Tls {
                client_config,
                server_name,
            }) => {
                // Connect the TCP socket to the real address, then run the TLS
                // + WebSocket handshake using `server_name` for SNI / cert
                // verification (the URL host, not the dialled IP).  This
                // mirrors `TlsTcpTransport`'s explicit-server-name posture.
                let tcp = TcpStream::connect(addr).await?;
                tcp.set_nodelay(true)?;
                self.local = tcp.local_addr().ok();
                let url = format!("wss://{server_name}/");
                let connector = tokio_tungstenite::Connector::Rustls(Arc::clone(client_config));
                let (ws_stream, _response) = tokio_tungstenite::client_async_tls_with_config(
                    url,
                    tcp,
                    None,
                    Some(connector),
                )
                .await
                .map_err(|e| crate::TransportError::Tls(format!("WebSocket TLS handshake: {e}")))?;
                ws_stream
            }
            Some(Security::PlaintextInsecure) => {
                let url = format!("ws://{addr}");
                let (ws_stream, _response) = connect_async(&url).await.map_err(|e| {
                    crate::TransportError::Protocol(format!("WebSocket handshake: {e}"))
                })?;
                if let MaybeTlsStream::Plain(s) = ws_stream.get_ref() {
                    self.local = s.local_addr().ok();
                }
                ws_stream
            }
            None => {
                return Err(crate::TransportError::Protocol(
                    "WebSocket transport was created from an accepted server stream and \
                     cannot dial out"
                        .into(),
                ));
            }
        };

        let (sink, stream) = split_boxed(ws_stream);
        self.remote = Some(addr);
        self.sink = Some(Arc::new(Mutex::new(sink)));
        self.stream = Some(Arc::new(Mutex::new(stream)));
        tracing::debug!(%addr, "WebSocket connected");
        Ok(())
    }

    async fn send(&self, data: Bytes) -> crate::Result<()> {
        let sink = self
            .sink
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        if data.len() > crate::MAX_MESSAGE_SIZE {
            return Err(crate::TransportError::MessageTooLarge {
                size: data.len(),
                max: crate::MAX_MESSAGE_SIZE,
            });
        }
        let mut s: tokio::sync::MutexGuard<'_, WsSink> = sink.lock().await;
        s.send(Message::Binary(data.to_vec().into()))
            .await
            .map_err(|e| crate::TransportError::Protocol(format!("WS send: {e}")))?;
        Ok(())
    }

    async fn recv(&self) -> crate::Result<Bytes> {
        let stream = self
            .stream
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        let mut s: tokio::sync::MutexGuard<'_, WsStream> = stream.lock().await;
        loop {
            match s.next().await {
                Some(Ok(Message::Binary(data))) => {
                    return Ok(Bytes::from(data.to_vec()));
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                    // Control frames — skip.
                    continue;
                }
                Some(Ok(Message::Close(_))) => {
                    return Err(crate::TransportError::ConnectionReset);
                }
                Some(Ok(_)) => {
                    // Text or other frames — skip non-binary.
                    continue;
                }
                Some(Err(e)) => {
                    return Err(crate::TransportError::Protocol(format!("WS recv: {e}")));
                }
                None => {
                    return Err(crate::TransportError::ConnectionReset);
                }
            }
        }
    }

    async fn close(&mut self) -> crate::Result<()> {
        if let Some(sink) = self.sink.take() {
            // Use lock-based close to work even with other references
            let mut s = sink.lock().await;
            let _ = s.close().await;
        }
        self.stream = None;
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
