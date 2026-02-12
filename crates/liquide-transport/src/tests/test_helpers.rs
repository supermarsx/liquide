use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Install the `ring` crypto provider for rustls (idempotent).
fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Self-signed test certificate and key pair.
pub struct TestCert {
    pub cert_der: CertificateDer<'static>,
    pub key_der: PrivateKeyDer<'static>,
}

/// Generate a self-signed certificate for `localhost` using `rcgen`.
pub fn generate_self_signed() -> TestCert {
    ensure_crypto_provider();
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    TestCert {
        cert_der: ck.cert.der().clone(),
        key_der: PrivateKeyDer::from(PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der())),
    }
}

/// Build a `rustls` client config that trusts only the given certificate.
pub fn make_rustls_client_config(cert: &CertificateDer<'static>) -> Arc<rustls::ClientConfig> {
    ensure_crypto_provider();
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add(cert.clone()).unwrap();
    Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    )
}

/// Build a `rustls` server config from the test certificate.
pub fn make_rustls_server_config(tc: &TestCert) -> Arc<rustls::ServerConfig> {
    Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![tc.cert_der.clone()], tc.key_der.clone_key())
            .unwrap(),
    )
}

/// Build a `quinn::ServerConfig` from the test certificate.
#[cfg(feature = "quic")]
pub fn make_quinn_server_config(tc: &TestCert) -> quinn::ServerConfig {
    let rustls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![tc.cert_der.clone()], tc.key_der.clone_key())
        .unwrap();
    let quic_config =
        quinn::crypto::rustls::QuicServerConfig::try_from(rustls_config).unwrap();
    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_config));
    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(10)).unwrap(),
    ));
    server_config.transport_config(Arc::new(transport));
    server_config
}
