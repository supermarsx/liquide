//! Multi-factor authentication (MFA) support: TOTP, FIDO2, etc.

use crate::AuthError;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Supported MFA methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MfaMethod {
    /// Time-based one-time password (RFC 6238).
    Totp,
    /// FIDO2 / WebAuthn hardware key.
    Fido2,
    /// Push notification to a registered mobile device.
    Push,
}

/// An MFA challenge issued to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaChallenge {
    /// Which MFA method is being used.
    pub method: MfaMethod,
    /// An opaque challenge identifier.
    pub challenge_id: String,
}

/// Verify an MFA response against a challenge.
///
/// # Errors
///
/// Returns [`AuthError::MfaFailed`](super::AuthError::MfaFailed) if the code
/// is incorrect or expired.
pub fn verify(challenge: &MfaChallenge, code: &str) -> super::Result<bool> {
    match challenge.method {
        MfaMethod::Totp => verify_totp(&challenge.challenge_id, code),
        MfaMethod::Fido2 => {
            // FIDO2 requires WebAuthn protocol -- not implemented yet
            tracing::warn!("FIDO2 verification not yet implemented");
            Err(AuthError::Internal("FIDO2 not implemented".into()))
        }
        MfaMethod::Push => {
            // Push notification verification requires mobile integration
            tracing::warn!("Push verification not yet implemented");
            Err(AuthError::Internal("Push MFA not implemented".into()))
        }
    }
}

/// TOTP verification (RFC 6238).
///
/// The `secret` is the base32-encoded shared secret stored in `challenge_id`.
/// The `code` is the 6-digit code from the user's authenticator app.
fn verify_totp(secret_b32: &str, code: &str) -> super::Result<bool> {
    // Validate code format (6 digits)
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Ok(false);
    }
    let user_code: u32 = code.parse().unwrap_or(0);

    // Decode base32 secret
    let secret = match base32_decode(secret_b32) {
        Some(s) => s,
        None => return Err(AuthError::Internal("invalid TOTP secret encoding".into())),
    };

    // Get current time step (30-second window)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let time_step = now / 30;

    // Check current window and +/-1 for clock skew tolerance
    for offset in [0i64, -1, 1] {
        let step = (time_step as i64 + offset) as u64;
        let expected = generate_totp(&secret, step);
        if expected == user_code {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Generate a TOTP code for a given time step using HMAC-SHA1.
fn generate_totp(secret: &[u8], time_step: u64) -> u32 {
    let msg = time_step.to_be_bytes();
    let hash = hmac_sha1(secret, &msg);

    // Dynamic truncation (RFC 4226 section 5.4)
    let offset = (hash[19] & 0x0F) as usize;
    let code = ((hash[offset] as u32 & 0x7F) << 24)
        | ((hash[offset + 1] as u32) << 16)
        | ((hash[offset + 2] as u32) << 8)
        | (hash[offset + 3] as u32);

    code % 1_000_000 // 6 digits
}

/// HMAC-SHA1 (RFC 2104) -- used by TOTP/HOTP.
fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    const BLOCK_SIZE: usize = 64;

    // If key is longer than block size, hash it first
    let key = if key.len() > BLOCK_SIZE {
        let h = sha1(key);
        h.to_vec()
    } else {
        key.to_vec()
    };

    // Pad key to block size
    let mut padded_key = vec![0u8; BLOCK_SIZE];
    padded_key[..key.len()].copy_from_slice(&key);

    // Inner and outer padding
    let mut ipad = vec![0x36u8; BLOCK_SIZE];
    let mut opad = vec![0x5Cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= padded_key[i];
        opad[i] ^= padded_key[i];
    }

    // Inner hash: SHA1(ipad || message)
    let mut inner = ipad;
    inner.extend_from_slice(message);
    let inner_hash = sha1(&inner);

    // Outer hash: SHA1(opad || inner_hash)
    let mut outer = opad;
    outer.extend_from_slice(&inner_hash);
    sha1(&outer)
}

/// SHA-1 hash (FIPS 180-4) -- ONLY for HMAC-SHA1 in TOTP. NOT for security.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0,
    ];

    // Pre-processing: pad message
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    // Process 512-bit blocks
    for block in padded.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = h;
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, &val) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

/// Decode base32 (RFC 4648) without padding.
fn base32_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut lookup = [255u8; 256];
    for (i, &b) in ALPHABET.iter().enumerate() {
        lookup[b as usize] = i as u8;
        lookup[b.to_ascii_lowercase() as usize] = i as u8;
    }

    let input = input.trim_end_matches('=');
    let mut result = Vec::with_capacity(input.len() * 5 / 8);
    let mut buffer: u64 = 0;
    let mut bits = 0;

    for &b in input.as_bytes() {
        let val = lookup[b as usize];
        if val == 255 {
            return None;
        }
        buffer = (buffer << 5) | val as u64;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_empty() {
        // SHA1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
        let hash = sha1(b"");
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_sha1_abc() {
        // SHA1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
        let hash = sha1(b"abc");
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn test_hmac_sha1_rfc2104() {
        // RFC 2104 test vector: key = 0x0b * 20, data = "Hi There"
        let key = vec![0x0bu8; 20];
        let data = b"Hi There";
        let mac = hmac_sha1(&key, data);
        let hex: String = mac.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "b617318655057264e28bc0b6fb378c8ef146be00");
    }

    #[test]
    fn test_base32_decode_basic() {
        // "JBSWY3DP" decodes to "Hello" (5 bytes = 40 bits = 8 base32 chars)
        let decoded = base32_decode("JBSWY3DP").unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_base32_decode_case_insensitive() {
        let upper = base32_decode("JBSWY3DP").unwrap();
        let lower = base32_decode("jbswy3dp").unwrap();
        assert_eq!(upper, lower);
    }

    #[test]
    fn test_base32_decode_with_padding() {
        // base32("fo") = "MZXQ===="
        let decoded = base32_decode("MZXQ====").unwrap();
        assert_eq!(decoded, b"fo");
    }

    #[test]
    fn test_base32_decode_invalid() {
        assert!(base32_decode("!!!").is_none());
    }

    #[test]
    fn test_generate_totp_rfc6238_vector() {
        // RFC 6238 test vector: secret = "12345678901234567890" (ASCII), time step = 1
        let secret = b"12345678901234567890";
        // Time = 59 seconds -> time_step = 59/30 = 1
        let code = generate_totp(secret, 1);
        // The expected TOTP at step 1 for this secret is 287082
        assert_eq!(code, 287082);
    }

    #[test]
    fn test_generate_totp_step_zero() {
        // RFC 6238: time=0, step=0 for SHA1 secret "12345678901234567890"
        // Expected: 755224 (this is actually HOTP counter=0)
        let secret = b"12345678901234567890";
        let code = generate_totp(secret, 0);
        assert_eq!(code, 755224);
    }

    #[test]
    fn test_verify_totp_invalid_format() {
        // Too short
        assert_eq!(verify_totp("JBSWY3DPEHPK3PXP", "123").unwrap(), false);
        // Non-digits
        assert_eq!(verify_totp("JBSWY3DPEHPK3PXP", "abcdef").unwrap(), false);
        // Too long
        assert_eq!(
            verify_totp("JBSWY3DPEHPK3PXP", "1234567").unwrap(),
            false
        );
    }

    #[test]
    fn test_verify_totp_invalid_secret() {
        let result = verify_totp("!!invalid!!", "123456");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_fido2_not_implemented() {
        let challenge = MfaChallenge {
            method: MfaMethod::Fido2,
            challenge_id: "test".into(),
        };
        assert!(verify(&challenge, "code").is_err());
    }

    #[test]
    fn test_verify_push_not_implemented() {
        let challenge = MfaChallenge {
            method: MfaMethod::Push,
            challenge_id: "test".into(),
        };
        assert!(verify(&challenge, "code").is_err());
    }

    #[test]
    fn test_totp_current_time_roundtrip() {
        // Generate a code for the current time step, then verify it matches
        let secret = b"12345678901234567890";
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let step = now / 30;
        let code = generate_totp(secret, step);
        let code_str = format!("{code:06}");

        // The base32 encoding of "12345678901234567890" is "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
        let result = verify_totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", &code_str).unwrap();
        assert!(result, "TOTP roundtrip failed for code {code_str}");
    }
}
