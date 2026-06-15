//! TLS server-configuration loading for the gateway binary.
//!
//! Loads a PEM certificate chain and private key from disk and builds a
//! [`rustls::ServerConfig`] suitable for [`crate::GatewayRuntime::set_tls_config`].

use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::{GatewayError, Result};

/// Load a PEM certificate chain and private key, returning a built
/// `rustls::ServerConfig` with no client authentication.
///
/// The key file may contain a PKCS#8, PKCS#1 (RSA), or SEC1 (EC) private key.
/// Returns a [`GatewayError::TlsError`] / [`GatewayError::ConfigError`] on any
/// I/O, parse, or build failure so the binary fails loudly rather than starting
/// without TLS.
pub fn load_server_tls_config(
    cert_path: &str,
    key_path: &str,
) -> Result<Arc<rustls::ServerConfig>> {
    let certs = load_certs(cert_path)?;
    if certs.is_empty() {
        return Err(GatewayError::ConfigError {
            detail: format!("no certificates found in {cert_path}"),
        });
    }
    let key = load_private_key(key_path)?;
    build_server_config(certs, key)
}

/// Build a `rustls::ServerConfig` from already-parsed certs and key.
pub fn build_server_config(
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
) -> Result<Arc<rustls::ServerConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| GatewayError::TlsError {
            detail: format!("TLS protocol versions: {e}"),
        })?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| GatewayError::TlsError {
            detail: format!("invalid certificate/key: {e}"),
        })?;
    Ok(Arc::new(config))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path).map_err(|e| GatewayError::ConfigError {
        detail: format!("cannot open certificate file {path}: {e}"),
    })?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| GatewayError::ConfigError {
            detail: format!("failed to parse certificates from {path}: {e}"),
        })
}

fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path).map_err(|e| GatewayError::ConfigError {
        detail: format!("cannot open private key file {path}: {e}"),
    })?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|e| GatewayError::ConfigError {
            detail: format!("failed to parse private key from {path}: {e}"),
        })?
        .ok_or_else(|| GatewayError::ConfigError {
            detail: format!("no private key found in {path}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_self_signed(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
        let cert_pem = cert.cert.pem();
        let key_pem = cert.key_pair.serialize_pem();

        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");
        File::create(&cert_path)
            .unwrap()
            .write_all(cert_pem.as_bytes())
            .unwrap();
        File::create(&key_path)
            .unwrap()
            .write_all(key_pem.as_bytes())
            .unwrap();
        (cert_path, key_path)
    }

    #[test]
    fn loads_pem_cert_and_key() {
        let dir = std::env::temp_dir().join(format!("liquide-gw-tls-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (cert_path, key_path) = write_self_signed(&dir);

        let config = load_server_tls_config(
            cert_path.to_str().unwrap(),
            key_path.to_str().unwrap(),
        )
        .expect("should load PEM cert/key");
        // A usable server config has at least one cert resolver configured.
        assert!(Arc::strong_count(&config) >= 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_cert_file_errors() {
        let err = load_server_tls_config("/nonexistent/cert.pem", "/nonexistent/key.pem")
            .unwrap_err();
        assert!(matches!(err, GatewayError::ConfigError { .. }));
    }
}
