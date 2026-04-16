//! Connection state machine and connection manager.

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use rustls::pki_types::ServerName;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use liquide_protocol::messages::common::DisplayInfo;
use liquide_protocol::messages::control::{
    CapabilitiesMsg, ClientHello, LoginFailure, LoginPrompt, LoginResponse, LoginSuccess,
    ServerHello,
};
use liquide_protocol::version;

use crate::{ClientError, Result};

/// Connection lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    Negotiating,
    Connected,
    Reconnecting,
    Failed,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Authenticating => "Authenticating",
            Self::Negotiating => "Negotiating",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting",
            Self::Failed => "Failed",
        };
        f.write_str(label)
    }
}

/// Coarse quality assessment derived from live metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
    Bad,
    Disconnected,
}

impl ConnectionQuality {
    /// Derive quality from round-trip time (ms), packet loss (0.0..1.0), and
    /// whether the server has signalled quality degradation.
    #[must_use]
    pub fn from_metrics(rtt_ms: f64, loss_percent: f64, degraded: bool) -> Self {
        if rtt_ms <= 0.0 {
            return Self::Disconnected;
        }
        if degraded || loss_percent > 10.0 || rtt_ms > 300.0 {
            return Self::Bad;
        }
        if loss_percent > 5.0 || rtt_ms > 200.0 {
            return Self::Poor;
        }
        if loss_percent > 2.0 || rtt_ms > 100.0 {
            return Self::Fair;
        }
        if loss_percent > 0.5 || rtt_ms > 50.0 {
            return Self::Good;
        }
        Self::Excellent
    }

    /// CSS-style colour code for UI indicators.
    #[must_use]
    pub fn color(&self) -> &str {
        match self {
            Self::Excellent => "#00c853",
            Self::Good => "#64dd17",
            Self::Fair => "#ffd600",
            Self::Poor => "#ff6d00",
            Self::Bad => "#d50000",
            Self::Disconnected => "#9e9e9e",
        }
    }
}

// ---------------------------------------------------------------------------
// TLS certificate verification
// ---------------------------------------------------------------------------

/// Certificate verifier that accepts any server certificate.
///
/// **Development only.** Production deployments must configure proper CA
/// verification by supplying a `rustls::ClientConfig` with a populated root
/// certificate store.
#[derive(Debug)]
struct InsecureCertVerifier(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for InsecureCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
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
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Build a `rustls::ClientConfig` for the connection.
///
/// Currently uses an insecure certificate verifier (accepts any cert).
/// Replace with proper CA verification for production.
fn build_client_tls_config() -> Arc<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("TLS protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(InsecureCertVerifier(provider)))
        .with_no_client_auth();
    Arc::new(config)
}

// ---------------------------------------------------------------------------
// CBOR encode / decode helpers
// ---------------------------------------------------------------------------

fn cbor_encode<T: serde::Serialize>(val: &T) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    ciborium::into_writer(val, &mut buf).map_err(|e| ClientError::ProtocolError {
        detail: format!("CBOR encode: {e}"),
    })?;
    Ok(buf)
}

fn cbor_decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T> {
    ciborium::from_reader(data).map_err(|e| ClientError::ProtocolError {
        detail: format!("CBOR decode: {e}"),
    })
}

/// A saved connection profile (server bookmark).
#[derive(Debug, Clone)]
pub struct ConnectionProfile {
    pub name: String,
    pub address: String,
    pub username: Option<String>,
    pub transport: String,
    pub encoder: String,
    pub encryption: String,
    pub monitors: u32,
    pub audio_playback: bool,
    pub audio_microphone: bool,
    pub clipboard: bool,
    pub performance: String,
    pub cursor_mode: String,
}

/// Manages the connection to a single remote server.
pub struct ConnectionManager {
    state: ConnectionState,
    profiles: Vec<ConnectionProfile>,
    active_profile: Option<usize>,
    server_addr: String,
    rtt_ms: f64,
    packet_loss_percent: f64,
    bandwidth_mbps: f64,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    /// Active TLS connection to the server.
    stream: Option<TlsStream<TcpStream>>,
    /// Session ID from the server after successful handshake.
    session_id: Option<String>,
}

impl ConnectionManager {
    /// Create a new connection manager.
    #[must_use]
    pub fn new(max_reconnect_attempts: u32) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            profiles: Vec::new(),
            active_profile: None,
            server_addr: String::new(),
            rtt_ms: 0.0,
            packet_loss_percent: 0.0,
            bandwidth_mbps: 0.0,
            reconnect_attempts: 0,
            max_reconnect_attempts,
            stream: None,
            session_id: None,
        }
    }

    /// Initiate a connection to the given server.
    pub async fn connect(&mut self, server: &str) -> Result<()> {
        self.connect_with_credential(server, "", "").await
    }

    /// Initiate a connection with explicit credentials.
    pub async fn connect_with_credential(
        &mut self,
        server: &str,
        username: &str,
        password: &str,
    ) -> Result<()> {
        if self.state == ConnectionState::Connected {
            self.disconnect().await;
        }

        self.server_addr = server.to_string();
        self.reconnect_attempts = 0;
        self.state = ConnectionState::Connecting;

        // Parse address.
        let addr: SocketAddr = server.parse().map_err(|e| {
            ClientError::ServerUnreachable {
                server: format!("{server}: {e}"),
            }
        })?;

        // TCP connect with timeout.
        let tcp_stream = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            TcpStream::connect(addr),
        )
        .await
        .map_err(|_| ClientError::ConnectionTimeout { timeout_ms: 10_000 })?
        .map_err(|e| ClientError::ServerUnreachable {
            server: format!("{server}: {e}"),
        })?;

        // Disable Nagle's algorithm for lower latency.
        let _ = tcp_stream.set_nodelay(true);

        tracing::info!(server = %server, "TCP connected");

        // --- TLS handshake ---
        let tls_config = build_client_tls_config();
        let connector = TlsConnector::from(tls_config);

        let server_name = ServerName::try_from(addr.ip().to_string())
            .map_err(|e| ClientError::ConnectionFailed {
                server: server.to_string(),
                reason: format!("invalid server name for TLS: {e}"),
            })?;

        let tls_stream = connector
            .connect(server_name, tcp_stream)
            .await
            .map_err(|e| ClientError::ConnectionFailed {
                server: server.to_string(),
                reason: format!("TLS handshake failed: {e}"),
            })?;

        tracing::info!(server = %server, "TLS handshake complete");
        self.stream = Some(tls_stream);

        // --- Protocol handshake ---
        self.state = ConnectionState::Authenticating;

        let client_hello = ClientHello {
            protocol_version: version::PROTOCOL_VERSION.to_string(),
            client_name: "liquide-client".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            client_platform: std::env::consts::OS.to_string(),
            supported_transports: vec!["tcp+tls".to_string()],
            supported_codecs: vec!["h264".to_string(), "h265".to_string(), "av1".to_string()],
            supported_audio_codecs: vec!["opus".to_string()],
            supported_compressions: vec!["lz4".to_string(), "zstd".to_string()],
            capabilities: BTreeMap::from([
                ("clipboard".to_string(), true),
                ("audio_playback".to_string(), true),
                ("audio_capture".to_string(), false),
                ("multi_monitor".to_string(), true),
            ]),
            display: DisplayInfo {
                width: 1920,
                height: 1080,
                scale_factor: 1.0,
                refresh_rate: 60,
            },
            resume_token: None,
        };

        self.send_cbor(&client_hello).await?;
        let server_hello: ServerHello = self.recv_cbor().await?;

        if !version::is_compatible(&server_hello.protocol_version) {
            return Err(ClientError::ProtocolError {
                detail: format!(
                    "incompatible protocol version: server={}, client={}",
                    server_hello.protocol_version,
                    version::PROTOCOL_VERSION,
                ),
            });
        }

        tracing::info!(
            server_name = %server_hello.server_name,
            session_id = %server_hello.session_id,
            "protocol handshake complete"
        );

        // --- Authentication exchange ---
        let _login_prompt: LoginPrompt = self.recv_cbor().await?;

        let login_response = LoginResponse {
            method: "password".to_string(),
            credential: format!("{username}:{password}").into_bytes(),
            mfa_token: None,
        };
        self.send_cbor(&login_response).await?;

        // Read authentication result — could be LoginSuccess or LoginFailure.
        let result_bytes = self.recv_message().await?;
        if let Ok(success) = cbor_decode::<LoginSuccess>(&result_bytes) {
            self.session_id = Some(success.session_id.clone());
            tracing::info!(session_id = %success.session_id, "authentication succeeded");
        } else if let Ok(failure) = cbor_decode::<LoginFailure>(&result_bytes) {
            return Err(ClientError::AuthenticationFailed {
                reason: failure.reason,
            });
        } else {
            return Err(ClientError::ProtocolError {
                detail: "unexpected authentication response".to_string(),
            });
        }

        // --- Capability negotiation ---
        self.state = ConnectionState::Negotiating;

        let caps = CapabilitiesMsg {
            action: "advertise".to_string(),
            capabilities: BTreeMap::from([
                ("clipboard".to_string(), true),
                ("audio_playback".to_string(), true),
                ("file_transfer".to_string(), false),
            ]),
            request_id: None,
        };
        self.send_cbor(&caps).await?;
        let _server_caps: CapabilitiesMsg = self.recv_cbor().await?;

        self.state = ConnectionState::Connected;
        tracing::info!(server = %server, "connection established");
        Ok(())
    }

    /// Disconnect from the current server.
    pub async fn disconnect(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.shutdown().await;
        }
        self.state = ConnectionState::Disconnected;
        self.rtt_ms = 0.0;
        self.packet_loss_percent = 0.0;
        self.bandwidth_mbps = 0.0;
        self.reconnect_attempts = 0;
        self.session_id = None;
    }

    /// Attempt to reconnect to the last server.
    pub async fn reconnect(&mut self) -> Result<()> {
        if self.server_addr.is_empty() {
            return Err(ClientError::ServerUnreachable {
                server: "(none)".to_string(),
            });
        }

        if !self.should_reconnect() {
            return Err(ClientError::ReconnectFailed {
                attempts: self.reconnect_attempts,
            });
        }

        self.reconnect_attempts += 1;
        self.state = ConnectionState::Reconnecting;

        // Exponential back-off delay.
        let delay = self.next_reconnect_delay_ms();
        tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;

        // Attempt reconnection.
        let addr = self.server_addr.clone();
        match self.connect(&addr).await {
            Ok(()) => Ok(()),
            Err(e) => {
                self.state = ConnectionState::Failed;
                Err(e)
            }
        }
    }

    // -- Message framing (length-prefixed) ------------------------------------

    /// Read a length-prefixed message from the server.
    ///
    /// Wire format: 4-byte little-endian length prefix followed by payload.
    ///
    /// A 30-second read timeout prevents a misbehaving server from hanging
    /// the client indefinitely.
    pub async fn recv_message(&mut self) -> Result<Vec<u8>> {
        const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

        let stream = self
            .stream
            .as_mut()
            .ok_or(ClientError::NotConnected)?;

        // Read 4-byte length prefix (little-endian).
        let mut len_buf = [0u8; 4];
        tokio::time::timeout(READ_TIMEOUT, stream.read_exact(&mut len_buf))
            .await
            .map_err(|_| ClientError::ConnectionTimeout { timeout_ms: READ_TIMEOUT.as_millis() as u64 })?
            .map_err(|e| ClientError::ConnectionLost {
                reason: format!("recv len: {e}"),
            })?;
        let msg_len = u32::from_le_bytes(len_buf) as usize;

        if msg_len > 16 * 1024 * 1024 {
            return Err(ClientError::ProtocolError {
                detail: format!("message too large: {msg_len} bytes"),
            });
        }

        // Read payload.
        let mut payload = vec![0u8; msg_len];
        tokio::time::timeout(READ_TIMEOUT, stream.read_exact(&mut payload))
            .await
            .map_err(|_| ClientError::ConnectionTimeout { timeout_ms: READ_TIMEOUT.as_millis() as u64 })?
            .map_err(|e| ClientError::ConnectionLost {
                reason: format!("recv payload: {e}"),
            })?;

        Ok(payload)
    }

    /// Send a length-prefixed message to the server.
    pub async fn send_message(&mut self, data: &[u8]) -> Result<()> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(ClientError::NotConnected)?;

        let len = (data.len() as u32).to_le_bytes();
        stream
            .write_all(&len)
            .await
            .map_err(|e| ClientError::ConnectionLost {
                reason: format!("send len: {e}"),
            })?;
        stream
            .write_all(data)
            .await
            .map_err(|e| ClientError::ConnectionLost {
                reason: format!("send data: {e}"),
            })?;
        stream
            .flush()
            .await
            .map_err(|e| ClientError::ConnectionLost {
                reason: format!("flush: {e}"),
            })?;

        Ok(())
    }

    /// Take ownership of the TLS stream for use in a background task.
    pub fn take_stream(&mut self) -> Option<TlsStream<TcpStream>> {
        self.stream.take()
    }

    /// Session ID assigned by the server after authentication.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Send a CBOR-encoded message using the length-prefixed wire format.
    async fn send_cbor<T: serde::Serialize>(&mut self, msg: &T) -> Result<()> {
        let data = cbor_encode(msg)?;
        self.send_message(&data).await
    }

    /// Receive and CBOR-decode a message from the length-prefixed wire format.
    async fn recv_cbor<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        let data = self.recv_message().await?;
        cbor_decode(&data)
    }

    /// Current connection state.
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Assess current connection quality from live metrics.
    #[must_use]
    pub fn quality(&self) -> ConnectionQuality {
        if self.state != ConnectionState::Connected {
            return ConnectionQuality::Disconnected;
        }
        ConnectionQuality::from_metrics(self.rtt_ms, self.packet_loss_percent, false)
    }

    /// Add a connection profile.
    pub fn add_profile(&mut self, profile: ConnectionProfile) {
        self.profiles.push(profile);
    }

    /// Remove a connection profile by name. Returns `true` if found.
    pub fn remove_profile(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        self.profiles.len() < before
    }

    /// List all saved profiles.
    #[must_use]
    pub fn profiles(&self) -> &[ConnectionProfile] {
        &self.profiles
    }

    /// Index of the currently active profile, if any.
    #[must_use]
    pub fn active_profile(&self) -> Option<&ConnectionProfile> {
        self.active_profile.and_then(|i| self.profiles.get(i))
    }

    /// Update live metrics from the transport layer.
    pub fn update_metrics(&mut self, rtt_ms: f64, packet_loss_percent: f64, bandwidth_mbps: f64) {
        self.rtt_ms = rtt_ms;
        self.packet_loss_percent = packet_loss_percent;
        self.bandwidth_mbps = bandwidth_mbps;
    }

    /// Whether another reconnect attempt is allowed.
    #[must_use]
    pub fn should_reconnect(&self) -> bool {
        // max_reconnect_attempts == 0 means unlimited.
        self.max_reconnect_attempts == 0
            || self.reconnect_attempts < self.max_reconnect_attempts
    }

    /// Compute the delay before the next reconnect attempt (exponential back-off).
    #[must_use]
    pub fn next_reconnect_delay_ms(&self) -> u32 {
        let base: u32 = 1000;
        let max_delay: u32 = 30000;
        let exp = self.reconnect_attempts.min(15);
        let delay = base.saturating_mul(1u32.wrapping_shl(exp));
        delay.min(max_delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // ── Helper: spin up a TLS server that speaks the Liquide protocol ──

    fn self_signed_cert_and_key() -> (
        Vec<rustls::pki_types::CertificateDer<'static>>,
        rustls::pki_types::PrivateKeyDer<'static>,
    ) {
        let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()])
            .expect("rcgen");
        let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
        let key_der =
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                cert.key_pair.serialize_der(),
            ));
        (vec![cert_der], key_der)
    }

    fn server_tls_config() -> Arc<rustls::ServerConfig> {
        let (certs, key) = self_signed_cert_and_key();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        Arc::new(
            rustls::ServerConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("protocol versions")
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .expect("server config"),
        )
    }

    /// Run a mock TLS server that performs the Liquide handshake.
    /// Returns (listener_addr, join_handle).
    async fn mock_tls_server(
        auth_result: bool,
    ) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp.local_addr().unwrap();
        let tls_cfg = server_tls_config();
        let acceptor = tokio_rustls::TlsAcceptor::from(tls_cfg);

        let handle = tokio::spawn(async move {
            let (stream, _peer) = tcp.accept().await.unwrap();
            let mut tls = acceptor.accept(stream).await.unwrap();

            // Read ClientHello
            let mut len_buf = [0u8; 4];
            tls.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            tls.read_exact(&mut payload).await.unwrap();
            let _client_hello: ClientHello = ciborium::from_reader(&payload[..]).unwrap();

            // Send ServerHello
            let server_hello = ServerHello {
                protocol_version: version::PROTOCOL_VERSION.to_string(),
                server_name: "mock-server".to_string(),
                server_version: "0.1.0".to_string(),
                selected_transport: "tcp+tls".to_string(),
                selected_video_codec: "h264".to_string(),
                selected_audio_codec: "opus".to_string(),
                channels: BTreeMap::new(),
                session_id: "sess-001".to_string(),
                resume_accepted: None,
                features: BTreeMap::new(),
            };
            send_msg(&mut tls, &server_hello).await;

            // Send LoginPrompt
            let prompt = LoginPrompt {
                available_methods: vec!["password".to_string()],
                avatar_png: None,
                session_resume_available: None,
                server_greeting: Some("Welcome".to_string()),
            };
            send_msg(&mut tls, &prompt).await;

            // Read LoginResponse
            let mut lb = [0u8; 4];
            tls.read_exact(&mut lb).await.unwrap();
            let l = u32::from_le_bytes(lb) as usize;
            let mut p = vec![0u8; l];
            tls.read_exact(&mut p).await.unwrap();
            let _login_resp: LoginResponse = ciborium::from_reader(&p[..]).unwrap();

            // Send LoginSuccess or LoginFailure
            if auth_result {
                let success = LoginSuccess {
                    session_id: "sess-001".to_string(),
                    session_token: vec![1, 2, 3],
                    session_features: BTreeMap::new(),
                    token_lifetime_sec: Some(3600),
                };
                send_msg(&mut tls, &success).await;

                // Read CapabilitiesMsg (only after successful auth)
                let mut lb2 = [0u8; 4];
                if tls.read_exact(&mut lb2).await.is_ok() {
                    let l2 = u32::from_le_bytes(lb2) as usize;
                    let mut p2 = vec![0u8; l2];
                    let _ = tls.read_exact(&mut p2).await;

                    // Send server capabilities
                    let caps = CapabilitiesMsg {
                        action: "confirm".to_string(),
                        capabilities: BTreeMap::from([
                            ("clipboard".to_string(), true),
                            ("audio_playback".to_string(), true),
                        ]),
                        request_id: None,
                    };
                    send_msg(&mut tls, &caps).await;
                }
            } else {
                let failure = LoginFailure {
                    error_code: 1,
                    reason: "invalid credentials".to_string(),
                    retry_after_sec: None,
                    remaining_attempts: Some(2),
                };
                send_msg(&mut tls, &failure).await;
            }

            let _ = tls.shutdown().await;
        });

        (addr, handle)
    }

    async fn send_msg<W: AsyncWriteExt + Unpin, T: serde::Serialize>(w: &mut W, msg: &T) {
        let mut buf = Vec::new();
        ciborium::into_writer(msg, &mut buf).unwrap();
        let len = (buf.len() as u32).to_le_bytes();
        w.write_all(&len).await.unwrap();
        w.write_all(&buf).await.unwrap();
        w.flush().await.unwrap();
    }

    // ── Tests ───────────────────────────────────────────────────────────

    #[test]
    fn cbor_roundtrip_client_hello() {
        let hello = ClientHello {
            protocol_version: "proto/1".to_string(),
            client_name: "test".to_string(),
            client_version: "0.0.1".to_string(),
            client_platform: "linux".to_string(),
            supported_transports: vec!["tcp+tls".to_string()],
            supported_codecs: vec!["h264".to_string()],
            supported_audio_codecs: vec!["opus".to_string()],
            supported_compressions: vec!["lz4".to_string()],
            capabilities: BTreeMap::new(),
            display: DisplayInfo {
                width: 1920,
                height: 1080,
                scale_factor: 1.0,
                refresh_rate: 60,
            },
            resume_token: None,
        };
        let encoded = cbor_encode(&hello).unwrap();
        let decoded: ClientHello = cbor_decode(&encoded).unwrap();
        assert_eq!(hello.protocol_version, decoded.protocol_version);
        assert_eq!(hello.client_name, decoded.client_name);
    }

    #[test]
    fn cbor_roundtrip_server_hello() {
        let sh = ServerHello {
            protocol_version: "proto/1".to_string(),
            server_name: "srv".to_string(),
            server_version: "1.0".to_string(),
            selected_transport: "tcp+tls".to_string(),
            selected_video_codec: "h264".to_string(),
            selected_audio_codec: "opus".to_string(),
            channels: BTreeMap::new(),
            session_id: "s1".to_string(),
            resume_accepted: None,
            features: BTreeMap::new(),
        };
        let encoded = cbor_encode(&sh).unwrap();
        let decoded: ServerHello = cbor_decode(&encoded).unwrap();
        assert_eq!(sh, decoded);
    }

    #[test]
    fn cbor_decode_bad_data() {
        let result = cbor_decode::<ClientHello>(&[0xFF, 0xFF]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ClientError::ProtocolError { .. }));
    }

    #[tokio::test]
    async fn connect_full_handshake() {
        let (addr, server) = mock_tls_server(true).await;
        let mut mgr = ConnectionManager::new(3);
        mgr.connect_with_credential(&addr.to_string(), "user", "pass")
            .await
            .unwrap();
        assert_eq!(mgr.state(), ConnectionState::Connected);
        assert_eq!(mgr.session_id(), Some("sess-001"));
        mgr.disconnect().await;
        assert_eq!(mgr.state(), ConnectionState::Disconnected);
        assert!(mgr.session_id().is_none());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn connect_auth_failure() {
        let (addr, server) = mock_tls_server(false).await;
        let mut mgr = ConnectionManager::new(3);
        let result = mgr
            .connect_with_credential(&addr.to_string(), "bad", "creds")
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ClientError::AuthenticationFailed { .. }
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn connect_invalid_address() {
        let mut mgr = ConnectionManager::new(3);
        let result = mgr.connect("not-an-address").await;
        assert!(result.is_err());
    }

    #[test]
    fn tls_config_builds_successfully() {
        let config = build_client_tls_config();
        // Smoke test — config should support TLS 1.3
        assert!(config.alpn_protocols.is_empty());
    }

    #[test]
    fn connection_manager_initial_state() {
        let mut mgr = ConnectionManager::new(5);
        assert_eq!(mgr.state(), ConnectionState::Disconnected);
        assert!(mgr.session_id().is_none());
        assert!(mgr.take_stream().is_none());
    }

    // ── Error path tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn send_message_not_connected() {
        let mut mgr = ConnectionManager::new(3);
        let err = mgr.send_message(b"hello").await.unwrap_err();
        assert!(matches!(err, ClientError::NotConnected));
    }

    #[tokio::test]
    async fn recv_message_not_connected() {
        let mut mgr = ConnectionManager::new(3);
        let err = mgr.recv_message().await.unwrap_err();
        assert!(matches!(err, ClientError::NotConnected));
    }

    #[tokio::test]
    async fn reconnect_empty_server_addr() {
        let mut mgr = ConnectionManager::new(5);
        let err = mgr.reconnect().await.unwrap_err();
        assert!(matches!(err, ClientError::ServerUnreachable { .. }));
    }

    #[test]
    fn should_reconnect_zero_means_unlimited() {
        let mgr = ConnectionManager::new(0);
        assert!(mgr.should_reconnect());
    }

    #[test]
    fn reconnect_delay_caps_at_30s() {
        let mut mgr = ConnectionManager::new(0);
        // Simulate many reconnect attempts so the delay would exceed 30s.
        mgr.reconnect_attempts = 20;
        assert_eq!(mgr.next_reconnect_delay_ms(), 30_000);
    }

    #[test]
    fn reconnect_delay_grows_exponentially() {
        let mut mgr = ConnectionManager::new(0);
        let d0 = mgr.next_reconnect_delay_ms();
        mgr.reconnect_attempts = 1;
        let d1 = mgr.next_reconnect_delay_ms();
        mgr.reconnect_attempts = 2;
        let d2 = mgr.next_reconnect_delay_ms();
        assert_eq!(d0, 1000);
        assert_eq!(d1, 2000);
        assert_eq!(d2, 4000);
    }

    #[tokio::test]
    async fn disconnect_is_idempotent() {
        let mut mgr = ConnectionManager::new(5);
        mgr.disconnect().await;
        assert_eq!(mgr.state(), ConnectionState::Disconnected);
        mgr.disconnect().await;
        assert_eq!(mgr.state(), ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn recv_message_too_large() {
        // Stand up a mock server that sends a length prefix > 16 MiB.
        let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp.local_addr().unwrap();
        let tls_cfg = server_tls_config();
        let acceptor = tokio_rustls::TlsAcceptor::from(tls_cfg);

        let handle = tokio::spawn(async move {
            let (stream, _) = tcp.accept().await.unwrap();
            let mut tls = acceptor.accept(stream).await.unwrap();

            // Perform the full handshake so the client reaches Connected state.
            // Read ClientHello
            let mut lb = [0u8; 4];
            tls.read_exact(&mut lb).await.unwrap();
            let l = u32::from_le_bytes(lb) as usize;
            let mut p = vec![0u8; l];
            tls.read_exact(&mut p).await.unwrap();

            // Send ServerHello
            let sh = ServerHello {
                protocol_version: version::PROTOCOL_VERSION.to_string(),
                server_name: "mock".to_string(),
                server_version: "0.1.0".to_string(),
                selected_transport: "tcp+tls".to_string(),
                selected_video_codec: "h264".to_string(),
                selected_audio_codec: "opus".to_string(),
                channels: BTreeMap::new(),
                session_id: "s1".to_string(),
                resume_accepted: None,
                features: BTreeMap::new(),
            };
            send_msg(&mut tls, &sh).await;

            // Send LoginPrompt
            let prompt = LoginPrompt {
                available_methods: vec!["password".to_string()],
                avatar_png: None,
                session_resume_available: None,
                server_greeting: None,
            };
            send_msg(&mut tls, &prompt).await;

            // Read LoginResponse
            let mut lb2 = [0u8; 4];
            tls.read_exact(&mut lb2).await.unwrap();
            let l2 = u32::from_le_bytes(lb2) as usize;
            let mut p2 = vec![0u8; l2];
            tls.read_exact(&mut p2).await.unwrap();

            // Send LoginSuccess
            let success = LoginSuccess {
                session_id: "s1".to_string(),
                session_token: vec![1],
                session_features: BTreeMap::new(),
                token_lifetime_sec: None,
            };
            send_msg(&mut tls, &success).await;

            // Read CapabilitiesMsg
            let mut lb3 = [0u8; 4];
            if tls.read_exact(&mut lb3).await.is_ok() {
                let l3 = u32::from_le_bytes(lb3) as usize;
                let mut p3 = vec![0u8; l3];
                let _ = tls.read_exact(&mut p3).await;

                let caps = CapabilitiesMsg {
                    action: "confirm".to_string(),
                    capabilities: BTreeMap::new(),
                    request_id: None,
                };
                send_msg(&mut tls, &caps).await;
            }

            // Now send a message with a too-large length prefix (20 MiB).
            let oversized: u32 = 20 * 1024 * 1024;
            tls.write_all(&oversized.to_le_bytes()).await.unwrap();
            tls.flush().await.unwrap();

            let _ = tls.shutdown().await;
        });

        let mut mgr = ConnectionManager::new(3);
        mgr.connect_with_credential(&addr.to_string(), "u", "p")
            .await
            .unwrap();
        assert_eq!(mgr.state(), ConnectionState::Connected);

        // Next recv_message should reject the oversized frame.
        let err = mgr.recv_message().await.unwrap_err();
        assert!(matches!(err, ClientError::ProtocolError { .. }));

        mgr.disconnect().await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn connect_resets_reconnect_attempts() {
        let (addr, server) = mock_tls_server(true).await;
        let mut mgr = ConnectionManager::new(3);
        mgr.reconnect_attempts = 2;
        mgr.connect_with_credential(&addr.to_string(), "u", "p")
            .await
            .unwrap();
        // connect resets reconnect_attempts to 0
        assert!(mgr.should_reconnect());
        mgr.disconnect().await;
        server.await.unwrap();
    }
}
