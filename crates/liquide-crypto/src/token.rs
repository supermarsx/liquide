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
    /// Uses OS-provided randomness (`/dev/urandom` on Unix,
    /// `BCryptGenRandom` on Windows).
    ///
    /// # Errors
    ///
    /// Returns a [`CryptoError`](super::CryptoError) if the system random
    /// number generator is unavailable.
    pub fn generate(ttl: Duration) -> super::Result<Self> {
        let mut value = vec![0u8; 32];
        fill_random(&mut value)?;
        Ok(Self {
            value,
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

/// Fill a buffer with cryptographically secure random bytes.
#[cfg(unix)]
fn fill_random(buf: &mut [u8]) -> super::Result<()> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom")
        .map_err(|e| super::CryptoError::Token(format!("failed to open /dev/urandom: {e}")))?;
    f.read_exact(buf)
        .map_err(|e| super::CryptoError::Token(format!("failed to read /dev/urandom: {e}")))?;
    Ok(())
}

/// Fill a buffer with cryptographically secure random bytes using BCryptGenRandom.
#[cfg(windows)]
fn fill_random(buf: &mut [u8]) -> super::Result<()> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(
            h_algorithm: *mut core::ffi::c_void,
            pb_buffer: *mut u8,
            cb_buffer: u32,
            dw_flags: u32,
        ) -> i32;
    }

    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;
    // SAFETY: We pass a valid buffer and length. BCRYPT_USE_SYSTEM_PREFERRED_RNG
    // means the first parameter (algorithm handle) can be null.
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return Err(super::CryptoError::Token(format!(
            "BCryptGenRandom failed with NTSTATUS 0x{status:08X}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_random_bytes() {
        let token = SessionToken::generate(Duration::from_secs(3600)).unwrap();
        assert_eq!(token.value.len(), 32);
        // Should not be all zeros (extremely unlikely with real randomness).
        assert!(token.value.iter().any(|&b| b != 0));
    }

    #[test]
    fn two_tokens_differ() {
        let a = SessionToken::generate(Duration::from_secs(60)).unwrap();
        let b = SessionToken::generate(Duration::from_secs(60)).unwrap();
        assert_ne!(a.value, b.value);
    }

    #[test]
    fn fresh_token_not_expired() {
        let token = SessionToken::generate(Duration::from_secs(3600)).unwrap();
        assert!(!token.is_expired());
    }

    #[test]
    fn zero_ttl_token_expires_immediately() {
        let token = SessionToken::generate(Duration::from_secs(0)).unwrap();
        // Allow a tiny window — the token was just created, but TTL=0 means
        // any elapsed time > 0 makes it expired.
        std::thread::sleep(Duration::from_millis(1));
        assert!(token.is_expired());
    }
}
