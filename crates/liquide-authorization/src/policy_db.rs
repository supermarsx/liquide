//! Extended authorization policy database with rich policy entries,
//! implied-authorization semantics, and decision evaluation.
//!
//! This module complements the existing `policy` module (which stores
//! simple action-pattern -> auth-level rules) by adding a full policy
//! entry model inspired by desktop authorization frameworks:
//!
//! - [`ActionId`] — typed, validated reverse-domain action identifier
//! - [`PolicyEntry`] — rich descriptor per action (description, message,
//!   icon, defaults for different contexts)
//! - [`ImpliedAuth`] — what happens by default (allow, deny, prompt)
//! - [`AuthDecision`] — the outcome of evaluating a policy
//! - [`PolicyDatabase`] — lookup table keyed by `ActionId`

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::level::AuthLevel;
use crate::subject::Subject;

// ── ActionId ────────────────────────────────────────────────────────

/// A validated reverse-domain action identifier such as
/// `"org.liquide.settings.write"`.
///
/// Action IDs must:
/// - Contain at least two dot-separated segments
/// - Use only ASCII alphanumeric characters, hyphens, and dots
/// - Not start or end with a dot
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(String);

impl ActionId {
    /// Create a new `ActionId`, returning `None` if the string is invalid.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Option<Self> {
        let s: String = id.into();
        if Self::validate(&s) {
            Some(Self(s))
        } else {
            None
        }
    }

    /// Create an `ActionId` without validation.
    ///
    /// # Safety (logical)
    /// The caller must guarantee the string is a valid dotted identifier.
    #[must_use]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Return the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate a candidate action-id string.
    fn validate(s: &str) -> bool {
        if s.is_empty() || s.starts_with('.') || s.ends_with('.') {
            return false;
        }
        let segments: Vec<&str> = s.split('.').collect();
        if segments.len() < 2 {
            return false;
        }
        for seg in &segments {
            if seg.is_empty() {
                return false;
            }
            if !seg
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return false;
            }
        }
        true
    }
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ActionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ── ImpliedAuth ─────────────────────────────────────────────────────

/// The default authorization outcome for an action when no explicit
/// rule overrides it.
///
/// These semantics are analogous to PolicyKit's `defaults` stanza:
///
/// - `No` — deny without prompting
/// - `Yes` — allow without prompting
/// - `AdminAuth` — require administrator credentials
/// - `UserAuth` — require the active user's credentials
/// - `AdminKeep` — require admin credentials, then cache the grant
/// - `UserKeep` — require user credentials, then cache the grant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImpliedAuth {
    /// Always deny.
    No,
    /// Always allow.
    Yes,
    /// Require administrator authentication (one-shot).
    AdminAuth,
    /// Require active-user authentication (one-shot).
    UserAuth,
    /// Require administrator authentication, cache result.
    AdminKeep,
    /// Require active-user authentication, cache result.
    UserKeep,
}

impl ImpliedAuth {
    /// Map the implied-auth to an [`AuthDecision`].
    #[must_use]
    pub fn to_decision(self) -> AuthDecision {
        match self {
            Self::No => AuthDecision::Deny,
            Self::Yes => AuthDecision::Allow,
            Self::AdminAuth | Self::AdminKeep => {
                AuthDecision::AuthRequired(AuthType::AdminPassword)
            }
            Self::UserAuth | Self::UserKeep => {
                AuthDecision::AuthRequired(AuthType::UserPassword)
            }
        }
    }

    /// Whether a successful grant should be cached (keep-alive).
    #[must_use]
    pub fn is_keep(self) -> bool {
        matches!(self, Self::AdminKeep | Self::UserKeep)
    }

    /// Whether this implied-auth requires some credential input.
    #[must_use]
    pub fn requires_auth(self) -> bool {
        !matches!(self, Self::No | Self::Yes)
    }
}

// ── AuthType ────────────────────────────────────────────────────────

/// The kind of authentication to request from the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthType {
    /// User must supply their own password.
    UserPassword,
    /// User must supply an administrator password.
    AdminPassword,
    /// User must authenticate via fingerprint.
    Fingerprint,
    /// User must authenticate via smart card.
    SmartCard,
}

impl AuthType {
    /// Convert to the crate-level [`AuthLevel`].
    #[must_use]
    pub fn to_auth_level(self) -> AuthLevel {
        match self {
            Self::UserPassword => AuthLevel::UserPassword,
            Self::AdminPassword => AuthLevel::AdminPassword,
            Self::Fingerprint => AuthLevel::Fingerprint,
            Self::SmartCard => AuthLevel::SmartCard,
        }
    }
}

// ── AuthDecision ────────────────────────────────────────────────────

/// The outcome of evaluating an authorization request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthDecision {
    /// The action is allowed without further interaction.
    Allow,
    /// The action is denied.
    Deny,
    /// The action requires authentication of the given type.
    AuthRequired(AuthType),
}

impl AuthDecision {
    #[must_use]
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow)
    }

    #[must_use]
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny)
    }

    #[must_use]
    pub fn is_auth_required(&self) -> bool {
        matches!(self, Self::AuthRequired(_))
    }
}

// ── PolicyEntry ─────────────────────────────────────────────────────

/// A rich policy descriptor for a single action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyEntry {
    /// The action this entry governs.
    pub action_id: ActionId,

    /// Human-readable description.
    pub description: String,

    /// Prompt message shown in the authentication dialog.
    pub message: String,

    /// Optional icon name for the dialog.
    pub icon: Option<String>,

    /// Default implied-auth for any active session.
    pub default_any: ImpliedAuth,

    /// Default implied-auth for a local (physically present) session.
    pub default_active: ImpliedAuth,

    /// Default implied-auth for an administrator.
    pub default_admin: ImpliedAuth,
}

impl PolicyEntry {
    /// Create a new policy entry with uniform defaults.
    #[must_use]
    pub fn new(
        action_id: ActionId,
        description: impl Into<String>,
        message: impl Into<String>,
        default: ImpliedAuth,
    ) -> Self {
        Self {
            action_id,
            description: description.into(),
            message: message.into(),
            icon: None,
            default_any: default,
            default_active: default,
            default_admin: ImpliedAuth::Yes,
        }
    }

    /// Set the icon.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Override the admin default.
    #[must_use]
    pub fn with_admin_default(mut self, auth: ImpliedAuth) -> Self {
        self.default_admin = auth;
        self
    }

    /// Override the active-session default.
    #[must_use]
    pub fn with_active_default(mut self, auth: ImpliedAuth) -> Self {
        self.default_active = auth;
        self
    }

    /// Override the any-session default.
    #[must_use]
    pub fn with_any_default(mut self, auth: ImpliedAuth) -> Self {
        self.default_any = auth;
        self
    }
}

// ── PolicyDatabase ──────────────────────────────────────────────────

/// A database of [`PolicyEntry`] values, keyed by [`ActionId`].
#[derive(Debug, Clone, Default)]
pub struct PolicyDatabase {
    entries: HashMap<ActionId, PolicyEntry>,
}

impl PolicyDatabase {
    /// Create an empty database.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Create a database pre-loaded with the built-in desktop policies.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut db = Self::new();
        for entry in builtin_policy_entries() {
            db.insert(entry);
        }
        db
    }

    /// Insert a policy entry, replacing any previous entry with the same ID.
    pub fn insert(&mut self, entry: PolicyEntry) {
        self.entries.insert(entry.action_id.clone(), entry);
    }

    /// Look up a policy entry by action ID.
    #[must_use]
    pub fn lookup(&self, action_id: &ActionId) -> Option<&PolicyEntry> {
        self.entries.get(action_id)
    }

    /// Look up by string, creating a temporary `ActionId`.
    #[must_use]
    pub fn lookup_str(&self, action_id: &str) -> Option<&PolicyEntry> {
        let key = ActionId::new_unchecked(action_id);
        self.entries.get(&key)
    }

    /// Remove a policy entry.
    pub fn remove(&mut self, action_id: &ActionId) -> Option<PolicyEntry> {
        self.entries.remove(action_id)
    }

    /// Return the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if the database is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&ActionId, &PolicyEntry)> {
        self.entries.iter()
    }

    /// Evaluate an authorization request against this database.
    ///
    /// Decision logic:
    /// 1. Look up the entry for `action_id`.
    /// 2. If the subject is an admin, use `default_admin`.
    /// 3. Otherwise, use `default_active` (local session) or
    ///    `default_any` (remote/unknown).
    /// 4. Return the corresponding [`AuthDecision`].
    ///
    /// Returns `AuthDecision::Deny` if no entry exists.
    #[must_use]
    pub fn evaluate(&self, action_id: &ActionId, subject: &Subject) -> AuthDecision {
        let entry = match self.lookup(action_id) {
            Some(e) => e,
            None => return AuthDecision::Deny,
        };

        let implied = if crate::subject::is_admin(subject) {
            entry.default_admin
        } else if subject.is_local_session() {
            entry.default_active
        } else {
            entry.default_any
        };

        implied.to_decision()
    }
}

/// Built-in policy entries for common desktop actions.
#[must_use]
fn builtin_policy_entries() -> Vec<PolicyEntry> {
    vec![
        // ── Desktop personalization (low privilege) ────────────────
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.desktop.change-wallpaper"),
            "Change the desktop wallpaper",
            "Authentication is required to change the wallpaper.",
            ImpliedAuth::Yes,
        )
        .with_icon("preferences-desktop-wallpaper"),
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.desktop.change-theme"),
            "Change the desktop theme",
            "Authentication is required to change the theme.",
            ImpliedAuth::Yes,
        )
        .with_icon("preferences-desktop-theme"),
        // ── Settings (user-level) ──────────────────────────────────
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.settings.read"),
            "Read system settings",
            "Authentication is required to view settings.",
            ImpliedAuth::Yes,
        )
        .with_icon("preferences-system"),
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.settings.write"),
            "Modify system settings",
            "Authentication is required to change settings.",
            ImpliedAuth::UserAuth,
        )
        .with_icon("preferences-system")
        .with_active_default(ImpliedAuth::UserKeep),
        // ── Package management (admin) ─────────────────────────────
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.package.install"),
            "Install software packages",
            "Authentication is required to install software.",
            ImpliedAuth::AdminAuth,
        )
        .with_icon("package-install")
        .with_active_default(ImpliedAuth::AdminKeep),
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.package.remove"),
            "Remove software packages",
            "Authentication is required to remove software.",
            ImpliedAuth::AdminAuth,
        )
        .with_icon("package-remove")
        .with_active_default(ImpliedAuth::AdminKeep),
        // ── Service management (admin) ─────────────────────────────
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.service.start"),
            "Start a system service",
            "Authentication is required to start a system service.",
            ImpliedAuth::AdminAuth,
        )
        .with_icon("system-run"),
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.service.stop"),
            "Stop a system service",
            "Authentication is required to stop a system service.",
            ImpliedAuth::AdminAuth,
        )
        .with_icon("process-stop"),
        // ── User management (admin) ────────────────────────────────
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.users.manage"),
            "Manage user accounts",
            "Authentication is required to manage user accounts.",
            ImpliedAuth::AdminAuth,
        )
        .with_icon("system-users")
        .with_admin_default(ImpliedAuth::AdminKeep),
        // ── Device mounting (user) ─────────────────────────────────
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.device.mount"),
            "Mount a storage device",
            "Authentication is required to mount a device.",
            ImpliedAuth::UserAuth,
        )
        .with_icon("drive-harddisk")
        .with_active_default(ImpliedAuth::Yes),
        // ── System power (no auth for console user) ────────────────
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.system.shutdown"),
            "Shut down the system",
            "The system will shut down.",
            ImpliedAuth::UserAuth,
        )
        .with_icon("system-shutdown")
        .with_active_default(ImpliedAuth::Yes)
        .with_admin_default(ImpliedAuth::Yes),
        PolicyEntry::new(
            ActionId::new_unchecked("org.liquide.system.reboot"),
            "Restart the system",
            "The system will restart.",
            ImpliedAuth::UserAuth,
        )
        .with_icon("system-reboot")
        .with_active_default(ImpliedAuth::Yes)
        .with_admin_default(ImpliedAuth::Yes),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subject::Subject;

    // ── ActionId tests ──────────────────────────────────────────────

    #[test]
    fn action_id_valid() {
        assert!(ActionId::new("org.liquide.settings.write").is_some());
        assert!(ActionId::new("com.example.test").is_some());
        assert!(ActionId::new("a.b").is_some());
        assert!(ActionId::new("org.liquide-desktop.app_launch").is_some());
    }

    #[test]
    fn action_id_invalid_single_segment() {
        assert!(ActionId::new("nosegment").is_none());
    }

    #[test]
    fn action_id_invalid_empty() {
        assert!(ActionId::new("").is_none());
    }

    #[test]
    fn action_id_invalid_leading_dot() {
        assert!(ActionId::new(".org.liquide").is_none());
    }

    #[test]
    fn action_id_invalid_trailing_dot() {
        assert!(ActionId::new("org.liquide.").is_none());
    }

    #[test]
    fn action_id_invalid_double_dot() {
        assert!(ActionId::new("org..liquide").is_none());
    }

    #[test]
    fn action_id_invalid_special_chars() {
        assert!(ActionId::new("org.liquide.foo bar").is_none());
        assert!(ActionId::new("org.liquide.foo/bar").is_none());
    }

    #[test]
    fn action_id_display() {
        let id = ActionId::new("org.liquide.test").unwrap();
        assert_eq!(id.to_string(), "org.liquide.test");
    }

    #[test]
    fn action_id_as_str() {
        let id = ActionId::new("org.liquide.test").unwrap();
        assert_eq!(id.as_str(), "org.liquide.test");
    }

    #[test]
    fn action_id_unchecked() {
        let id = ActionId::new_unchecked("anything");
        assert_eq!(id.as_str(), "anything");
    }

    #[test]
    fn action_id_serde_roundtrip() {
        let id = ActionId::new("org.liquide.settings.write").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let back: ActionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn action_id_hash_eq() {
        let a = ActionId::new("org.liquide.test").unwrap();
        let b = ActionId::new("org.liquide.test").unwrap();
        let mut set = std::collections::HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
    }

    // ── ImpliedAuth tests ───────────────────────────────────────────

    #[test]
    fn implied_auth_to_decision() {
        assert_eq!(ImpliedAuth::No.to_decision(), AuthDecision::Deny);
        assert_eq!(ImpliedAuth::Yes.to_decision(), AuthDecision::Allow);
        assert_eq!(
            ImpliedAuth::AdminAuth.to_decision(),
            AuthDecision::AuthRequired(AuthType::AdminPassword)
        );
        assert_eq!(
            ImpliedAuth::UserAuth.to_decision(),
            AuthDecision::AuthRequired(AuthType::UserPassword)
        );
        assert_eq!(
            ImpliedAuth::AdminKeep.to_decision(),
            AuthDecision::AuthRequired(AuthType::AdminPassword)
        );
        assert_eq!(
            ImpliedAuth::UserKeep.to_decision(),
            AuthDecision::AuthRequired(AuthType::UserPassword)
        );
    }

    #[test]
    fn implied_auth_is_keep() {
        assert!(!ImpliedAuth::No.is_keep());
        assert!(!ImpliedAuth::Yes.is_keep());
        assert!(!ImpliedAuth::AdminAuth.is_keep());
        assert!(!ImpliedAuth::UserAuth.is_keep());
        assert!(ImpliedAuth::AdminKeep.is_keep());
        assert!(ImpliedAuth::UserKeep.is_keep());
    }

    #[test]
    fn implied_auth_requires_auth() {
        assert!(!ImpliedAuth::No.requires_auth());
        assert!(!ImpliedAuth::Yes.requires_auth());
        assert!(ImpliedAuth::AdminAuth.requires_auth());
        assert!(ImpliedAuth::UserAuth.requires_auth());
        assert!(ImpliedAuth::AdminKeep.requires_auth());
        assert!(ImpliedAuth::UserKeep.requires_auth());
    }

    // ── AuthDecision tests ──────────────────────────────────────────

    #[test]
    fn auth_decision_predicates() {
        assert!(AuthDecision::Allow.is_allow());
        assert!(!AuthDecision::Allow.is_deny());
        assert!(!AuthDecision::Allow.is_auth_required());

        assert!(AuthDecision::Deny.is_deny());
        assert!(!AuthDecision::Deny.is_allow());

        let req = AuthDecision::AuthRequired(AuthType::UserPassword);
        assert!(req.is_auth_required());
        assert!(!req.is_allow());
        assert!(!req.is_deny());
    }

    // ── AuthType tests ──────────────────────────────────────────────

    #[test]
    fn auth_type_to_auth_level() {
        assert_eq!(AuthType::UserPassword.to_auth_level(), AuthLevel::UserPassword);
        assert_eq!(AuthType::AdminPassword.to_auth_level(), AuthLevel::AdminPassword);
        assert_eq!(AuthType::Fingerprint.to_auth_level(), AuthLevel::Fingerprint);
        assert_eq!(AuthType::SmartCard.to_auth_level(), AuthLevel::SmartCard);
    }

    // ── PolicyEntry tests ───────────────────────────────────────────

    #[test]
    fn policy_entry_new() {
        let id = ActionId::new("org.liquide.test.action").unwrap();
        let entry = PolicyEntry::new(
            id.clone(),
            "Test action",
            "Please authenticate",
            ImpliedAuth::UserAuth,
        );
        assert_eq!(entry.action_id, id);
        assert_eq!(entry.description, "Test action");
        assert_eq!(entry.message, "Please authenticate");
        assert!(entry.icon.is_none());
        assert_eq!(entry.default_any, ImpliedAuth::UserAuth);
        assert_eq!(entry.default_active, ImpliedAuth::UserAuth);
        assert_eq!(entry.default_admin, ImpliedAuth::Yes);
    }

    #[test]
    fn policy_entry_builders() {
        let id = ActionId::new("org.liquide.test.action").unwrap();
        let entry = PolicyEntry::new(id, "desc", "msg", ImpliedAuth::No)
            .with_icon("test-icon")
            .with_admin_default(ImpliedAuth::AdminAuth)
            .with_active_default(ImpliedAuth::UserKeep)
            .with_any_default(ImpliedAuth::UserAuth);
        assert_eq!(entry.icon.as_deref(), Some("test-icon"));
        assert_eq!(entry.default_admin, ImpliedAuth::AdminAuth);
        assert_eq!(entry.default_active, ImpliedAuth::UserKeep);
        assert_eq!(entry.default_any, ImpliedAuth::UserAuth);
    }

    #[test]
    fn policy_entry_serde_roundtrip() {
        let id = ActionId::new("org.liquide.test.action").unwrap();
        let entry = PolicyEntry::new(id, "desc", "msg", ImpliedAuth::AdminKeep)
            .with_icon("lock");
        let json = serde_json::to_string(&entry).unwrap();
        let back: PolicyEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    // ── PolicyDatabase tests ────────────────────────────────────────

    #[test]
    fn policy_db_empty() {
        let db = PolicyDatabase::new();
        assert!(db.is_empty());
        assert_eq!(db.len(), 0);
    }

    #[test]
    fn policy_db_insert_and_lookup() {
        let mut db = PolicyDatabase::new();
        let id = ActionId::new("org.liquide.test.action").unwrap();
        let entry = PolicyEntry::new(id.clone(), "desc", "msg", ImpliedAuth::Yes);
        db.insert(entry.clone());
        assert_eq!(db.len(), 1);

        let found = db.lookup(&id).unwrap();
        assert_eq!(found.description, "desc");
    }

    #[test]
    fn policy_db_lookup_str() {
        let mut db = PolicyDatabase::new();
        let id = ActionId::new("org.liquide.test.action").unwrap();
        db.insert(PolicyEntry::new(id, "desc", "msg", ImpliedAuth::Yes));

        assert!(db.lookup_str("org.liquide.test.action").is_some());
        assert!(db.lookup_str("org.liquide.missing").is_none());
    }

    #[test]
    fn policy_db_remove() {
        let mut db = PolicyDatabase::new();
        let id = ActionId::new("org.liquide.test.action").unwrap();
        db.insert(PolicyEntry::new(id.clone(), "desc", "msg", ImpliedAuth::Yes));
        assert_eq!(db.len(), 1);

        let removed = db.remove(&id);
        assert!(removed.is_some());
        assert!(db.is_empty());
    }

    #[test]
    fn policy_db_replace_on_insert() {
        let mut db = PolicyDatabase::new();
        let id = ActionId::new("org.liquide.test.action").unwrap();
        db.insert(PolicyEntry::new(id.clone(), "first", "msg", ImpliedAuth::Yes));
        db.insert(PolicyEntry::new(id.clone(), "second", "msg", ImpliedAuth::No));
        assert_eq!(db.len(), 1);
        assert_eq!(db.lookup(&id).unwrap().description, "second");
    }

    #[test]
    fn policy_db_with_builtins() {
        let db = PolicyDatabase::with_builtins();
        assert!(!db.is_empty());
        assert!(db.lookup_str("org.liquide.desktop.change-wallpaper").is_some());
        assert!(db.lookup_str("org.liquide.package.install").is_some());
        assert!(db.lookup_str("org.liquide.system.shutdown").is_some());
    }

    #[test]
    fn policy_db_iter() {
        let db = PolicyDatabase::with_builtins();
        let count = db.iter().count();
        assert_eq!(count, db.len());
    }

    // ── evaluate() tests ────────────────────────────────────────────

    #[test]
    fn evaluate_admin_subject_gets_admin_default() {
        let db = PolicyDatabase::with_builtins();
        let admin = Subject::new(0, 1000, "session-1")
            .with_group("admin")
            .with_group("wheel");
        let id = ActionId::new("org.liquide.package.install").unwrap();
        let decision = db.evaluate(&id, &admin);
        // Admin default for package.install is ImpliedAuth::Yes
        assert!(decision.is_allow());
    }

    #[test]
    fn evaluate_regular_user_local_gets_active_default() {
        let db = PolicyDatabase::with_builtins();
        let user = Subject::new(1000, 2000, "session-local").as_local();
        let id = ActionId::new("org.liquide.package.install").unwrap();
        let decision = db.evaluate(&id, &user);
        // Active default for package.install is AdminKeep -> AuthRequired(Admin)
        assert_eq!(
            decision,
            AuthDecision::AuthRequired(AuthType::AdminPassword)
        );
    }

    #[test]
    fn evaluate_remote_user_gets_any_default() {
        let db = PolicyDatabase::with_builtins();
        let user = Subject::new(1000, 3000, "session-remote");
        let id = ActionId::new("org.liquide.package.install").unwrap();
        let decision = db.evaluate(&id, &user);
        // Any default for package.install is AdminAuth -> AuthRequired(Admin)
        assert_eq!(
            decision,
            AuthDecision::AuthRequired(AuthType::AdminPassword)
        );
    }

    #[test]
    fn evaluate_unknown_action_denied() {
        let db = PolicyDatabase::with_builtins();
        let user = Subject::new(1000, 1234, "s");
        let id = ActionId::new("org.liquide.nonexistent.action").unwrap();
        assert!(db.evaluate(&id, &user).is_deny());
    }

    #[test]
    fn evaluate_wallpaper_always_allowed() {
        let db = PolicyDatabase::with_builtins();
        let user = Subject::new(1000, 5000, "s");
        let id = ActionId::new("org.liquide.desktop.change-wallpaper").unwrap();
        assert!(db.evaluate(&id, &user).is_allow());
    }

    #[test]
    fn evaluate_shutdown_local_user_allowed() {
        let db = PolicyDatabase::with_builtins();
        let user = Subject::new(1000, 6000, "s").as_local();
        let id = ActionId::new("org.liquide.system.shutdown").unwrap();
        assert!(db.evaluate(&id, &user).is_allow());
    }

    #[test]
    fn evaluate_shutdown_remote_user_needs_auth() {
        let db = PolicyDatabase::with_builtins();
        let user = Subject::new(1000, 7000, "s");
        let id = ActionId::new("org.liquide.system.shutdown").unwrap();
        let decision = db.evaluate(&id, &user);
        assert_eq!(
            decision,
            AuthDecision::AuthRequired(AuthType::UserPassword)
        );
    }
}
