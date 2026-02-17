//! WebSocket transport backend for browser clients and firewall traversal.
//!
//! Uses `tokio-tungstenite` for the WebSocket protocol.  Each LiquiDE message
//! is sent as a single WebSocket binary frame — no additional length-prefix
//! framing is applied.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};

type WsSink = futures_util::stream::SplitSink<
    WebSocketStream<MaybeTlsStream<TcpStream>>,
    Message,
>;
type WsStream = futures_util::stream::SplitStream<
    WebSocketStream<MaybeTlsStream<TcpStream>>,
>;

/// WebSocket transport for browser-based clients.
pub struct WebSocketTransport {
    remote: Option<SocketAddr>,
    local: Option<SocketAddr>,
    sink: Option<Arc<Mutex<WsSink>>>,
    stream: Option<Arc<Mutex<WsStream>>>,
}

impl WebSocketTransport {
    /// Create a new, unconnected WebSocket transport.
    #[must_use]
    pub fn new() -> Self {
        Self {
            remote: None,
            local: None,
            sink: None,
            stream: None,
        }
    }

    /// Wrap an already-connected server-side WebSocket stream.
    pub fn from_server_stream(
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        peer: SocketAddr,
    ) -> Self {
        let (sink, stream) = ws.split();
        Self {
            remote: Some(peer),
            local: None,
            sink: Some(Arc::new(Mutex::new(sink))),
            stream: Some(Arc::new(Mutex::new(stream))),
        }
    }
}

impl Default for WebSocketTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::Transport for WebSocketTransport {
    async fn connect(&mut self, addr: SocketAddr) -> crate::Result<()> {
        let url = format!("ws://{addr}");
        let (ws_stream, _response) = connect_async(&url)
            .await
            .map_err(|e| crate::TransportError::Protocol(format!("WebSocket handshake: {e}")))?;

        let local = match ws_stream.get_ref() {
            MaybeTlsStream::Plain(s) => s.local_addr().ok(),
            _ => None,
        };

        let (sink, stream) = ws_stream.split();
        self.remote = Some(addr);
        self.local = local;
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
