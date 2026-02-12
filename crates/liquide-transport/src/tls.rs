//! Optional TLS wrapping for TCP transports using `tokio-rustls`.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use crate::codec;

/// TCP transport wrapped in a TLS layer.
pub struct TlsTcpTransport {
    remote: Option<SocketAddr>,
    local: Option<SocketAddr>,
    reader: Option<Arc<Mutex<tokio::io::ReadHalf<TlsStream<TcpStream>>>>>,
    writer: Option<Arc<Mutex<tokio::io::WriteHalf<TlsStream<TcpStream>>>>>,
    connector: TlsConnector,
    server_name: String,
}

impl TlsTcpTransport {
    /// Create a new TLS TCP transport with the given `rustls` client config
    /// and expected server name (used for SNI / certificate verification).
    #[must_use]
    pub fn new(client_config: Arc<rustls::ClientConfig>, server_name: String) -> Self {
        Self {
            remote: None,
            local: None,
            reader: None,
            writer: None,
            connector: TlsConnector::from(client_config),
            server_name,
        }
    }

    /// Wrap an existing TLS stream.
    pub fn from_stream(
        stream: TlsStream<TcpStream>,
        peer: SocketAddr,
    ) -> crate::Result<Self> {
        let local = stream.get_ref().0.local_addr().ok();
        let (reader, writer) = tokio::io::split(stream);
        Ok(Self {
            remote: Some(peer),
            local,
            reader: Some(Arc::new(Mutex::new(reader))),
            writer: Some(Arc::new(Mutex::new(writer))),
            connector: TlsConnector::from(Arc::new(
                rustls::ClientConfig::builder()
                    .with_root_certificates(rustls::RootCertStore::empty())
                    .with_no_client_auth(),
            )),
            server_name: String::new(),
        })
    }
}

impl crate::Transport for TlsTcpTransport {
    async fn connect(&mut self, addr: SocketAddr) -> crate::Result<()> {
        let tcp = TcpStream::connect(addr).await?;
        tcp.set_nodelay(true)?;
        self.local = tcp.local_addr().ok();

        let domain = rustls::pki_types::ServerName::try_from(self.server_name.clone())
            .map_err(|e| crate::TransportError::Tls(format!("invalid server name: {e}")))?;

        let tls_stream = self
            .connector
            .connect(domain, tcp)
            .await
            .map_err(|e| crate::TransportError::Tls(e.to_string()))?;

        let (reader, writer) = tokio::io::split(tls_stream);
        self.remote = Some(addr);
        self.reader = Some(Arc::new(Mutex::new(reader)));
        self.writer = Some(Arc::new(Mutex::new(writer)));
        tracing::debug!(%addr, "TLS TCP connected");
        Ok(())
    }

    async fn send(&self, data: Bytes) -> crate::Result<()> {
        let writer = self
            .writer
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        if data.len() > crate::MAX_MESSAGE_SIZE {
            return Err(crate::TransportError::MessageTooLarge {
                size: data.len(),
                max: crate::MAX_MESSAGE_SIZE,
            });
        }
        let mut w = writer.lock().await;
        codec::write_msg(&mut *w, &data).await?;
        Ok(())
    }

    async fn recv(&self) -> crate::Result<Bytes> {
        let reader = self
            .reader
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        let mut r = reader.lock().await;
        codec::read_msg(&mut *r, crate::MAX_MESSAGE_SIZE).await
    }

    async fn close(&mut self) -> crate::Result<()> {
        if let Some(writer) = self.writer.take() {
            if let Ok(mut w) = Arc::try_unwrap(writer) {
                let _ = w.get_mut().shutdown().await;
            }
        }
        self.reader = None;
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
