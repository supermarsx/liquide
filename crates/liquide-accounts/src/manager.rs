//! High-level user manager that wraps a platform backend and
//! enforces password policy.

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::{LoginEntry, LoginHistory};
use crate::password::{PasswordPolicy, PasswordStrength};
use crate::platform::PlatformBackend;
use crate::types::{AccountType, UserAccount};

use liquide_authz_runtime::{AuthorizationRuntime, Resource, Subject};

/// Optional authorization enforcement for privileged account mutations.
///
/// When a `UserManager` carries one of these (via
/// [`UserManager::with_enforcement`]), destructive / system-state account
/// operations (`create_user`, `delete_user`, `change_password`) are gated
/// through the canonical [`AuthorizationRuntime`] facade and fail closed: any
/// outcome other than `Granted` returns [`AccountError::PermissionDenied`]
/// and the underlying mutation is NOT performed.
///
/// When no enforcement is configured (the default), behaviour is unchanged —
/// existing callers that have not yet been wired to a runtime keep working.
struct Enforcement {
    /// The canonical authorization facade (agent + audit + event sink).
    runtime: AuthorizationRuntime,
    /// The subject (who is requesting) attributed to every gated mutation.
    subject: Subject,
}

/// High-level account manager.
///
/// Wraps a [`PlatformBackend`] and adds password-policy enforcement,
/// username validation, and login history tracking.
pub struct UserManager {
    backend: Box<dyn PlatformBackend>,
    password_policy: PasswordPolicy,
    login_history: LoginHistory,
    /// Optional authorization enforcement for privileged mutations.
    ///
    /// `None` → no gating (legacy behaviour). `Some` → destructive account
    /// ops are gated fail-closed through the [`AuthorizationRuntime`] facade.
    enforcement: Option<Enforcement>,
}

impl UserManager {
    /// Create a `UserManager` with the given backend and default password policy.
    pub fn new(backend: Box<dyn PlatformBackend>) -> Self {
        Self {
            backend,
            password_policy: PasswordPolicy::default(),
            login_history: LoginHistory::new(),
            enforcement: None,
        }
    }

    /// Create a `UserManager` using the platform's default backend.
    pub fn with_default_backend() -> Self {
        Self::new(Box::new(crate::platform::DefaultBackend::default()))
    }

    /// Attach authorization enforcement to this manager.
    ///
    /// Once attached, the destructive account mutations (`create_user`,
    /// `delete_user`, `change_password`) are gated through the canonical
    /// [`AuthorizationRuntime`] facade and fail closed: if authorization is not
    /// `Granted`, the method returns [`AccountError::PermissionDenied`] and the
    /// mutation is not performed. `subject` identifies the principal (who is
    /// requesting); it is recorded in the audit trail for every gated op.
    #[must_use]
    pub fn with_enforcement(mut self, runtime: AuthorizationRuntime, subject: Subject) -> Self {
        self.enforcement = Some(Enforcement { runtime, subject });
        self
    }

    /// Gate a privileged mutation through the authorization facade.
    ///
    /// Fail-closed: returns `Err(AccountError::PermissionDenied)` if a runtime
    /// is configured and the decision is anything other than `Granted`. Returns
    /// `Ok(())` (allowing the mutation) when no enforcement is configured, or
    /// when the runtime grants the request. `resource` scopes the audit entry
    /// to the target user.
    fn enforce(&mut self, action_id: &str, resource: Option<Resource>) -> Result<(), AccountError> {
        if let Some(enforcement) = self.enforcement.as_mut() {
            let result =
                enforcement
                    .runtime
                    .authorize(action_id, &enforcement.subject, resource.as_ref());
            if !result.is_granted() {
                return Err(AccountError::PermissionDenied);
            }
        }
        Ok(())
    }

    /// Get a reference to the current password policy.
    pub fn password_policy(&self) -> &PasswordPolicy {
        &self.password_policy
    }

    /// Set a custom password policy.
    pub fn set_password_policy(&mut self, policy: PasswordPolicy) {
        self.password_policy = policy;
    }

    /// Get a reference to the login history.
    pub fn login_history(&self) -> &LoginHistory {
        &self.login_history
    }

    /// Get a mutable reference to the login history.
    pub fn login_history_mut(&mut self) -> &mut LoginHistory {
        &mut self.login_history
    }

    /// Check the strength of a password against the current policy.
    pub fn check_password_strength(&self, password: &str) -> PasswordStrength {
        self.password_policy.strength(password)
    }

    /// Validate a username for correctness.
    pub fn validate_username(username: &str) -> Result<(), AccountError> {
        if username.is_empty() {
            return Err(AccountError::InvalidUsername(
                "username cannot be empty".to_string(),
            ));
        }
        if username.len() > 32 {
            return Err(AccountError::InvalidUsername(
                "username too long (max 32 characters)".to_string(),
            ));
        }
        if !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(AccountError::InvalidUsername(
                "username contains invalid characters (allowed: a-z, A-Z, 0-9, _, -, .)"
                    .to_string(),
            ));
        }
        let first = username.chars().next().unwrap();
        if !first.is_ascii_lowercase() && first != '_' {
            return Err(AccountError::InvalidUsername(
                "username must start with a lowercase letter or underscore".to_string(),
            ));
        }
        Ok(())
    }

    // ── Delegating methods ──────────────────────────────────────────

    /// Return the currently logged-in user.
    pub fn current_user(&self) -> Result<UserAccount, AccountError> {
        self.backend.current_user()
    }

    /// List all human user accounts.
    pub fn list_users(&self) -> Result<Vec<UserAccount>, AccountError> {
        self.backend.list_users()
    }

    /// Create a new user account, enforcing password policy.
    pub fn create_user(
        &mut self,
        username: &str,
        display_name: &str,
        account_type: AccountType,
        password: &str,
    ) -> Result<UserAccount, AccountError> {
        // Authorization gate (fail-closed): a non-Granted decision blocks the
        // mutation entirely.
        self.enforce(
            "accounts.create_user",
            Some(Resource::new(0, format!("user:{username}"))),
        )?;

        // Validate username.
        Self::validate_username(username)?;

        // Enforce password policy.
        if let Err(violations) = self.password_policy.check(password) {
            return Err(AccountError::WeakPassword(violations.join("; ")));
        }

        self.backend
            .create_user(username, display_name, account_type, password)
    }

    /// Delete a user account.
    pub fn delete_user(&mut self, uid: u32, delete_home: bool) -> Result<(), AccountError> {
        // Authorization gate (fail-closed): a non-Granted decision blocks the
        // deletion.
        self.enforce(
            "accounts.delete_user",
            Some(Resource::new(uid, format!("user:{uid}"))),
        )?;

        self.backend.delete_user(uid, delete_home)
    }

    /// Change a user's display name.
    pub fn set_display_name(&mut self, uid: u32, name: &str) -> Result<(), AccountError> {
        self.backend.set_display_name(uid, name)
    }

    /// Set a user's avatar image.
    pub fn set_avatar(&mut self, uid: u32, path: &str) -> Result<(), AccountError> {
        self.backend.set_avatar(uid, path)
    }

    /// Change a user's password, enforcing the password policy on the new password.
    pub fn change_password(
        &mut self,
        uid: u32,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AccountError> {
        // Authorization gate (fail-closed): a non-Granted decision blocks the
        // password change.
        self.enforce(
            "accounts.change_password",
            Some(Resource::new(uid, format!("user:{uid}"))),
        )?;

        if old_password.is_empty() {
            return Err(AccountError::PlatformError(
                "old password must not be empty".into(),
            ));
        }

        // Enforce password policy on new password.
        if let Err(violations) = self.password_policy.check(new_password) {
            return Err(AccountError::WeakPassword(violations.join("; ")));
        }

        self.backend
            .change_password(uid, old_password, new_password)
    }

    /// Enable or disable auto-login for a user.
    pub fn set_auto_login(&mut self, uid: u32, enabled: bool) -> Result<(), AccountError> {
        self.backend.set_auto_login(uid, enabled)
    }

    /// Lock (disable) a user account.
    pub fn lock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        self.backend.lock_account(uid)
    }

    /// Unlock (re-enable) a user account.
    pub fn unlock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        self.backend.unlock_account(uid)
    }

    /// Change a user's account type.
    pub fn set_account_type(
        &mut self,
        uid: u32,
        account_type: AccountType,
    ) -> Result<(), AccountError> {
        self.backend.set_account_type(uid, account_type)
    }

    /// List all groups.
    pub fn list_groups(&self) -> Result<Vec<Group>, AccountError> {
        self.backend.list_groups()
    }

    /// Return the groups that a specific user belongs to.
    pub fn user_groups(&self, uid: u32) -> Result<Vec<Group>, AccountError> {
        self.backend.user_groups(uid)
    }

    /// Add a user to a group.
    pub fn add_to_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        self.backend.add_to_group(uid, gid)
    }

    /// Remove a user from a group.
    pub fn remove_from_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        self.backend.remove_from_group(uid, gid)
    }

    /// Retrieve recent login entries for a user.
    ///
    /// Tries the platform backend first; falls back to the in-memory
    /// login history if the platform returns nothing.
    pub fn recent_logins(&self, uid: u32, count: usize) -> Vec<LoginEntry> {
        // Try platform first.
        if let Ok(entries) = self.backend.recent_logins(uid, count) {
            if !entries.is_empty() {
                return entries;
            }
        }
        // Fall back to in-memory history.
        self.login_history.recent_logins(uid, count)
    }
}
