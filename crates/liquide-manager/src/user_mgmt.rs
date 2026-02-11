//! User management operations.

use serde::{Deserialize, Serialize};

use crate::config::AdminRole;

/// User summary for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub username: String,
    pub active_sessions: u32,
    pub last_login: Option<u64>,
    pub policy_group: String,
    pub locked: bool,
}

/// Detailed user information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDetail {
    pub summary: UserSummary,
    pub session_ids: Vec<String>,
    pub effective_policies: Vec<String>,
}

/// Admin account record.
#[derive(Debug, Clone)]
pub struct AdminAccount {
    pub username: String,
    pub role: AdminRole,
    pub locked: bool,
    pub login_failures: u32,
    pub last_login: Option<u64>,
    pub lockout_until: Option<u64>,
}

/// Admin account store.
pub struct AdminStore {
    accounts: Vec<AdminAccount>,
}

impl AdminStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }

    /// Add an admin account.
    pub fn add(&mut self, username: String, role: AdminRole) {
        if !self.accounts.iter().any(|a| a.username == username) {
            self.accounts.push(AdminAccount {
                username,
                role,
                locked: false,
                login_failures: 0,
                last_login: None,
                lockout_until: None,
            });
        }
    }

    /// Authenticate an admin (stub — real impl would check password hash).
    pub fn authenticate(&mut self, username: &str, now: u64) -> crate::Result<&AdminAccount> {
        let account = self
            .accounts
            .iter_mut()
            .find(|a| a.username == username)
            .ok_or_else(|| crate::ManagerError::AuthenticationFailed {
                reason: "unknown user".to_string(),
            })?;

        if account.locked {
            return Err(crate::ManagerError::AuthenticationFailed {
                reason: "account locked".to_string(),
            });
        }

        if let Some(until) = account.lockout_until {
            if now < until {
                return Err(crate::ManagerError::AuthenticationFailed {
                    reason: format!("locked out until {until}"),
                });
            }
            account.lockout_until = None;
            account.login_failures = 0;
        }

        account.last_login = Some(now);
        account.login_failures = 0;

        Ok(account)
    }

    /// Record a failed login attempt. Returns true if account is now locked out.
    pub fn record_failure(&mut self, username: &str, max_attempts: u32, lockout_sec: u64, now: u64) -> bool {
        if let Some(account) = self.accounts.iter_mut().find(|a| a.username == username) {
            account.login_failures += 1;
            if account.login_failures >= max_attempts {
                account.lockout_until = Some(now + lockout_sec);
                account.login_failures = 0;
                return true;
            }
        }
        false
    }

    /// Change a user's role.
    pub fn set_role(&mut self, username: &str, role: AdminRole) -> crate::Result<()> {
        let account = self
            .accounts
            .iter_mut()
            .find(|a| a.username == username)
            .ok_or_else(|| crate::ManagerError::UserNotFound {
                username: username.to_string(),
            })?;
        account.role = role;
        Ok(())
    }

    /// Lock an account.
    pub fn lock(&mut self, username: &str) -> crate::Result<()> {
        let account = self
            .accounts
            .iter_mut()
            .find(|a| a.username == username)
            .ok_or_else(|| crate::ManagerError::UserNotFound {
                username: username.to_string(),
            })?;
        account.locked = true;
        Ok(())
    }

    /// Unlock an account.
    pub fn unlock(&mut self, username: &str) -> crate::Result<()> {
        let account = self
            .accounts
            .iter_mut()
            .find(|a| a.username == username)
            .ok_or_else(|| crate::ManagerError::UserNotFound {
                username: username.to_string(),
            })?;
        account.locked = false;
        account.lockout_until = None;
        account.login_failures = 0;
        Ok(())
    }

    /// Get an account by username.
    #[must_use]
    pub fn get(&self, username: &str) -> Option<&AdminAccount> {
        self.accounts.iter().find(|a| a.username == username)
    }

    /// List all accounts.
    #[must_use]
    pub fn list(&self) -> &[AdminAccount] {
        &self.accounts
    }

    /// Count of admin accounts.
    #[must_use]
    pub fn count(&self) -> usize {
        self.accounts.len()
    }
}

impl Default for AdminStore {
    fn default() -> Self {
        Self::new()
    }
}
