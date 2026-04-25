//! QUIC transport backend using the `quinn` crate.
//!
//! QUIC provides multiplexed, encrypted streams with built-in TLS 1.3.
//! This backend opens a single bidirectional stream per transport instance
//! and applies length-prefixed framing on that stream.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, VarInt};
use tokio::sync::Mutex;

use crate::codec;

/// QUIC-based transport.
pub struct QuicTransport {
    remote: Option<SocketAddr>,
    local: Option<SocketAddr>,
    connection: Option<quinn::Connection>,
    sender: Option<Arc<Mutex<SendStream>>>,
    receiver: Option<Arc<Mutex<RecvStream>>>,
    endpoint: Option<Endpoint>,
}

impl QuicTransport {
    /// Create a new, unconnected QUIC transport.
    #[must_use]
    pub fn new() -> Self {
        Self {
            remote: None,
            local: None,
            connection: None,
            sender: None,
            receiver: None,
            endpoint: None,
        }
    }

    /// Build a QUIC client config that skips server certificate verification.
    ///
    /// # Safety
    /// **DANGEROUS: Only for testing / development.** This config accepts ANY
    /// certificate without validation, making it vulnerable to MITM attacks.
    /// Production deployments MUST supply a proper [`rustls::ClientConfig`]
    /// with certificate verification via [`Self::with_client_config()`].
    #[must_use]
    #[cfg(any(test, feature = "dangerous_insecure_quic"))]
    pub fn insecure_client_config() -> ClientConfig {
        use quinn::TransportConfig;
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipVerification))
            .with_no_client_auth();
        let mut transport = TransportConfig::default();
        transport.max_idle_timeout(Some(
            quinn::IdleTimeout::try_from(std::time::Duration::from_secs(30)).unwrap(),
        ));
        let mut cfg = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap(),
        ));
        cfg.transport_config(Arc::new(transport));
        cfg
    }

    /// Create a QUIC transport with a custom [`ClientConfig`].
    pub fn with_client_config(config: ClientConfig) -> crate::Result<Self> {
        let mut t = Self::new();
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())?;
        endpoint.set_default_client_config(config);
        t.endpoint = Some(endpoint);
        Ok(t)
    }

    /// Wrap an existing QUIC connection (server-side).
    ///
    /// Accepts the first bidirectional stream opened by the remote peer.
    pub async fn from_connection(connection: quinn::Connection) -> crate::Result<Self> {
        let remote = connection.remote_address();
        let (send, recv) = connection.accept_bi().await.map_err(|e| {
            crate::TransportError::Protocol(format!("failed to accept bi stream: {e}"))
        })?;
        Ok(Self {
            remote: Some(remote),
            local: None,
            connection: Some(connection),
            sender: Some(Arc::new(Mutex::new(send))),
            receiver: Some(Arc::new(Mutex::new(recv))),
            endpoint: None,
        })
    }
}

impl Default for QuicTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for QuicTransport {
    fn drop(&mut self) {
        // Best-effort: finish the send stream so quinn sends buffered data
        // with a FIN rather than a RESET_STREAM.
        if let Some(sender) = self.sender.take() {
            if let Ok(mut s) = sender.try_lock() {
                let _ = s.finish();
            }
        }
    }
}

impl crate::Transport for QuicTransport {
    async fn connect(&mut self, addr: SocketAddr) -> crate::Result<()> {
        let endpoint = if let Some(ref ep) = self.endpoint {
            ep.clone()
        } else {
            // Require explicit client config - no insecure default
            return Err(crate::TransportError::Protocol(
                "QUIC transport requires explicit ClientConfig via with_client_config(). \
                 Use QuicTransport::insecure_client_config() only for testing."
                    .to_string(),
            ));
        };

        self.local = endpoint.local_addr().ok();

        let connection = endpoint
            .connect(addr, "liquide")
            .map_err(|e| crate::TransportError::Protocol(e.to_string()))?
            .await
            .map_err(|e| match e {
                quinn::ConnectionError::TimedOut => crate::TransportError::Timeout,
                other => crate::TransportError::Protocol(other.to_string()),
            })?;

        let (send, recv) = connection.open_bi().await.map_err(|e| {
            crate::TransportError::Protocol(format!("failed to open bi stream: {e}"))
        })?;

        self.remote = Some(addr);
        self.connection = Some(connection);
        self.sender = Some(Arc::new(Mutex::new(send)));
        self.receiver = Some(Arc::new(Mutex::new(recv)));
        tracing::debug!(%addr, "QUIC connected");
        Ok(())
    }

    async fn send(&self, data: Bytes) -> crate::Result<()> {
        let sender = self
            .sender
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        if data.len() > crate::MAX_MESSAGE_SIZE {
            return Err(crate::TransportError::MessageTooLarge {
                size: data.len(),
                max: crate::MAX_MESSAGE_SIZE,
            });
        }
        let mut s = sender.lock().await;
        codec::write_msg(&mut *s, &data).await?;
        Ok(())
    }

    async fn recv(&self) -> crate::Result<Bytes> {
        let receiver = self
            .receiver
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        let mut r = receiver.lock().await;
        codec::read_msg(&mut *r, crate::MAX_MESSAGE_SIZE).await
    }

    async fn close(&mut self) -> crate::Result<()> {
        // Gracefully finish the send stream before closing the connection.
        // This ensures buffered data is transmitted with a FIN rather than
        // being discarded by an immediate CONNECTION_CLOSE.
        if let Some(sender) = self.sender.take() {
            let mut s = sender.lock().await;
            let _ = s.finish();
        }
        self.receiver = None;
        if let Some(conn) = self.connection.take() {
            conn.close(VarInt::from_u32(0), b"done");
        }
        self.remote = None;
        if let Some(ep) = self.endpoint.take() {
            ep.close(VarInt::from_u32(0), b"done");
        }
        Ok(())
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        self.remote
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.local
    }
}

// ---------------------------------------------------------------------------
// Certificate verification skip (testing only)
// ---------------------------------------------------------------------------

#[cfg(any(test, feature = "dangerous_insecure_quic"))]
#[derive(Debug)]
struct SkipVerification;

#[cfg(any(test, feature = "dangerous_insecure_quic"))]
impl rustls::client::danger::ServerCertVerifier for SkipVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}
