//! Platform-specific account management backends.
//!
//! Each platform module implements `PlatformBackend` using native
//! system commands or APIs.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as DefaultBackend;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsBackend as DefaultBackend;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacosBackend as DefaultBackend;

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub use stub::StubBackend as DefaultBackend;

pub mod stub;

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::LoginEntry;
use crate::types::{AccountType, UserAccount};

/// Trait abstracting platform-specific user/group operations.
///
/// Implementations shell out to system utilities or call native APIs.
/// All mutating operations require appropriate privileges (e.g. root on
/// Linux, Administrator on Windows).
pub trait PlatformBackend: Send {
    /// Return the currently logged-in user.
    fn current_user(&self) -> Result<UserAccount, AccountError>;

    /// List all human user accounts (uid >= 1000 on Linux, non-system
    /// accounts on Windows/macOS).
    fn list_users(&self) -> Result<Vec<UserAccount>, AccountError>;

    /// Create a new user account.
    fn create_user(
        &mut self,
        username: &str,
        display_name: &str,
        account_type: AccountType,
        password: &str,
    ) -> Result<UserAccount, AccountError>;

    /// Delete a user account. If `delete_home` is `true`, also remove
    /// the user's home directory.
    fn delete_user(&mut self, uid: u32, delete_home: bool) -> Result<(), AccountError>;

    /// Change the display name (GECOS / full name) for a user.
    fn set_display_name(&mut self, uid: u32, name: &str) -> Result<(), AccountError>;

    /// Set the avatar image path for a user.
    fn set_avatar(&mut self, uid: u32, path: &str) -> Result<(), AccountError>;

    /// Change a user's password (requires the old password for verification
    /// on real backends).
    fn change_password(
        &mut self,
        uid: u32,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AccountError>;

    /// Enable or disable auto-login for a user.
    fn set_auto_login(&mut self, uid: u32, enabled: bool) -> Result<(), AccountError>;

    /// Lock (disable) a user account.
    fn lock_account(&mut self, uid: u32) -> Result<(), AccountError>;

    /// Unlock (re-enable) a user account.
    fn unlock_account(&mut self, uid: u32) -> Result<(), AccountError>;

    /// Change a user's account type (standard / administrator).
    fn set_account_type(&mut self, uid: u32, account_type: AccountType)
    -> Result<(), AccountError>;

    /// List all groups on the system.
    fn list_groups(&self) -> Result<Vec<Group>, AccountError>;

    /// Return the groups that a specific user belongs to.
    fn user_groups(&self, uid: u32) -> Result<Vec<Group>, AccountError>;

    /// Add a user to a group.
    fn add_to_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError>;

    /// Remove a user from a group.
    fn remove_from_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError>;

    /// Retrieve recent login entries from system logs.
    fn recent_logins(&self, uid: u32, count: usize) -> Result<Vec<LoginEntry>, AccountError>;
}
