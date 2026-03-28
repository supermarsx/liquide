//! macOS account management backend.
//!
//! Uses `dscl .` (Directory Service command line) for user/group
//! enumeration and management, and `sysadminctl` for user creation.

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::{LoginEntry, LoginMethod};
use crate::platform::PlatformBackend;
use crate::types::{AccountType, UserAccount};
use std::process::Command;

/// Minimum UID for human accounts on macOS.
const MIN_HUMAN_UID: u32 = 500;

pub struct MacosBackend;

impl MacosBackend {
    pub fn new() -> Self {
        Self
    }

    /// Run `dscl . -read <path> <key>` and return the value.
    fn dscl_read(path: &str, key: &str) -> Result<String, AccountError> {
        let output = Command::new("dscl")
            .args([".", "-read", path, key])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AccountError::PlatformError(format!("dscl error: {stderr}")));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        // dscl output format: "Key: Value" or "Key:\n Value" for multi-line.
        let value = text
            .lines()
            .filter_map(|line| {
                if let Some(rest) = line.strip_prefix(&format!("{key}:")) {
                    Some(rest.trim().to_string())
                } else if line.starts_with(' ') {
                    Some(line.trim().to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        Ok(value.trim().to_string())
    }

    /// List all user directory entries.
    fn list_user_dirs() -> Result<Vec<String>, AccountError> {
        let output = Command::new("dscl")
            .args([".", "-list", "/Users"])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl list failed: {e}")))?;

        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('_'))
            .collect())
    }

    /// Build a UserAccount from a username.
    fn build_user_account(username: &str) -> Result<UserAccount, AccountError> {
        let user_path = format!("/Users/{username}");

        let uid_str = Self::dscl_read(&user_path, "UniqueID")?;
        let uid: u32 = uid_str
            .parse()
            .map_err(|_| AccountError::PlatformError("cannot parse UID".to_string()))?;

        let display_name = Self::dscl_read(&user_path, "RealName").unwrap_or(username.to_string());
        let home_dir = Self::dscl_read(&user_path, "NFSHomeDirectory")
            .unwrap_or(format!("/Users/{username}"));
        let shell = Self::dscl_read(&user_path, "UserShell").unwrap_or("/bin/zsh".to_string());

        // Check admin group membership.
        let is_admin = Self::is_in_group(username, "admin");

        let account_type = if is_admin {
            AccountType::Administrator
        } else {
            AccountType::Standard
        };

        // Check login status via `who`.
        let is_logged_in = Command::new("who")
            .output()
            .map(|out| {
                let text = String::from_utf8_lossy(&out.stdout);
                text.lines().any(|l| {
                    l.split_whitespace()
                        .next()
                        .map(|u| u == username)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        // Check if account is disabled.
        let auth_authority =
            Self::dscl_read(&user_path, "AuthenticationAuthority").unwrap_or_default();
        let is_locked = auth_authority.contains("DisabledUser");

        // Avatar: macOS stores user pictures in /Library/User Pictures or
        // via dsimport; we check the standard location.
        let avatar_path = format!("/Library/User Pictures/{username}.jpg");
        let avatar = if std::path::Path::new(&avatar_path).exists() {
            Some(avatar_path)
        } else {
            None
        };

        // Password last changed — use `dscl . -read /Users/<user> passwordPolicyOptions`
        let password_last_changed =
            Self::dscl_read(&user_path, "passwordPolicyOptions")
                .ok()
                .and_then(|xml| {
                    // Rough extraction of passwordLastSetTime from plist XML.
                    if let Some(pos) = xml.find("passwordLastSetTime") {
                        xml[pos..]
                            .split("<real>")
                            .nth(1)
                            .and_then(|s| s.split("</real>").next())
                            .and_then(|s| s.trim().parse::<f64>().ok())
                            .map(|t| t as u64)
                    } else {
                        None
                    }
                });

        // Auto-login: check /etc/kcpassword existence and loginwindow pref.
        let auto_login = Command::new("defaults")
            .args([
                "read",
                "/Library/Preferences/com.apple.loginwindow",
                "autoLoginUser",
            ])
            .output()
            .map(|out| {
                let text = String::from_utf8_lossy(&out.stdout);
                text.trim() == username
            })
            .unwrap_or(false);

        Ok(UserAccount {
            uid,
            username: username.to_string(),
            display_name,
            home_dir,
            shell,
            account_type,
            avatar,
            is_logged_in,
            is_locked,
            password_last_changed,
            auto_login,
        })
    }

    /// Check if a user is a member of a specific group.
    fn is_in_group(username: &str, group: &str) -> bool {
        Command::new("dseditgroup")
            .args(["-o", "checkmember", "-m", username, group])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Resolve a UID to a username.
    fn uid_to_username(uid: u32) -> Result<String, AccountError> {
        let output = Command::new("dscl")
            .args([".", "-search", "/Users", "UniqueID", &uid.to_string()])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl search failed: {e}")))?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .next()
            .and_then(|l| l.split_whitespace().next())
            .map(|s| s.to_string())
            .ok_or(AccountError::NotFound)
    }
}

impl Default for MacosBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBackend for MacosBackend {
    fn current_user(&self) -> Result<UserAccount, AccountError> {
        let username = std::env::var("USER")
            .unwrap_or_else(|_| whoami_fallback());
        Self::build_user_account(&username)
    }

    fn list_users(&self) -> Result<Vec<UserAccount>, AccountError> {
        let usernames = Self::list_user_dirs()?;
        let mut users = Vec::new();
        for username in &usernames {
            if username == "daemon" || username == "nobody" || username == "root" {
                continue;
            }
            match Self::build_user_account(username) {
                Ok(user) if user.uid >= MIN_HUMAN_UID => users.push(user),
                _ => continue,
            }
        }
        Ok(users)
    }

    fn create_user(
        &mut self,
        username: &str,
        display_name: &str,
        account_type: AccountType,
        password: &str,
    ) -> Result<UserAccount, AccountError> {
        if username.is_empty() {
            return Err(AccountError::InvalidUsername(
                "username cannot be empty".to_string(),
            ));
        }
        if !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(AccountError::InvalidUsername(
                "username contains invalid characters".to_string(),
            ));
        }

        // Check if exists.
        if Self::list_user_dirs()?.iter().any(|u| u == username) {
            return Err(AccountError::AlreadyExists);
        }

        let admin_flag = if account_type == AccountType::Administrator {
            "-admin"
        } else {
            ""
        };

        let mut args = vec![
            "-addUser",
            username,
            "-fullName",
            display_name,
            "-password",
            password,
        ];
        if !admin_flag.is_empty() {
            args.push(admin_flag);
        }

        let output = Command::new("sysadminctl")
            .args(&args)
            .output()
            .map_err(|e| AccountError::PlatformError(format!("sysadminctl failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("PermissionError") || stderr.contains("authentication") {
                return Err(AccountError::PermissionDenied);
            }
            return Err(AccountError::PlatformError(format!(
                "sysadminctl: {stderr}"
            )));
        }

        Self::build_user_account(username)
    }

    fn delete_user(&mut self, uid: u32, delete_home: bool) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let mut args = vec!["-deleteUser", &username];
        if !delete_home {
            args.push("-keepHome");
        }
        let output = Command::new("sysadminctl")
            .args(&args)
            .output()
            .map_err(|e| AccountError::PlatformError(format!("sysadminctl failed: {e}")))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(AccountError::PlatformError(format!(
                "sysadminctl: {stderr}"
            )))
        }
    }

    fn set_display_name(&mut self, uid: u32, name: &str) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let path = format!("/Users/{username}");
        let output = Command::new("dscl")
            .args([".", "-change", &path, "RealName", &username, name])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            // Try create instead of change (in case RealName wasn't set).
            let output2 = Command::new("dscl")
                .args([".", "-create", &path, "RealName", name])
                .output()
                .map_err(|e| AccountError::PlatformError(format!("dscl failed: {e}")))?;
            if output2.status.success() {
                Ok(())
            } else {
                Err(AccountError::PlatformError(
                    "failed to set display name".to_string(),
                ))
            }
        }
    }

    fn set_avatar(&mut self, uid: u32, path: &str) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let user_path = format!("/Users/{username}");
        // Use dsimport or dscl to set the picture.
        let output = Command::new("dscl")
            .args([".", "-create", &user_path, "Picture", path])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(AccountError::PlatformError(format!("dscl: {stderr}")))
        }
    }

    fn change_password(
        &mut self,
        uid: u32,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let user_path = format!("/Users/{username}");
        let output = Command::new("dscl")
            .args([
                ".",
                "-passwd",
                &user_path,
                old_password,
                new_password,
            ])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl passwd failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("eDSAuthFailed") {
                Err(AccountError::PermissionDenied)
            } else {
                Err(AccountError::PlatformError(format!("dscl: {stderr}")))
            }
        }
    }

    fn set_auto_login(&mut self, uid: u32, enabled: bool) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        if enabled {
            let output = Command::new("defaults")
                .args([
                    "write",
                    "/Library/Preferences/com.apple.loginwindow",
                    "autoLoginUser",
                    &username,
                ])
                .output()
                .map_err(|e| AccountError::PlatformError(format!("defaults write failed: {e}")))?;
            if !output.status.success() {
                return Err(AccountError::PermissionDenied);
            }
        } else {
            let _ = Command::new("defaults")
                .args([
                    "delete",
                    "/Library/Preferences/com.apple.loginwindow",
                    "autoLoginUser",
                ])
                .output();
        }
        Ok(())
    }

    fn lock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let user_path = format!("/Users/{username}");
        let output = Command::new("dscl")
            .args([
                ".",
                "-append",
                &user_path,
                "AuthenticationAuthority",
                ";DisabledUser;",
            ])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(AccountError::PermissionDenied)
        }
    }

    fn unlock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let user_path = format!("/Users/{username}");
        // Read current AuthenticationAuthority, remove DisabledUser, write back.
        let auth = Self::dscl_read(&user_path, "AuthenticationAuthority").unwrap_or_default();
        let cleaned = auth.replace(";DisabledUser;", "");
        let output = Command::new("dscl")
            .args([
                ".",
                "-create",
                &user_path,
                "AuthenticationAuthority",
                &cleaned,
            ])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(AccountError::PermissionDenied)
        }
    }

    fn set_account_type(
        &mut self,
        uid: u32,
        account_type: AccountType,
    ) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        match account_type {
            AccountType::Administrator => {
                let output = Command::new("dseditgroup")
                    .args(["-o", "edit", "-a", &username, "-t", "user", "admin"])
                    .output()
                    .map_err(|e| AccountError::PlatformError(format!("dseditgroup failed: {e}")))?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(AccountError::PermissionDenied)
                }
            }
            AccountType::Standard => {
                let output = Command::new("dseditgroup")
                    .args(["-o", "edit", "-d", &username, "-t", "user", "admin"])
                    .output()
                    .map_err(|e| AccountError::PlatformError(format!("dseditgroup failed: {e}")))?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(AccountError::PermissionDenied)
                }
            }
        }
    }

    fn list_groups(&self) -> Result<Vec<Group>, AccountError> {
        let output = Command::new("dscl")
            .args([".", "-list", "/Groups", "PrimaryGroupID"])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dscl failed: {e}")))?;
        let text = String::from_utf8_lossy(&output.stdout);

        let mut groups = Vec::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let name = parts[0].to_string();
            if name.starts_with('_') {
                continue; // Skip system groups.
            }
            let gid = parts[1].parse::<u32>().unwrap_or(0);

            // Get members.
            let members_str = Self::dscl_read(&format!("/Groups/{name}"), "GroupMembership")
                .unwrap_or_default();
            let member_names: Vec<&str> = members_str.split_whitespace().collect();

            // Resolve to UIDs.
            let member_uids: Vec<u32> = member_names
                .iter()
                .filter_map(|mname| {
                    Self::dscl_read(&format!("/Users/{mname}"), "UniqueID")
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok())
                })
                .collect();

            groups.push(Group {
                gid,
                name,
                members: member_uids,
            });
        }

        Ok(groups)
    }

    fn user_groups(&self, uid: u32) -> Result<Vec<Group>, AccountError> {
        let username = Self::uid_to_username(uid)?;
        let output = Command::new("id")
            .args(["-Gn", &username])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("id failed: {e}")))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let group_names: Vec<&str> = text.split_whitespace().collect();

        let all_groups = self.list_groups()?;
        Ok(all_groups
            .into_iter()
            .filter(|g| group_names.contains(&g.name.as_str()))
            .collect())
    }

    fn add_to_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let groups = self.list_groups()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;
        let output = Command::new("dseditgroup")
            .args(["-o", "edit", "-a", &username, "-t", "user", &group.name])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dseditgroup failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(AccountError::PermissionDenied)
        }
    }

    fn remove_from_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let groups = self.list_groups()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;
        let output = Command::new("dseditgroup")
            .args(["-o", "edit", "-d", &username, "-t", "user", &group.name])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("dseditgroup failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(AccountError::PermissionDenied)
        }
    }

    fn recent_logins(&self, uid: u32, count: usize) -> Result<Vec<LoginEntry>, AccountError> {
        let username = Self::uid_to_username(uid)?;
        let output = Command::new("last")
            .args(["-n", &count.to_string(), &username])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("last failed: {e}")))?;
        let text = String::from_utf8_lossy(&output.stdout);

        let mut entries = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("wtmp") {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.is_empty() || fields[0] != username {
                continue;
            }

            let ip = fields.get(2).and_then(|s| {
                if s.contains('.') || s.contains(':') {
                    Some(s.to_string())
                } else {
                    None
                }
            });

            let method = if ip.is_some() {
                LoginMethod::RemoteDesktop
            } else {
                LoginMethod::Password
            };

            entries.push(LoginEntry {
                uid,
                timestamp: 0,
                success: true,
                method,
                ip,
            });
        }

        Ok(entries)
    }
}

/// Fallback to get current username when $USER is not set.
fn whoami_fallback() -> String {
    Command::new("whoami")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
