//! In-memory stub backend for testing and unsupported platforms.
//!
//! All data lives in memory — no system commands are executed.

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::LoginEntry;
use crate::platform::PlatformBackend;
use crate::types::{AccountType, UserAccount};

/// An in-memory account backend that requires no system access.
#[allow(dead_code)]
pub struct StubBackend {
    users: Vec<UserAccount>,
    groups: Vec<Group>,
    login_entries: Vec<LoginEntry>,
    next_uid: u32,
    next_gid: u32,
    current_uid: u32,
    /// Avatars stored as (uid, path) pairs.
    avatars: Vec<(u32, String)>,
    /// Auto-login UID (only one user can have auto-login at a time).
    auto_login_uid: Option<u32>,
}

#[allow(dead_code)]
impl StubBackend {
    /// Create a stub backend with a single default user.
    pub fn new() -> Self {
        let default_user = UserAccount {
            uid: 1000,
            username: "user".to_string(),
            display_name: "Default User".to_string(),
            home_dir: "/home/user".to_string(),
            shell: "/bin/bash".to_string(),
            account_type: AccountType::Administrator,
            avatar: None,
            is_logged_in: true,
            is_locked: false,
            password_last_changed: Some(1_700_000_000),
            auto_login: false,
        };

        let default_group = Group {
            gid: 1000,
            name: "users".to_string(),
            members: vec![1000],
        };

        let admin_group = Group {
            gid: 27,
            name: "sudo".to_string(),
            members: vec![1000],
        };

        Self {
            users: vec![default_user],
            groups: vec![default_group, admin_group],
            login_entries: Vec::new(),
            next_uid: 1001,
            next_gid: 1001,
            current_uid: 1000,
            avatars: Vec::new(),
            auto_login_uid: None,
        }
    }

    /// Create a completely empty stub backend (no users or groups).
    pub fn empty() -> Self {
        Self {
            users: Vec::new(),
            groups: Vec::new(),
            login_entries: Vec::new(),
            next_uid: 1000,
            next_gid: 1000,
            current_uid: 0,
            avatars: Vec::new(),
            auto_login_uid: None,
        }
    }

    /// Set which UID is considered "current" (logged in).
    pub fn set_current_uid(&mut self, uid: u32) {
        self.current_uid = uid;
    }

    /// Add a login entry to the history.
    pub fn record_login(&mut self, entry: LoginEntry) {
        self.login_entries.push(entry);
    }

    fn find_user(&self, uid: u32) -> Result<&UserAccount, AccountError> {
        self.users
            .iter()
            .find(|u| u.uid == uid)
            .ok_or(AccountError::NotFound)
    }

    fn find_user_mut(&mut self, uid: u32) -> Result<&mut UserAccount, AccountError> {
        self.users
            .iter_mut()
            .find(|u| u.uid == uid)
            .ok_or(AccountError::NotFound)
    }

    fn find_group(&self, gid: u32) -> Result<&Group, AccountError> {
        self.groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)
    }

    fn find_group_mut(&mut self, gid: u32) -> Result<&mut Group, AccountError> {
        self.groups
            .iter_mut()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)
    }
}

impl Default for StubBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBackend for StubBackend {
    fn current_user(&self) -> Result<UserAccount, AccountError> {
        self.find_user(self.current_uid).cloned()
    }

    fn list_users(&self) -> Result<Vec<UserAccount>, AccountError> {
        Ok(self.users.clone())
    }

    fn create_user(
        &mut self,
        username: &str,
        display_name: &str,
        account_type: AccountType,
        _password: &str,
    ) -> Result<UserAccount, AccountError> {
        // Validate username.
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
                "username contains invalid characters (allowed: a-z, 0-9, _, -, .)".to_string(),
            ));
        }
        let first_char = username.chars().next().unwrap_or('0');
        if !first_char.is_ascii_lowercase() && first_char != '_' {
            return Err(AccountError::InvalidUsername(
                "username must start with a lowercase letter or underscore".to_string(),
            ));
        }

        // Check for duplicates.
        if self.users.iter().any(|u| u.username == username) {
            return Err(AccountError::AlreadyExists);
        }

        let uid = self.next_uid;
        self.next_uid += 1;

        let user = UserAccount {
            uid,
            username: username.to_string(),
            display_name: display_name.to_string(),
            home_dir: format!("/home/{username}"),
            shell: "/bin/bash".to_string(),
            account_type,
            avatar: None,
            is_logged_in: false,
            is_locked: false,
            password_last_changed: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
            auto_login: false,
        };

        self.users.push(user.clone());

        // Create a personal group for the user.
        let gid = self.next_gid;
        self.next_gid += 1;
        self.groups.push(Group {
            gid,
            name: username.to_string(),
            members: vec![uid],
        });

        // If admin, add to sudo group.
        if account_type == AccountType::Administrator {
            if let Some(sudo_group) = self.groups.iter_mut().find(|g| g.name == "sudo") {
                if !sudo_group.members.contains(&uid) {
                    sudo_group.members.push(uid);
                }
            }
        }

        Ok(user)
    }

    fn delete_user(&mut self, uid: u32, _delete_home: bool) -> Result<(), AccountError> {
        let pos = self
            .users
            .iter()
            .position(|u| u.uid == uid)
            .ok_or(AccountError::NotFound)?;
        self.users.remove(pos);

        // Remove from all groups.
        for group in &mut self.groups {
            group.members.retain(|&m| m != uid);
        }

        // Remove avatar.
        self.avatars.retain(|(u, _)| *u != uid);

        // Clear auto-login if this was the auto-login user.
        if self.auto_login_uid == Some(uid) {
            self.auto_login_uid = None;
        }

        Ok(())
    }

    fn set_display_name(&mut self, uid: u32, name: &str) -> Result<(), AccountError> {
        let user = self.find_user_mut(uid)?;
        user.display_name = name.to_string();
        Ok(())
    }

    fn set_avatar(&mut self, uid: u32, path: &str) -> Result<(), AccountError> {
        // Verify user exists.
        let _ = self.find_user(uid)?;
        // Update or insert avatar.
        if let Some(entry) = self.avatars.iter_mut().find(|(u, _)| *u == uid) {
            entry.1 = path.to_string();
        } else {
            self.avatars.push((uid, path.to_string()));
        }
        // Also update the user record.
        let user = self.find_user_mut(uid)?;
        user.avatar = Some(path.to_string());
        Ok(())
    }

    fn change_password(
        &mut self,
        uid: u32,
        _old_password: &str,
        _new_password: &str,
    ) -> Result<(), AccountError> {
        let user = self.find_user_mut(uid)?;
        user.password_last_changed = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        Ok(())
    }

    fn set_auto_login(&mut self, uid: u32, enabled: bool) -> Result<(), AccountError> {
        let _ = self.find_user(uid)?;
        if enabled {
            // Disable auto-login for any previous user.
            if let Some(prev_uid) = self.auto_login_uid {
                if let Ok(prev) = self.find_user_mut(prev_uid) {
                    prev.auto_login = false;
                }
            }
            self.auto_login_uid = Some(uid);
            let user = self.find_user_mut(uid)?;
            user.auto_login = true;
        } else {
            if self.auto_login_uid == Some(uid) {
                self.auto_login_uid = None;
            }
            let user = self.find_user_mut(uid)?;
            user.auto_login = false;
        }
        Ok(())
    }

    fn lock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let user = self.find_user_mut(uid)?;
        user.is_locked = true;
        Ok(())
    }

    fn unlock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let user = self.find_user_mut(uid)?;
        user.is_locked = false;
        Ok(())
    }

    fn set_account_type(
        &mut self,
        uid: u32,
        account_type: AccountType,
    ) -> Result<(), AccountError> {
        let user = self.find_user_mut(uid)?;
        user.account_type = account_type;

        // Update sudo group membership accordingly.
        let in_sudo = self
            .groups
            .iter()
            .find(|g| g.name == "sudo")
            .map(|g| g.members.contains(&uid))
            .unwrap_or(false);

        match account_type {
            AccountType::Administrator if !in_sudo => {
                if let Some(sudo_group) = self.groups.iter_mut().find(|g| g.name == "sudo") {
                    sudo_group.members.push(uid);
                }
            }
            AccountType::Standard if in_sudo => {
                if let Some(sudo_group) = self.groups.iter_mut().find(|g| g.name == "sudo") {
                    sudo_group.members.retain(|&m| m != uid);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn list_groups(&self) -> Result<Vec<Group>, AccountError> {
        Ok(self.groups.clone())
    }

    fn user_groups(&self, uid: u32) -> Result<Vec<Group>, AccountError> {
        // Verify user exists.
        let _ = self.find_user(uid)?;
        Ok(self
            .groups
            .iter()
            .filter(|g| g.members.contains(&uid))
            .cloned()
            .collect())
    }

    fn add_to_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let _ = self.find_user(uid)?;
        let group = self.find_group_mut(gid)?;
        if !group.members.contains(&uid) {
            group.members.push(uid);
        }
        Ok(())
    }

    fn remove_from_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let _ = self.find_user(uid)?;
        let group = self.find_group_mut(gid)?;
        group.members.retain(|&m| m != uid);
        Ok(())
    }

    fn recent_logins(&self, uid: u32, count: usize) -> Result<Vec<LoginEntry>, AccountError> {
        let mut entries: Vec<&LoginEntry> =
            self.login_entries.iter().filter(|e| e.uid == uid).collect();
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(entries.into_iter().take(count).cloned().collect())
    }
}
