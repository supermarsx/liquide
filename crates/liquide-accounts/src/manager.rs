//! High-level user manager that wraps a platform backend and
//! enforces password policy.

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::{LoginEntry, LoginHistory};
use crate::password::{PasswordPolicy, PasswordStrength};
use crate::platform::PlatformBackend;
use crate::types::{AccountType, UserAccount};

/// High-level account manager.
///
/// Wraps a [`PlatformBackend`] and adds password-policy enforcement,
/// username validation, and login history tracking.
pub struct UserManager {
    backend: Box<dyn PlatformBackend>,
    password_policy: PasswordPolicy,
    login_history: LoginHistory,
}

impl UserManager {
    /// Create a `UserManager` with the given backend and default password policy.
    pub fn new(backend: Box<dyn PlatformBackend>) -> Self {
        Self {
            backend,
            password_policy: PasswordPolicy::default(),
            login_history: LoginHistory::new(),
        }
    }

    /// Create a `UserManager` using the platform's default backend.
    pub fn with_default_backend() -> Self {
        Self::new(Box::new(crate::platform::DefaultBackend::default()))
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
