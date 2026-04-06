//! TLS configuration builder and helpers.

use std::path::PathBuf;
use std::sync::Arc;

/// TLS configuration used by both server and client transports.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to the PEM-encoded certificate chain.
    pub cert_path: PathBuf,
    /// Path to the PEM-encoded private key.
    pub key_path: PathBuf,
    /// Optional path to a CA bundle for peer verification.
    pub ca_path: Option<PathBuf>,
    /// Whether to require client certificates (mTLS).
    pub require_client_auth: bool,
    /// Minimum TLS version to accept.
    pub min_version: TlsVersion,
}

/// Supported TLS protocol versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.2.
    Tls12,
    /// TLS 1.3 (default and recommended).
    Tls13,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: PathBuf::from("/etc/liquide/cert.pem"),
            key_path: PathBuf::from("/etc/liquide/key.pem"),
            ca_path: None,
            require_client_auth: false,
            min_version: TlsVersion::Tls13,
        }
    }
}

impl TlsConfig {
    /// Build a `rustls::ServerConfig` from this [`TlsConfig`].
    ///
    /// # Errors
    ///
    /// Returns a [`CryptoError`](super::CryptoError) if the certificate or key
    /// files cannot be loaded or are invalid.
    pub fn build_server_config(&self) -> super::Result<Arc<rustls::ServerConfig>> {
        use rustls::pki_types::CertificateDer;

        // Load certificate chain from PEM file.
        let cert_file = std::fs::File::open(&self.cert_path)
            .map_err(|e| super::CryptoError::Certificate(format!("failed to open cert: {e}")))?;
        let mut cert_reader = std::io::BufReader::new(cert_file);
        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| super::CryptoError::Certificate(format!("failed to parse certs: {e}")))?;
        if certs.is_empty() {
            return Err(super::CryptoError::Certificate(
                "no certificates found in PEM file".into(),
            ));
        }

        // Load private key from PEM file.
        let key_file = std::fs::File::open(&self.key_path)
            .map_err(|e| super::CryptoError::Certificate(format!("failed to open key: {e}")))?;
        let mut key_reader = std::io::BufReader::new(key_file);
        let key = rustls_pemfile::private_key(&mut key_reader)
            .map_err(|e| super::CryptoError::Certificate(format!("failed to parse key: {e}")))?
            .ok_or_else(|| {
                super::CryptoError::Certificate("no private key found in PEM file".into())
            })?;

        // Build server config.
        let mut config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key.into())
            .map_err(|e| super::CryptoError::Tls(format!("server config error: {e}")))?;

        // Set ALPN protocols.
        config.alpn_protocols = vec![b"liquide/1".to_vec()];

        Ok(Arc::new(config))
    }

    /// Build a `rustls::ClientConfig` from this [`TlsConfig`].
    ///
    /// # Errors
    ///
    /// Returns a [`CryptoError`](super::CryptoError) if the CA bundle cannot
    /// be loaded or contains invalid certificates.
    pub fn build_client_config(&self) -> super::Result<Arc<rustls::ClientConfig>> {
        use rustls::pki_types::CertificateDer;

        let mut root_store = rustls::RootCertStore::empty();

        // Load custom CA if provided.
        if let Some(ref ca_path) = self.ca_path {
            let ca_file = std::fs::File::open(ca_path)
                .map_err(|e| super::CryptoError::Certificate(format!("failed to open CA: {e}")))?;
            let mut ca_reader = std::io::BufReader::new(ca_file);
            let ca_certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut ca_reader)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| {
                    super::CryptoError::Certificate(format!("failed to parse CA certs: {e}"))
                })?;
            for cert in ca_certs {
                root_store
                    .add(cert)
                    .map_err(|e| super::CryptoError::Certificate(format!("invalid CA cert: {e}")))?;
            }
        }

        if root_store.is_empty() && self.ca_path.is_none() {
            tracing::warn!("no CA configured — TLS client will reject all certificates");
        }

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(Arc::new(config))
    }
}
