//! Core account types.

use std::fmt;

/// The privilege level of a user account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountType {
    /// A regular (non-privileged) user.
    Standard,
    /// A user with administrative (root/sudo) privileges.
    Administrator,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard => write!(f, "Standard"),
            Self::Administrator => write!(f, "Administrator"),
        }
    }
}

impl Default for AccountType {
    fn default() -> Self {
        Self::Standard
    }
}

/// A system user account.
#[derive(Debug, Clone)]
pub struct UserAccount {
    /// Numeric user identifier (UID on Unix, RID on Windows).
    pub uid: u32,
    /// The login name (e.g. `"alice"`).
    pub username: String,
    /// Human-readable display name (e.g. `"Alice Smith"`).
    pub display_name: String,
    /// Absolute path to the user's home directory.
    pub home_dir: String,
    /// Login shell (e.g. `"/bin/bash"`). On Windows this is typically
    /// `"cmd.exe"` or `"powershell.exe"`.
    pub shell: String,
    /// Whether this is a standard or administrative user.
    pub account_type: AccountType,
    /// Optional path to the user's avatar image.
    pub avatar: Option<String>,
    /// Whether the user is currently logged in.
    pub is_logged_in: bool,
    /// Whether the account is locked (login disabled).
    pub is_locked: bool,
    /// Unix timestamp of the last password change, if known.
    pub password_last_changed: Option<u64>,
    /// Whether the user should be logged in automatically at boot.
    pub auto_login: bool,
}

impl UserAccount {
    /// Returns `true` if this account has administrative privileges.
    pub fn is_admin(&self) -> bool {
        self.account_type == AccountType::Administrator
    }
}

impl fmt::Display for UserAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (uid={}, {})",
            self.username, self.uid, self.account_type
        )
    }
}
