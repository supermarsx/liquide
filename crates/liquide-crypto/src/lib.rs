#![doc = "TLS configuration, certificate management, and token generation."]
#![doc = ""]
#![doc = "This crate centralises all cryptographic plumbing so that other crates"]
#![doc = "never deal with raw TLS or certificate details directly."]

pub mod certificate;
pub mod tls;
pub mod token;

pub use certificate::CertificateStore;
pub use tls::TlsConfig;

use thiserror::Error;

/// Errors originating from cryptographic operations.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// A TLS handshake or configuration error.
    #[error("TLS error: {0}")]
    Tls(String),

    /// A certificate could not be loaded or verified.
    #[error("certificate error: {0}")]
    Certificate(String),

    /// Token generation or validation failed.
    #[error("token error: {0}")]
    Token(String),

    /// I/O failure while reading key material.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, CryptoError>;

impl From<CryptoError> for liquide_common::LiquideError {
    fn from(e: CryptoError) -> Self {
        Self::Crypto(e.to_string())
    }
}
