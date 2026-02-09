//! Multi-factor authentication (MFA) support: TOTP, FIDO2, etc.

use serde::{Deserialize, Serialize};

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
pub fn verify(_challenge: &MfaChallenge, _code: &str) -> super::Result<bool> {
    todo!("MFA verification")
}
