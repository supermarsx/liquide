/// Multi-factor authentication support.
///
/// Provides traits and types for second-factor challenges (TOTP, SMS,
/// push notifications, hardware keys) that can be wired into the
/// greeter/session flow after primary authentication succeeds.

// ---------------------------------------------------------------------------
// Challenge types
// ---------------------------------------------------------------------------

/// The kind of second-factor challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaChallengeType {
    /// Time-based one-time password (RFC 6238).
    TOTP,
    /// One-time code sent via SMS.
    SMS,
    /// One-time code sent via email.
    Email,
    /// Push notification to a mobile app.
    Push,
    /// Hardware security key (FIDO2/U2F).
    HardwareKey,
}

/// A second-factor challenge issued by an `MfaProvider`.
#[derive(Debug, Clone)]
pub struct MfaChallenge {
    /// The type of challenge.
    pub challenge_type: MfaChallengeType,
    /// Human-readable prompt (e.g. "Enter the 6-digit code from your
    /// authenticator app").
    pub prompt: String,
    /// Challenge validity period in milliseconds.  After this the
    /// challenge must be re-requested.
    pub expires_ms: u64,
}

impl MfaChallenge {
    pub fn new(challenge_type: MfaChallengeType, prompt: &str, expires_ms: u64) -> Self {
        Self {
            challenge_type,
            prompt: prompt.to_string(),
            expires_ms,
        }
    }

    /// Whether the challenge has expired at timestamp `now_ms`
    /// relative to `issued_at_ms`.
    pub fn is_expired(&self, issued_at_ms: u64, now_ms: u64) -> bool {
        now_ms >= issued_at_ms + self.expires_ms
    }
}

// ---------------------------------------------------------------------------
// MfaConfig
// ---------------------------------------------------------------------------

/// When MFA is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaRequirement {
    /// Always require MFA.
    Always,
    /// Only require MFA when authenticating from a remote session.
    WhenRemote,
    /// Never require MFA.
    Never,
}

/// Configuration for multi-factor authentication.
#[derive(Debug, Clone)]
pub struct MfaConfig {
    /// When MFA is required.
    pub required: MfaRequirement,
    /// Grace period in milliseconds after successful MFA before
    /// the user is prompted again (e.g. for screen unlock).
    pub grace_period_ms: u64,
    /// How many days to remember a device so MFA is not re-prompted.
    /// 0 = never remember.
    pub remember_device_days: u32,
}

impl Default for MfaConfig {
    fn default() -> Self {
        Self {
            required: MfaRequirement::Never,
            grace_period_ms: 300_000, // 5 minutes
            remember_device_days: 0,
        }
    }
}

impl MfaConfig {
    /// Whether MFA is effectively required given the session context.
    pub fn is_required(&self, is_remote: bool) -> bool {
        match self.required {
            MfaRequirement::Always => true,
            MfaRequirement::WhenRemote => is_remote,
            MfaRequirement::Never => false,
        }
    }
}

// ---------------------------------------------------------------------------
// MfaProvider trait
// ---------------------------------------------------------------------------

/// A provider of multi-factor authentication challenges.
pub trait MfaProvider: Send {
    /// Human-readable name for this MFA method.
    fn name(&self) -> &str;

    /// The challenge type this provider issues.
    fn challenge_type(&self) -> MfaChallengeType;

    /// Request a new challenge.  Returns a challenge with a prompt
    /// and expiration.
    fn request_challenge(&self) -> MfaChallenge;

    /// Verify the user's response to the challenge.
    fn verify(&self, code: &str) -> bool;
}

// ---------------------------------------------------------------------------
// MockMfaProvider
// ---------------------------------------------------------------------------

/// A mock MFA provider for testing.
pub struct MockMfaProvider {
    name: String,
    challenge_type: MfaChallengeType,
    /// The valid code that `verify()` will accept.
    valid_code: String,
    /// Prompt shown to the user.
    prompt: String,
    /// Challenge expiration in milliseconds.
    expires_ms: u64,
}

impl MockMfaProvider {
    /// Create a TOTP mock that accepts the given code.
    pub fn totp(valid_code: &str) -> Self {
        Self {
            name: "Mock TOTP".into(),
            challenge_type: MfaChallengeType::TOTP,
            valid_code: valid_code.to_string(),
            prompt: "Enter the 6-digit code from your authenticator app.".into(),
            expires_ms: 30_000,
        }
    }

    /// Create an SMS mock that accepts the given code.
    pub fn sms(valid_code: &str) -> Self {
        Self {
            name: "Mock SMS".into(),
            challenge_type: MfaChallengeType::SMS,
            valid_code: valid_code.to_string(),
            prompt: "Enter the code sent to your phone.".into(),
            expires_ms: 120_000,
        }
    }

    /// Create a push-notification mock (any code accepted).
    pub fn push() -> Self {
        Self {
            name: "Mock Push".into(),
            challenge_type: MfaChallengeType::Push,
            valid_code: String::new(),
            prompt: "Approve the login request on your device.".into(),
            expires_ms: 60_000,
        }
    }

    /// Create a hardware-key mock that accepts the given response.
    pub fn hardware_key(valid_response: &str) -> Self {
        Self {
            name: "Mock Hardware Key".into(),
            challenge_type: MfaChallengeType::HardwareKey,
            valid_code: valid_response.to_string(),
            prompt: "Touch your security key.".into(),
            expires_ms: 60_000,
        }
    }

    /// Builder: override the prompt.
    pub fn with_prompt(mut self, prompt: &str) -> Self {
        self.prompt = prompt.to_string();
        self
    }

    /// Builder: override the expiration.
    pub fn with_expires_ms(mut self, ms: u64) -> Self {
        self.expires_ms = ms;
        self
    }
}

impl MfaProvider for MockMfaProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn challenge_type(&self) -> MfaChallengeType {
        self.challenge_type
    }

    fn request_challenge(&self) -> MfaChallenge {
        MfaChallenge::new(self.challenge_type, &self.prompt, self.expires_ms)
    }

    fn verify(&self, code: &str) -> bool {
        if self.valid_code.is_empty() {
            // Push — accept anything (simulates server-side approval)
            true
        } else {
            code == self.valid_code
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- MfaChallengeType tests --

    #[test]
    fn challenge_type_eq() {
        assert_eq!(MfaChallengeType::TOTP, MfaChallengeType::TOTP);
        assert_ne!(MfaChallengeType::TOTP, MfaChallengeType::SMS);
        assert_ne!(MfaChallengeType::Email, MfaChallengeType::Push);
        assert_ne!(MfaChallengeType::Push, MfaChallengeType::HardwareKey);
    }

    // -- MfaChallenge tests --

    #[test]
    fn challenge_new() {
        let c = MfaChallenge::new(MfaChallengeType::TOTP, "Enter code", 30_000);
        assert_eq!(c.challenge_type, MfaChallengeType::TOTP);
        assert_eq!(c.prompt, "Enter code");
        assert_eq!(c.expires_ms, 30_000);
    }

    #[test]
    fn challenge_not_expired() {
        let c = MfaChallenge::new(MfaChallengeType::TOTP, "code", 30_000);
        assert!(!c.is_expired(1000, 1000));
        assert!(!c.is_expired(1000, 30_999));
    }

    #[test]
    fn challenge_expired() {
        let c = MfaChallenge::new(MfaChallengeType::TOTP, "code", 30_000);
        assert!(c.is_expired(1000, 31_000));
        assert!(c.is_expired(0, 30_000));
    }

    // -- MfaConfig tests --

    #[test]
    fn mfa_config_default() {
        let cfg = MfaConfig::default();
        assert_eq!(cfg.required, MfaRequirement::Never);
        assert_eq!(cfg.grace_period_ms, 300_000);
        assert_eq!(cfg.remember_device_days, 0);
    }

    #[test]
    fn mfa_config_is_required_always() {
        let cfg = MfaConfig {
            required: MfaRequirement::Always,
            ..Default::default()
        };
        assert!(cfg.is_required(false));
        assert!(cfg.is_required(true));
    }

    #[test]
    fn mfa_config_is_required_when_remote() {
        let cfg = MfaConfig {
            required: MfaRequirement::WhenRemote,
            ..Default::default()
        };
        assert!(!cfg.is_required(false));
        assert!(cfg.is_required(true));
    }

    #[test]
    fn mfa_config_is_required_never() {
        let cfg = MfaConfig {
            required: MfaRequirement::Never,
            ..Default::default()
        };
        assert!(!cfg.is_required(false));
        assert!(!cfg.is_required(true));
    }

    // -- MockMfaProvider TOTP tests --

    #[test]
    fn mock_totp_metadata() {
        let p = MockMfaProvider::totp("123456");
        assert_eq!(p.name(), "Mock TOTP");
        assert_eq!(p.challenge_type(), MfaChallengeType::TOTP);
    }

    #[test]
    fn mock_totp_challenge() {
        let p = MockMfaProvider::totp("123456");
        let c = p.request_challenge();
        assert_eq!(c.challenge_type, MfaChallengeType::TOTP);
        assert!(c.prompt.contains("authenticator"));
        assert_eq!(c.expires_ms, 30_000);
    }

    #[test]
    fn mock_totp_verify_correct() {
        let p = MockMfaProvider::totp("123456");
        assert!(p.verify("123456"));
    }

    #[test]
    fn mock_totp_verify_wrong() {
        let p = MockMfaProvider::totp("123456");
        assert!(!p.verify("000000"));
    }

    // -- MockMfaProvider SMS tests --

    #[test]
    fn mock_sms_challenge() {
        let p = MockMfaProvider::sms("9876");
        let c = p.request_challenge();
        assert_eq!(c.challenge_type, MfaChallengeType::SMS);
        assert!(c.prompt.contains("phone"));
        assert_eq!(c.expires_ms, 120_000);
    }

    #[test]
    fn mock_sms_verify() {
        let p = MockMfaProvider::sms("9876");
        assert!(p.verify("9876"));
        assert!(!p.verify("1111"));
    }

    // -- MockMfaProvider Push tests --

    #[test]
    fn mock_push_accepts_anything() {
        let p = MockMfaProvider::push();
        assert_eq!(p.challenge_type(), MfaChallengeType::Push);
        assert!(p.verify(""));
        assert!(p.verify("anything"));
    }

    // -- MockMfaProvider HardwareKey tests --

    #[test]
    fn mock_hardware_key() {
        let p = MockMfaProvider::hardware_key("FIDO_RESPONSE");
        assert_eq!(p.challenge_type(), MfaChallengeType::HardwareKey);
        assert!(p.verify("FIDO_RESPONSE"));
        assert!(!p.verify("wrong"));
    }

    // -- Builder tests --

    #[test]
    fn mock_custom_prompt() {
        let p = MockMfaProvider::totp("123456").with_prompt("Custom prompt");
        let c = p.request_challenge();
        assert_eq!(c.prompt, "Custom prompt");
    }

    #[test]
    fn mock_custom_expires() {
        let p = MockMfaProvider::totp("123456").with_expires_ms(5_000);
        let c = p.request_challenge();
        assert_eq!(c.expires_ms, 5_000);
    }

    // -- Email mock --

    #[test]
    fn mock_email_via_custom() {
        let p = MockMfaProvider {
            name: "Email OTP".into(),
            challenge_type: MfaChallengeType::Email,
            valid_code: "ABC123".into(),
            prompt: "Check your email.".into(),
            expires_ms: 300_000,
        };
        assert_eq!(p.challenge_type(), MfaChallengeType::Email);
        assert!(p.verify("ABC123"));
        assert!(!p.verify("wrong"));
    }
}
