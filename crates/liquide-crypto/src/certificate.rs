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
    fn load(&self, _subject: &str) -> super::Result<CertificateBundle> {
        Err(super::CryptoError::Certificate(
            "FsCertificateStore::load not yet implemented".into(),
        ))
    }

    fn store(&self, _subject: &str, _bundle: &CertificateBundle) -> super::Result<()> {
        Err(super::CryptoError::Certificate(
            "FsCertificateStore::store not yet implemented".into(),
        ))
    }

    fn is_valid(&self, _subject: &str) -> super::Result<bool> {
        Err(super::CryptoError::Certificate(
            "FsCertificateStore::is_valid not yet implemented".into(),
        ))
    }
}
