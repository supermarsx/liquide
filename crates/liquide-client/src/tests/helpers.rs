use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use liquide_protocol::messages::control::{
    CapabilitiesMsg, ClientHello, LoginFailure, LoginPrompt, LoginResponse, LoginSuccess,
    ServerHello,
};
use liquide_protocol::version;

fn self_signed_cert_and_key() -> (
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
) {
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).expect("rcgen");
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
    );
    (vec![cert_der], key_der)
}

fn server_tls_config() -> (
    Arc<rustls::ServerConfig>,
    rustls::pki_types::CertificateDer<'static>,
) {
    let (certs, key) = self_signed_cert_and_key();
    let trust_cert = certs[0].clone();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = Arc::new(
        rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("protocol versions")
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("server config"),
    );
    (config, trust_cert)
}

async fn send_msg<W: AsyncWriteExt + Unpin, T: serde::Serialize>(w: &mut W, msg: &T) {
    let mut buf = Vec::new();
    ciborium::into_writer(msg, &mut buf).unwrap();
    let len = (buf.len() as u32).to_le_bytes();
    w.write_all(&len).await.unwrap();
    w.write_all(&buf).await.unwrap();
    w.flush().await.unwrap();
}

/// Spin up a mock TLS server that performs the Liquide handshake.
/// Returns (listener_addr, join_handle).
pub async fn mock_tls_server(
    auth_result: bool,
) -> (
    SocketAddr,
    rustls::pki_types::CertificateDer<'static>,
    tokio::task::JoinHandle<()>,
) {
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = tcp.local_addr().unwrap();
    let (tls_cfg, trust_cert) = server_tls_config();
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

            // Read CapabilitiesMsg
            let mut lb2 = [0u8; 4];
            if tls.read_exact(&mut lb2).await.is_ok() {
                let l2 = u32::from_le_bytes(lb2) as usize;
                let mut p2 = vec![0u8; l2];
                let _ = tls.read_exact(&mut p2).await;

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

    (addr, trust_cert, handle)
}
