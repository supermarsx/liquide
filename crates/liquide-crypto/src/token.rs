//! Opaque token generation and validation for session authentication.

use std::time::{Duration, SystemTime};

/// An opaque bearer token issued after successful authentication.
#[derive(Debug, Clone)]
pub struct SessionToken {
    /// Raw token bytes (typically random or a signed JWT).
    pub value: Vec<u8>,
    /// When the token was issued.
    pub issued_at: SystemTime,
    /// Token lifetime.
    pub ttl: Duration,
}

impl SessionToken {
    /// Generate a new random session token with the given TTL.
    ///
    /// # Errors
    ///
    /// Returns a [`CryptoError`](super::CryptoError) if the system random
    /// number generator is unavailable.
    pub fn generate(ttl: Duration) -> super::Result<Self> {
        // Stub: real implementation would use a CSPRNG.
        Ok(Self {
            value: vec![0u8; 32], // placeholder
            issued_at: SystemTime::now(),
            ttl,
        })
    }

    /// Check whether the token has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.issued_at
            .elapsed()
            .map(|elapsed| elapsed > self.ttl)
            .unwrap_or(true)
    }
}
