use serde::{Deserialize, Serialize};

/// The level of authentication required for a privileged action.
///
/// Levels are ordered from least to most restrictive. A policy rule
/// specifies which level is required, and the authorization agent
/// will prompt accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AuthLevel {
    /// No authentication required; the action is always allowed.
    NoAuth = 0,
    /// The user must provide their own password.
    UserPassword = 1,
    /// The user must provide an administrator/root password.
    AdminPassword = 2,
    /// The user must authenticate via fingerprint reader.
    Fingerprint = 3,
    /// The user must authenticate via smart card (PIV/PKCS#11).
    SmartCard = 4,
}

impl AuthLevel {
    /// Returns a human-readable label for the auth level.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::NoAuth => "No authentication",
            Self::UserPassword => "User password",
            Self::AdminPassword => "Administrator password",
            Self::Fingerprint => "Fingerprint",
            Self::SmartCard => "Smart card",
        }
    }

    /// Returns true if this level requires some form of credential input.
    #[must_use]
    pub fn requires_credential(&self) -> bool {
        !matches!(self, Self::NoAuth)
    }
}

impl std::fmt::Display for AuthLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering() {
        assert!(AuthLevel::NoAuth < AuthLevel::UserPassword);
        assert!(AuthLevel::UserPassword < AuthLevel::AdminPassword);
        assert!(AuthLevel::AdminPassword < AuthLevel::Fingerprint);
        assert!(AuthLevel::Fingerprint < AuthLevel::SmartCard);
    }

    #[test]
    fn requires_credential() {
        assert!(!AuthLevel::NoAuth.requires_credential());
        assert!(AuthLevel::UserPassword.requires_credential());
        assert!(AuthLevel::AdminPassword.requires_credential());
        assert!(AuthLevel::Fingerprint.requires_credential());
        assert!(AuthLevel::SmartCard.requires_credential());
    }

    #[test]
    fn display() {
        assert_eq!(AuthLevel::NoAuth.to_string(), "No authentication");
        assert_eq!(AuthLevel::AdminPassword.to_string(), "Administrator password");
    }

    #[test]
    fn label_matches_display() {
        for level in [
            AuthLevel::NoAuth,
            AuthLevel::UserPassword,
            AuthLevel::AdminPassword,
            AuthLevel::Fingerprint,
            AuthLevel::SmartCard,
        ] {
            assert_eq!(level.label(), level.to_string());
        }
    }

    #[test]
    fn serde_roundtrip() {
        let level = AuthLevel::AdminPassword;
        let json = serde_json::to_string(&level).unwrap();
        let back: AuthLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(level, back);
    }
}
