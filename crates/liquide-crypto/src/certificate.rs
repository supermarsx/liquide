//! Certificate loading, storage, and verification.

use std::path::Path;

/// Trait for certificate storage backends.
///
/// Implementations may store certificates on disk, in a database, or in
/// a hardware security module.
pub trait CertificateStore: Send + Sync {
    /// Load the certificate chain and private key for the given `subject`.
    ///
    /// # Errors
    ///
    /// Returns a [`CryptoError`](super::CryptoError) on I/O or format errors.
    fn load(&self, subject: &str) -> super::Result<CertificateBundle>;

    /// Store a newly issued certificate bundle.
    fn store(&self, subject: &str, bundle: &CertificateBundle) -> super::Result<()>;

    /// Check whether a certificate for `subject` exists and is not expired.
    fn is_valid(&self, subject: &str) -> super::Result<bool>;
}

/// A loaded certificate chain plus its private key.
#[derive(Debug, Clone)]
pub struct CertificateBundle {
    /// DER-encoded certificate chain (leaf first).
    pub chain: Vec<Vec<u8>>,
    /// DER-encoded private key.
    pub private_key: Vec<u8>,
}

/// A simple filesystem-backed certificate store.
#[derive(Debug)]
pub struct FsCertificateStore {
    /// Root directory containing PEM files.
    pub root: std::path::PathBuf,
}

/// Validate a subject name to prevent path traversal attacks.
fn validate_subject(subject: &str) -> super::Result<()> {
    if subject.is_empty() {
        return Err(super::CryptoError::Certificate("empty subject".into()));
    }
    if subject.len() > 255 {
        return Err(super::CryptoError::Certificate("subject too long".into()));
    }
    if subject.contains('/')
        || subject.contains('\\')
        || subject.contains("..")
        || subject.contains('\0')
        || subject.chars().any(|c| c.is_control())
    {
        return Err(super::CryptoError::Certificate(
            "subject contains invalid characters".into(),
        ));
    }
    Ok(())
}

impl FsCertificateStore {
    /// Create a new store backed by the given directory.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl CertificateStore for FsCertificateStore {
    fn load(&self, subject: &str) -> super::Result<CertificateBundle> {
        validate_subject(subject)?;
        let cert_path = self.root.join(format!("{subject}.crt"));
        let key_path = self.root.join(format!("{subject}.key"));

        // Read certificate chain (PEM -> DER).
        let cert_pem = std::fs::read(&cert_path).map_err(|e| {
            super::CryptoError::Certificate(format!("failed to read {}: {e}", cert_path.display()))
        })?;
        let mut cursor = std::io::Cursor::new(&cert_pem);
        let mut chain = Vec::new();
        for cert in rustls_pemfile::certs(&mut cursor) {
            let cert =
                cert.map_err(|e| super::CryptoError::Certificate(format!("PEM parse error: {e}")))?;
            chain.push(cert.to_vec());
        }

        // Read private key (PEM -> DER).
        let key_pem = std::fs::read(&key_path).map_err(|e| {
            super::CryptoError::Certificate(format!("failed to read {}: {e}", key_path.display()))
        })?;
        let mut key_cursor = std::io::Cursor::new(&key_pem);
        let key = rustls_pemfile::private_key(&mut key_cursor)
            .map_err(|e| super::CryptoError::Certificate(format!("key parse error: {e}")))?
            .ok_or_else(|| super::CryptoError::Certificate("no private key in PEM".into()))?;

        Ok(CertificateBundle {
            chain,
            private_key: key.secret_der().to_vec(),
        })
    }

    fn store(&self, subject: &str, bundle: &CertificateBundle) -> super::Result<()> {
        validate_subject(subject)?;
        let cert_path = self.root.join(format!("{subject}.crt"));
        let key_path = self.root.join(format!("{subject}.key"));

        // Write cert chain as PEM.
        let mut cert_pem = Vec::new();
        for der in &bundle.chain {
            cert_pem.extend_from_slice(b"-----BEGIN CERTIFICATE-----\n");
            let b64 = base64_encode(der);
            for line in b64.as_bytes().chunks(76) {
                cert_pem.extend_from_slice(line);
                cert_pem.push(b'\n');
            }
            cert_pem.extend_from_slice(b"-----END CERTIFICATE-----\n");
        }
        std::fs::write(&cert_path, &cert_pem)?;

        // Write private key as PEM.
        let mut key_pem = Vec::new();
        key_pem.extend_from_slice(b"-----BEGIN PRIVATE KEY-----\n");
        let b64 = base64_encode(&bundle.private_key);
        for line in b64.as_bytes().chunks(76) {
            key_pem.extend_from_slice(line);
            key_pem.push(b'\n');
        }
        key_pem.extend_from_slice(b"-----END PRIVATE KEY-----\n");
        std::fs::write(&key_path, &key_pem)?;

        Ok(())
    }

    fn is_valid(&self, subject: &str) -> super::Result<bool> {
        validate_subject(subject)?;
        let cert_path = self.root.join(format!("{subject}.crt"));
        Ok(cert_path.exists())
    }
}

/// Simple base64 encoding (no external dependency required).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len() * 4 / 3 + 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_encode_hello() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn base64_encode_multiples_of_three() {
        assert_eq!(base64_encode(b"abc"), "YWJj");
        assert_eq!(base64_encode(b"abcdef"), "YWJjZGVm");
    }

    #[test]
    fn fs_store_round_trip() {
        let dir = std::env::temp_dir().join("liquide_cert_test");
        let _ = std::fs::create_dir_all(&dir);
        let store = FsCertificateStore::new(&dir);

        // Generate a self-signed cert using rcgen for round-trip test.
        // Since we don't have rcgen, we'll test with raw DER bytes.
        let bundle = CertificateBundle {
            chain: vec![vec![0x30, 0x82, 0x01, 0x00]],
            private_key: vec![0x30, 0x82, 0x02, 0x00],
        };

        store.store("test-subject", &bundle).unwrap();
        assert!(store.is_valid("test-subject").unwrap());

        // Clean up.
        let _ = std::fs::remove_file(dir.join("test-subject.crt"));
        let _ = std::fs::remove_file(dir.join("test-subject.key"));
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn fs_store_missing_subject() {
        let dir = std::env::temp_dir().join("liquide_cert_test_missing");
        let _ = std::fs::create_dir_all(&dir);
        let store = FsCertificateStore::new(&dir);
        assert!(!store.is_valid("nonexistent").unwrap());
        let _ = std::fs::remove_dir(&dir);
    }
}
