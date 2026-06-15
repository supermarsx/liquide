//! Integration coverage for client credential routing (audit TODO 4) and
//! client-side session resume (audit TODO 5).
//!
//! A mock TLS server speaks the Liquide handshake and records what the client
//! sent (the credential and the `ClientHello.resume_token`), so we can assert
//! that real credentials reach the wire and that the second connection presents
//! a resume token built from the first login's session id + token.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use liquide_client::connection::{ConnectionManager, ConnectionState};
use liquide_protocol::messages::control::{
    parse_resume_token, CapabilitiesMsg, ClientHello, LoginFailure, LoginPrompt, LoginResponse,
    LoginSuccess, ServerHello,
};
use liquide_protocol::version;

fn self_signed() -> (
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
    rustls::pki_types::CertificateDer<'static>,
) {
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
    let trust = cert_der.clone();
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
    );
    (vec![cert_der], key_der, trust)
}

fn server_tls() -> (
    Arc<rustls::ServerConfig>,
    rustls::pki_types::CertificateDer<'static>,
) {
    let (certs, key, trust) = self_signed();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = Arc::new(
        rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap(),
    );
    (cfg, trust)
}

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

/// What the mock server observed from one client connection.
struct Observed {
    credential: Vec<u8>,
    resume_token: Option<Vec<u8>>,
}

/// Run a one-shot mock server. `accept` decides auth success/failure; the
/// issued session id/token are fixed so the test can predict the resume token.
/// Reports what it saw on `tx`.
async fn mock_server(
    accept: bool,
    session_id: &'static str,
    session_token: Vec<u8>,
    resume_accepted: Option<bool>,
    tx: mpsc::Sender<Observed>,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>, rustls::pki_types::CertificateDer<'static>) {
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = tcp.local_addr().unwrap();
    let (cfg, trust) = server_tls();
    let acceptor = tokio_rustls::TlsAcceptor::from(cfg);

    let handle = tokio::spawn(async move {
        let (stream, _) = tcp.accept().await.unwrap();
        let mut tls = acceptor.accept(stream).await.unwrap();

        let hello: ClientHello = recv(&mut tls).await;
        send(
            &mut tls,
            &ServerHello {
                protocol_version: version::PROTOCOL_VERSION.to_string(),
                server_name: "mock".to_string(),
                server_version: "0.1.0".to_string(),
                selected_transport: "tcp+tls".to_string(),
                selected_video_codec: "h264".to_string(),
                selected_audio_codec: "opus".to_string(),
                channels: BTreeMap::new(),
                session_id: session_id.to_string(),
                resume_accepted,
                features: BTreeMap::new(),
            },
        )
        .await;

        send(
            &mut tls,
            &LoginPrompt {
                available_methods: vec!["password".to_string()],
                avatar_png: None,
                session_resume_available: None,
                server_greeting: None,
            },
        )
        .await;

        let login: LoginResponse = recv(&mut tls).await;

        tx.send(Observed {
            credential: login.credential.clone(),
            resume_token: hello.resume_token.clone(),
        })
        .unwrap();

        if accept {
            send(
                &mut tls,
                &LoginSuccess {
                    session_id: session_id.to_string(),
                    session_token,
                    session_features: BTreeMap::new(),
                    token_lifetime_sec: Some(3600),
                },
            )
            .await;
            // Capability exchange.
            let _caps: CapabilitiesMsg = recv(&mut tls).await;
            send(
                &mut tls,
                &CapabilitiesMsg {
                    action: "confirm".to_string(),
                    capabilities: BTreeMap::new(),
                    request_id: None,
                },
            )
            .await;
        } else {
            send(
                &mut tls,
                &LoginFailure {
                    error_code: 1,
                    reason: "invalid credentials".to_string(),
                    retry_after_sec: None,
                    remaining_attempts: Some(2),
                },
            )
            .await;
        }
        let _ = tls.shutdown().await;
    });

    (addr, handle, trust)
}

/// TODO 4: real credentials supplied to `connect_with_credential` reach the
/// wire as `user:pass`, and a successful login lands the client in Connected.
#[tokio::test]
async fn credentials_reach_the_wire_on_success() {
    let (tx, rx) = mpsc::channel();
    let (addr, server, trust) =
        mock_server(true, "sess-A", vec![9, 8, 7, 6], None, tx).await;

    let mut mgr = ConnectionManager::new(3);
    mgr.add_trusted_server_certificate(trust);
    mgr.connect_with_credential(&addr.to_string(), "alice", "s3cr3t")
        .await
        .unwrap();
    assert_eq!(mgr.state(), ConnectionState::Connected);
    assert_eq!(mgr.session_id(), Some("sess-A"));
    assert!(mgr.has_session_token(), "token must be retained for resume");

    let seen = rx.recv().unwrap();
    assert_eq!(
        seen.credential, b"alice:s3cr3t",
        "credential must reach the wire as user:pass"
    );
    assert!(
        seen.resume_token.is_none(),
        "first connect must not send a resume token"
    );

    server.await.unwrap();
}

/// TODO 4: an auth failure surfaces as an error and leaves the client not
/// connected.
#[tokio::test]
async fn auth_failure_is_reported() {
    let (tx, _rx) = mpsc::channel();
    let (addr, server, trust) =
        mock_server(false, "sess-X", vec![1], None, tx).await;

    let mut mgr = ConnectionManager::new(3);
    mgr.add_trusted_server_certificate(trust);
    let result = mgr
        .connect_with_credential(&addr.to_string(), "bad", "creds")
        .await;
    assert!(result.is_err(), "auth failure must be an error");
    assert_ne!(mgr.state(), ConnectionState::Connected);

    server.await.unwrap();
}

/// TODO 5: after a first login, a reconnect presents a resume token built from
/// the stored session id + token, and the client records the server's
/// `resume_accepted`.
#[tokio::test]
async fn second_connect_sends_resume_token() {
    let session_id = "sess-RESUME";
    let token = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x11, 0x22];

    // First login: establishes the stored session id + token.
    let (tx1, rx1) = mpsc::channel();
    let (addr1, server1, trust1) =
        mock_server(true, session_id, token.clone(), None, tx1).await;
    let mut mgr = ConnectionManager::new(3);
    mgr.add_trusted_server_certificate(trust1);
    mgr.connect_with_credential(&addr1.to_string(), "bob", "pw")
        .await
        .unwrap();
    let seen1 = rx1.recv().unwrap();
    assert!(seen1.resume_token.is_none());
    server1.await.unwrap();

    // Second login to a new mock that accepts the resume: the client must send
    // a resume token decoding to (session_id, token).
    let (tx2, rx2) = mpsc::channel();
    let (addr2, server2, trust2) =
        mock_server(true, session_id, token.clone(), Some(true), tx2).await;
    mgr.add_trusted_server_certificate(trust2);
    mgr.connect_with_credential(&addr2.to_string(), "bob", "pw")
        .await
        .unwrap();

    let seen2 = rx2.recv().unwrap();
    let resume = seen2
        .resume_token
        .expect("second connect must present a resume token");
    let (parsed_id, parsed_token) =
        parse_resume_token(&resume).expect("resume token must be well-formed");
    assert_eq!(parsed_id, session_id);
    assert_eq!(parsed_token, token);
    assert_eq!(
        mgr.resume_accepted(),
        Some(true),
        "client must record the server's resume decision"
    );

    server2.await.unwrap();
}
