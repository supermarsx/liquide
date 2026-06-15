//! End-to-end remote-first integration tests for the gateway binary path.
//!
//! These exercise the real wiring covered by remote-shell audit TODOs 1-3 and 5:
//! TLS termination (1), post-login stream relay to a backend (2), backend
//! registration so routing has a target (3), and session-resume tokens (5).
//!
//! They use the gateway's public library API the way `main.rs` does
//! (`set_tls_config` + `handle_server_registration` + `handle_tcp_connection`)
//! against a real backend TCP server, and drive a real TLS client.

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use liquide_gateway::{
    ClusterConfig, GatewayConfig, GatewayRuntime, HealthCheckConfig, LimitsConfig,
    ManagementApiConfig, RelayConfig, RoutingConfig, ServerCapabilities, ServerHealth,
};
use liquide_protocol::messages::common::DisplayInfo;
use liquide_protocol::messages::control::{
    build_resume_token, CapabilitiesMsg, ClientHello, LoginPrompt, LoginResponse, LoginSuccess,
    ServerHello,
};
use liquide_protocol::version;

// --- TLS helpers -----------------------------------------------------------

fn self_signed() -> (
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
) {
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
    );
    (vec![cert_der], key_der)
}

fn server_tls() -> Arc<rustls::ServerConfig> {
    let (certs, key) = self_signed();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(
        rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap(),
    )
}

#[derive(Debug)]
struct InsecureVerifier(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn client_tls() -> Arc<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(
        rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureVerifier(provider)))
            .with_no_client_auth(),
    )
}

// --- Length-prefixed CBOR helpers (matching the gateway wire framing) ------

async fn send<W: AsyncWriteExt + Unpin, T: serde::Serialize>(w: &mut W, msg: &T) {
    let mut buf = Vec::new();
    ciborium::into_writer(msg, &mut buf).unwrap();
    w.write_all(&(buf.len() as u32).to_le_bytes()).await.unwrap();
    w.write_all(&buf).await.unwrap();
    w.flush().await.unwrap();
}

async fn recv<R: AsyncReadExt + Unpin, T: serde::de::DeserializeOwned>(r: &mut R) -> T {
    let mut lb = [0u8; 4];
    r.read_exact(&mut lb).await.unwrap();
    let len = u32::from_le_bytes(lb) as usize;
    let mut p = vec![0u8; len];
    r.read_exact(&mut p).await.unwrap();
    ciborium::from_reader(&p[..]).unwrap()
}

fn make_runtime_with_apikey(api_key: &str) -> GatewayRuntime {
    let mgmt = ManagementApiConfig {
        api_key: api_key.to_string(),
        ..ManagementApiConfig::default()
    };
    GatewayRuntime::new(
        GatewayConfig::default(),
        RoutingConfig::default(),
        RelayConfig::default(),
        LimitsConfig::default(),
        HealthCheckConfig::default(),
        mgmt,
        ClusterConfig::default(),
    )
}

fn client_hello(resume_token: Option<Vec<u8>>) -> ClientHello {
    ClientHello {
        protocol_version: version::PROTOCOL_VERSION.to_string(),
        client_name: "e2e-client".to_string(),
        client_version: "0.1.0".to_string(),
        client_platform: "test".to_string(),
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
        resume_token,
    }
}

/// Drive a full client handshake over a fresh TLS connection to `gw_addr`,
/// authenticating with the api-key method. Returns the established TLS stream
/// plus the `ServerHello` and `LoginSuccess`.
async fn client_handshake(
    gw_addr: std::net::SocketAddr,
    api_key: &str,
    resume_token: Option<Vec<u8>>,
) -> (
    tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
    ServerHello,
    LoginSuccess,
) {
    let tcp = tokio::net::TcpStream::connect(gw_addr).await.unwrap();
    let connector = tokio_rustls::TlsConnector::from(client_tls());
    let name = rustls::pki_types::ServerName::try_from("127.0.0.1".to_string()).unwrap();
    let mut tls = connector.connect(name, tcp).await.unwrap();

    send(&mut tls, &client_hello(resume_token)).await;
    let sh: ServerHello = recv(&mut tls).await;
    let _prompt: LoginPrompt = recv(&mut tls).await;

    send(
        &mut tls,
        &LoginResponse {
            method: "apikey".to_string(),
            credential: api_key.as_bytes().to_vec(),
            mfa_token: None,
        },
    )
    .await;

    let success: LoginSuccess = recv(&mut tls).await;

    // Capability negotiation (advertise then read confirm).
    send(
        &mut tls,
        &CapabilitiesMsg {
            action: "advertise".to_string(),
            capabilities: BTreeMap::new(),
            request_id: None,
        },
    )
    .await;
    let _confirm: CapabilitiesMsg = recv(&mut tls).await;

    (tls, sh, success)
}

/// TODO 1+2+3: TLS handshake completes, a registered backend exists, and after
/// login the gateway relays bytes between the client and the backend (proving
/// the authenticated stream stays alive and frames/input survive the login).
#[tokio::test]
async fn tls_login_then_relay_to_backend_echo() {
    let api_key = "test-api-key-1234";

    // 1. Backend: a real TCP echo server that the relay connects to.
    let backend = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend_addr = backend.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut sock, _) = backend.accept().await.unwrap();
        let mut buf = vec![0u8; 1024];
        loop {
            match sock.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if sock.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                    let _ = sock.flush().await;
                }
            }
        }
    });

    // 2. Gateway: TLS configured + one healthy backend registered.
    let mut rt = make_runtime_with_apikey(api_key);
    rt.set_tls_config(server_tls());
    let server_id = rt
        .handle_server_registration(backend_addr.to_string(), ServerCapabilities::default(), 1)
        .unwrap();
    rt.server_registry_mut()
        .update_health(&server_id, ServerHealth::Healthy);

    // Gateway client-facing listener + accept loop in a task.
    let gw_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gw_addr = gw_listener.local_addr().unwrap();
    tokio::spawn(async move {
        // Handle a single client connection for this test.
        let (stream, peer) = gw_listener.accept().await.unwrap();
        rt.handle_tcp_connection(stream, peer).await;
        // Keep rt (and its spawned relay task) alive for the test duration.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(rt);
    });

    // 3. Client: full handshake, then exercise the post-login relay path.
    let (mut tls, sh, success) = client_handshake(gw_addr, api_key, None).await;
    assert_eq!(sh.protocol_version, version::PROTOCOL_VERSION);
    assert!(!success.session_id.is_empty());
    assert!(
        !success.session_token.is_empty(),
        "server must issue a session token"
    );

    // After login the stream is spliced to the backend echo server: bytes we
    // write must come back, proving the authenticated stream survives login.
    let payload = b"frame-after-login";
    tls.write_all(payload).await.unwrap();
    tls.flush().await.unwrap();

    let mut echoed = vec![0u8; payload.len()];
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tls.read_exact(&mut echoed),
    )
    .await
    .expect("relay must forward bytes after login")
    .expect("read echoed bytes");
    assert_eq!(&echoed, payload, "relayed bytes must round-trip");
}

/// TODO 5: a client that presents a valid resume token (session id + the
/// secret token from a prior `LoginSuccess`) is accepted for resume and keeps
/// its prior session id; an invalid token is rejected.
#[tokio::test]
async fn resume_token_is_validated_end_to_end() {
    let api_key = "resume-key-9999";

    // Backend echo server (relay target).
    let backend = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend_addr = backend.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = backend.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 256];
                while let Ok(n) = sock.read(&mut buf).await {
                    if n == 0 || sock.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });

    let mut rt = make_runtime_with_apikey(api_key);
    rt.set_tls_config(server_tls());
    let server_id = rt
        .handle_server_registration(backend_addr.to_string(), ServerCapabilities::default(), 1)
        .unwrap();
    rt.server_registry_mut()
        .update_health(&server_id, ServerHealth::Healthy);

    let gw_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gw_addr = gw_listener.local_addr().unwrap();
    tokio::spawn(async move {
        for _ in 0..3 {
            let (stream, peer) = gw_listener.accept().await.unwrap();
            rt.handle_tcp_connection(stream, peer).await;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        drop(rt);
    });

    // First login: no resume token -> fresh session, server_hello.resume_accepted == None.
    let (mut tls1, sh1, success1) = client_handshake(gw_addr, api_key, None).await;
    assert_eq!(sh1.resume_accepted, None);
    let session_id = success1.session_id.clone();
    let token = success1.session_token.clone();
    drop(tls1.shutdown());

    // Second login: valid resume token -> accepted, prior session id preserved.
    let good = build_resume_token(&session_id, &token);
    let (_tls2, sh2, _success2) = client_handshake(gw_addr, api_key, Some(good)).await;
    assert_eq!(
        sh2.resume_accepted,
        Some(true),
        "valid resume token must be accepted"
    );
    assert_eq!(
        sh2.session_id, session_id,
        "resumed session must keep the prior session id"
    );

    // Third login: forged token (right session id, wrong secret) -> rejected.
    let forged = build_resume_token(&session_id, b"not-the-real-token-bytes-32-byteslong");
    let (_tls3, sh3, _success3) = client_handshake(gw_addr, api_key, Some(forged)).await;
    assert_eq!(
        sh3.resume_accepted,
        Some(false),
        "forged resume token must be rejected"
    );
    assert_ne!(
        sh3.session_id, session_id,
        "a rejected resume must issue a new session id"
    );
}
