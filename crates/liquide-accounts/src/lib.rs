//! User account management for the LiquiDE desktop environment.
//!
//! Provides user enumeration, creation, deletion, password policy
//! enforcement, group management, and login history through a
//! platform-abstracted `UserManager`.
//!
//! # Platform support
//!
//! - **Linux**: reads `/etc/passwd`, shells out to `useradd`/`userdel`/
//!   `usermod`/`passwd`/`chage`, parses `last` output.
//! - **Windows**: uses `net user`, `Get-LocalUser`, `New-LocalUser`,
//!   `Set-LocalUser` via PowerShell.
//! - **macOS**: uses `dscl .` and `sysadminctl`.
//! - **Other**: a stub backend that operates on an in-memory database
//!   (useful for testing and remote-desktop scenarios).

mod error;
mod groups;
mod login_history;
mod manager;
mod password;
mod platform;
mod types;

pub use error::AccountError;
pub use groups::Group;
pub use login_history::{LoginEntry, LoginHistory, LoginMethod};
pub use manager::UserManager;
pub use password::{PasswordPolicy, PasswordStrength};
pub use platform::PlatformBackend;
pub use types::{AccountType, UserAccount};

#[cfg(test)]
mod tests;
