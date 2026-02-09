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
        // Stub — real implementation reads certs, builds the config.
        let _ = &self.cert_path;
        Err(super::CryptoError::Tls(
            "server TLS config not yet implemented".into(),
        ))
    }

    /// Build a `rustls::ClientConfig` from this [`TlsConfig`].
    pub fn build_client_config(&self) -> super::Result<Arc<rustls::ClientConfig>> {
        let _ = &self.cert_path;
        Err(super::CryptoError::Tls(
            "client TLS config not yet implemented".into(),
        ))
    }
}
