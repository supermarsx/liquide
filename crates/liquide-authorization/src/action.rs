use serde::{Deserialize, Serialize};

use crate::level::AuthLevel;

/// A privileged action that may require authorization before it can proceed.
///
/// Actions are identified by reverse-domain-style IDs such as
/// `"org.liquide.system.shutdown"` or `"org.liquide.package.install"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationAction {
    /// Reverse-domain-style identifier for this action
    /// (e.g., `"org.liquide.system.shutdown"`).
    pub id: String,

    /// Human-readable description of what the action does.
    pub description: String,

    /// The prompt message shown to the user when authorization is requested.
    pub message: String,

    /// Optional icon name to display in the authorization dialog.
    pub icon: Option<String>,

    /// The minimum authentication level required by this action.
    pub required_level: AuthLevel,
}

impl AuthorizationAction {
    /// Create a new authorization action.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        message: impl Into<String>,
        required_level: AuthLevel,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            message: message.into(),
            icon: None,
            required_level,
        }
    }

    /// Set the icon for this action.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

impl std::fmt::Display for AuthorizationAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.id, self.required_level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_action() {
        let action = AuthorizationAction::new(
            "org.liquide.test",
            "Test action",
            "Please authenticate to test",
            AuthLevel::UserPassword,
        );
        assert_eq!(action.id, "org.liquide.test");
        assert_eq!(action.description, "Test action");
        assert_eq!(action.message, "Please authenticate to test");
        assert!(action.icon.is_none());
        assert_eq!(action.required_level, AuthLevel::UserPassword);
    }

    #[test]
    fn with_icon() {
        let action = AuthorizationAction::new(
            "org.liquide.test",
            "Test",
            "Authenticate",
            AuthLevel::NoAuth,
        )
        .with_icon("dialog-password");
        assert_eq!(action.icon.as_deref(), Some("dialog-password"));
    }

    #[test]
    fn display() {
        let action = AuthorizationAction::new(
            "org.liquide.system.shutdown",
            "Shutdown",
            "Shut down the system",
            AuthLevel::NoAuth,
        );
        assert_eq!(
            action.to_string(),
            "org.liquide.system.shutdown (No authentication)"
        );
    }

    #[test]
    fn serde_roundtrip() {
        let action = AuthorizationAction::new(
            "org.liquide.package.install",
            "Install package",
            "Authenticate to install software",
            AuthLevel::AdminPassword,
        )
        .with_icon("package-install");

        let json = serde_json::to_string(&action).unwrap();
        let back: AuthorizationAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, back);
    }
}
