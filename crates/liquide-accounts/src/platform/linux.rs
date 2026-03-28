//! Linux account management backend.
//!
//! Reads `/etc/passwd` and `/etc/group` for enumeration, shells out to
//! `useradd`, `userdel`, `usermod`, `passwd`, `chage`, `gpasswd`, and
//! `last` for mutations and login history.

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::{LoginEntry, LoginMethod};
use crate::platform::PlatformBackend;
use crate::types::{AccountType, UserAccount};
use std::fs;
use std::process::Command;

/// Minimum UID for human (non-system) accounts on most Linux distros.
const MIN_HUMAN_UID: u32 = 1000;
/// UIDs at or above this are typically `nobody` / overflow accounts.
const MAX_HUMAN_UID: u32 = 60_000;

pub struct LinuxBackend {
    /// Path to avatars directory (default: /var/lib/AccountsService/icons).
    avatar_dir: String,
}

impl LinuxBackend {
    pub fn new() -> Self {
        Self {
            avatar_dir: "/var/lib/AccountsService/icons".to_string(),
        }
    }

    /// Parse `/etc/passwd` and return all entries.
    fn parse_passwd() -> Result<Vec<PasswdEntry>, AccountError> {
        let content = fs::read_to_string("/etc/passwd").map_err(|e| {
            AccountError::PlatformError(format!("failed to read /etc/passwd: {e}"))
        })?;

        let mut entries = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() < 7 {
                continue;
            }
            let uid = fields[2].parse::<u32>().unwrap_or(0);
            let gid = fields[3].parse::<u32>().unwrap_or(0);
            entries.push(PasswdEntry {
                username: fields[0].to_string(),
                uid,
                gid,
                gecos: fields[4].to_string(),
                home_dir: fields[5].to_string(),
                shell: fields[6].to_string(),
            });
        }
        Ok(entries)
    }

    /// Parse `/etc/group` and return all entries.
    fn parse_group_file() -> Result<Vec<Group>, AccountError> {
        let content = fs::read_to_string("/etc/group")
            .map_err(|e| AccountError::PlatformError(format!("failed to read /etc/group: {e}")))?;

        let mut groups = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() < 4 {
                continue;
            }
            let gid = fields[2].parse::<u32>().unwrap_or(0);
            let member_names: Vec<&str> = if fields[3].is_empty() {
                Vec::new()
            } else {
                fields[3].split(',').collect()
            };

            // Resolve member names to UIDs. This is O(N*M) but the user
            // count is small.
            let passwd_entries = Self::parse_passwd().unwrap_or_default();
            let member_uids: Vec<u32> = member_names
                .iter()
                .filter_map(|name| {
                    passwd_entries
                        .iter()
                        .find(|e| e.username == *name)
                        .map(|e| e.uid)
                })
                .collect();

            groups.push(Group {
                gid,
                name: fields[0].to_string(),
                members: member_uids,
            });
        }
        Ok(groups)
    }

    /// Check if a user is in the `sudo` or `wheel` group.
    fn is_admin(username: &str) -> bool {
        let groups = Self::parse_group_file().unwrap_or_default();
        groups
            .iter()
            .any(|g| (g.name == "sudo" || g.name == "wheel") && {
                // Re-check by username since we resolved UIDs above but
                // also need the raw member list.
                let content = fs::read_to_string("/etc/group").unwrap_or_default();
                content.lines().any(|line| {
                    let fields: Vec<&str> = line.split(':').collect();
                    fields.len() >= 4
                        && (fields[0] == "sudo" || fields[0] == "wheel")
                        && fields[3].split(',').any(|m| m == username)
                })
            })
    }

    /// Check if the account is locked (password starts with `!` in /etc/shadow,
    /// or `passwd -S` reports `L`).
    fn is_account_locked(username: &str) -> bool {
        let output = Command::new("passwd")
            .args(["-S", username])
            .output();
        match output {
            Ok(out) => {
                let status = String::from_utf8_lossy(&out.stdout);
                // Format: "username L ..." where L means locked
                status
                    .split_whitespace()
                    .nth(1)
                    .map(|s| s == "L" || s == "LK")
                    .unwrap_or(false)
            }
            Err(_) => false,
        }
    }

    /// Read password last-changed date from `chage -l`.
    fn password_last_changed(username: &str) -> Option<u64> {
        let output = Command::new("chage")
            .args(["-l", username])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.starts_with("Last password change") {
                // Format: "Last password change\t\t\t: Mon DD, YYYY"
                let date_str = line.split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string();
                if date_str == "never" || date_str.is_empty() {
                    return None;
                }
                // Parse "Mon DD, YYYY" — rough parsing, return epoch.
                return parse_date_to_epoch(&date_str);
            }
        }
        None
    }

    /// Check if auto-login is configured (reads lightdm/gdm config).
    fn is_auto_login(username: &str) -> bool {
        // Check GDM custom.conf
        if let Ok(content) = fs::read_to_string("/etc/gdm3/custom.conf")
            .or_else(|_| fs::read_to_string("/etc/gdm/custom.conf"))
        {
            let has_auto_login_enable = content.lines().any(|l| {
                let l = l.trim();
                l.eq_ignore_ascii_case("AutomaticLoginEnable=true")
                    || l.eq_ignore_ascii_case("AutomaticLoginEnable = true")
            });
            let has_auto_login_user = content.lines().any(|l| {
                let l = l.trim();
                l.starts_with("AutomaticLogin=") && l.ends_with(username)
            });
            if has_auto_login_enable && has_auto_login_user {
                return true;
            }
        }

        // Check LightDM
        if let Ok(content) = fs::read_to_string("/etc/lightdm/lightdm.conf") {
            let has_user = content.lines().any(|l| {
                let l = l.trim();
                l.starts_with("autologin-user=") && l.ends_with(username)
            });
            if has_user {
                return true;
            }
        }

        false
    }

    /// Resolve a UID to a username via the passwd entries.
    fn uid_to_username(uid: u32) -> Result<String, AccountError> {
        let entries = Self::parse_passwd()?;
        entries
            .iter()
            .find(|e| e.uid == uid)
            .map(|e| e.username.clone())
            .ok_or(AccountError::NotFound)
    }

    /// Convert a `PasswdEntry` to a `UserAccount`.
    fn entry_to_account(&self, entry: &PasswdEntry) -> UserAccount {
        let display_name = if entry.gecos.is_empty() {
            entry.username.clone()
        } else {
            // GECOS field: "Full Name,Room,Work Phone,Home Phone,Other"
            entry.gecos.split(',').next().unwrap_or(&entry.username).to_string()
        };

        let avatar_path = format!("{}/{}", self.avatar_dir, entry.username);
        let avatar = if std::path::Path::new(&avatar_path).exists() {
            Some(avatar_path)
        } else {
            None
        };

        let account_type = if Self::is_admin(&entry.username) {
            AccountType::Administrator
        } else {
            AccountType::Standard
        };

        UserAccount {
            uid: entry.uid,
            username: entry.username.clone(),
            display_name,
            home_dir: entry.home_dir.clone(),
            shell: entry.shell.clone(),
            account_type,
            avatar,
            is_logged_in: is_user_logged_in(&entry.username),
            is_locked: Self::is_account_locked(&entry.username),
            password_last_changed: Self::password_last_changed(&entry.username),
            auto_login: Self::is_auto_login(&entry.username),
        }
    }

    /// Run a command and check for success.
    fn run_cmd(cmd: &str, args: &[&str]) -> Result<(), AccountError> {
        let output = Command::new(cmd)
            .args(args)
            .output()
            .map_err(|e| AccountError::PlatformError(format!("failed to run {cmd}: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Permission denied") || stderr.contains("Operation not permitted") {
                Err(AccountError::PermissionDenied)
            } else {
                Err(AccountError::PlatformError(format!(
                    "{cmd} failed: {stderr}"
                )))
            }
        }
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBackend for LinuxBackend {
    fn current_user(&self) -> Result<UserAccount, AccountError> {
        let uid = unsafe { libc::getuid() };
        let entries = Self::parse_passwd()?;
        let entry = entries
            .iter()
            .find(|e| e.uid == uid)
            .ok_or(AccountError::NotFound)?;
        Ok(self.entry_to_account(entry))
    }

    fn list_users(&self) -> Result<Vec<UserAccount>, AccountError> {
        let entries = Self::parse_passwd()?;
        let users: Vec<UserAccount> = entries
            .iter()
            .filter(|e| {
                e.uid >= MIN_HUMAN_UID
                    && e.uid < MAX_HUMAN_UID
                    && !e.shell.ends_with("/nologin")
                    && !e.shell.ends_with("/false")
            })
            .map(|e| self.entry_to_account(e))
            .collect();
        Ok(users)
    }

    fn create_user(
        &mut self,
        username: &str,
        display_name: &str,
        account_type: AccountType,
        password: &str,
    ) -> Result<UserAccount, AccountError> {
        // Validate username.
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

        // Check if user already exists.
        let entries = Self::parse_passwd()?;
        if entries.iter().any(|e| e.username == username) {
            return Err(AccountError::AlreadyExists);
        }

        // Create user with useradd.
        let mut args = vec!["-m", "-c", display_name, "-s", "/bin/bash"];
        if account_type == AccountType::Administrator {
            args.extend(["-G", "sudo"]);
        }
        args.push(username);
        Self::run_cmd("useradd", &args)?;

        // Set password via chpasswd (reads "username:password" from stdin).
        let passwd_input = format!("{username}:{password}");
        let output = Command::new("chpasswd")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(passwd_input.as_bytes())?;
                }
                child.wait_with_output()
            })
            .map_err(|e| AccountError::PlatformError(format!("chpasswd failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AccountError::PlatformError(format!(
                "chpasswd failed: {stderr}"
            )));
        }

        // Re-read the user from /etc/passwd.
        let entries = Self::parse_passwd()?;
        let entry = entries
            .iter()
            .find(|e| e.username == username)
            .ok_or_else(|| {
                AccountError::PlatformError("user created but not found in /etc/passwd".to_string())
            })?;
        Ok(self.entry_to_account(entry))
    }

    fn delete_user(&mut self, uid: u32, delete_home: bool) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let mut args = vec![];
        if delete_home {
            args.push("-r");
        }
        args.push(&username);
        Self::run_cmd("userdel", &args)
    }

    fn set_display_name(&mut self, uid: u32, name: &str) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        Self::run_cmd("usermod", &["-c", name, &username])
    }

    fn set_avatar(&mut self, uid: u32, path: &str) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        // Copy the file to AccountsService icons directory.
        let dest = format!("{}/{}", self.avatar_dir, username);
        Self::run_cmd("cp", &[path, &dest])
    }

    fn change_password(
        &mut self,
        uid: u32,
        _old_password: &str,
        new_password: &str,
    ) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let passwd_input = format!("{username}:{new_password}");
        let output = Command::new("chpasswd")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(passwd_input.as_bytes())?;
                }
                child.wait_with_output()
            })
            .map_err(|e| AccountError::PlatformError(format!("chpasswd failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(AccountError::PlatformError(format!(
                "chpasswd failed: {stderr}"
            )))
        }
    }

    fn set_auto_login(&mut self, uid: u32, enabled: bool) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        // Try GDM first, then LightDM.
        let gdm_path = if std::path::Path::new("/etc/gdm3/custom.conf").exists() {
            "/etc/gdm3/custom.conf"
        } else {
            "/etc/gdm/custom.conf"
        };

        if std::path::Path::new(gdm_path).exists() {
            let content = fs::read_to_string(gdm_path)
                .map_err(|e| AccountError::PlatformError(format!("cannot read {gdm_path}: {e}")))?;

            let mut new_lines: Vec<String> = Vec::new();
            let mut in_daemon_section = false;
            let mut wrote_auto = false;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed == "[daemon]" {
                    in_daemon_section = true;
                    new_lines.push(line.to_string());
                    continue;
                }
                if trimmed.starts_with('[') && trimmed != "[daemon]" {
                    if in_daemon_section && !wrote_auto && enabled {
                        new_lines.push(format!("AutomaticLoginEnable=true"));
                        new_lines.push(format!("AutomaticLogin={username}"));
                        wrote_auto = true;
                    }
                    in_daemon_section = false;
                }
                if in_daemon_section
                    && (trimmed.starts_with("AutomaticLoginEnable")
                        || trimmed.starts_with("AutomaticLogin="))
                {
                    continue; // Strip old auto-login lines.
                }
                new_lines.push(line.to_string());
            }
            if in_daemon_section && !wrote_auto && enabled {
                new_lines.push(format!("AutomaticLoginEnable=true"));
                new_lines.push(format!("AutomaticLogin={username}"));
            }

            fs::write(gdm_path, new_lines.join("\n") + "\n")
                .map_err(|e| AccountError::PlatformError(format!("cannot write {gdm_path}: {e}")))?;
            return Ok(());
        }

        Err(AccountError::PlatformError(
            "no supported display manager configuration found".to_string(),
        ))
    }

    fn lock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        Self::run_cmd("usermod", &["-L", &username])
    }

    fn unlock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        Self::run_cmd("usermod", &["-U", &username])
    }

    fn set_account_type(
        &mut self,
        uid: u32,
        account_type: AccountType,
    ) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        match account_type {
            AccountType::Administrator => {
                // Add to sudo (or wheel) group.
                Self::run_cmd("usermod", &["-aG", "sudo", &username])
                    .or_else(|_| Self::run_cmd("usermod", &["-aG", "wheel", &username]))
            }
            AccountType::Standard => {
                // Remove from sudo and wheel groups.
                let _ = Self::run_cmd("gpasswd", &["-d", &username, "sudo"]);
                let _ = Self::run_cmd("gpasswd", &["-d", &username, "wheel"]);
                Ok(())
            }
        }
    }

    fn list_groups(&self) -> Result<Vec<Group>, AccountError> {
        Self::parse_group_file()
    }

    fn user_groups(&self, uid: u32) -> Result<Vec<Group>, AccountError> {
        let username = Self::uid_to_username(uid)?;
        let all_groups = Self::parse_group_file()?;

        // Also check if the user's primary GID matches any group.
        let entries = Self::parse_passwd()?;
        let primary_gid = entries
            .iter()
            .find(|e| e.uid == uid)
            .map(|e| e.gid)
            .unwrap_or(0);

        Ok(all_groups
            .into_iter()
            .filter(|g| {
                g.gid == primary_gid
                    || {
                        // Check raw /etc/group for this user's membership.
                        let content = fs::read_to_string("/etc/group").unwrap_or_default();
                        content.lines().any(|line| {
                            let fields: Vec<&str> = line.split(':').collect();
                            fields.len() >= 4
                                && fields[2].parse::<u32>().ok() == Some(g.gid)
                                && fields[3].split(',').any(|m| m == username)
                        })
                    }
            })
            .collect())
    }

    fn add_to_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let groups = Self::parse_group_file()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;
        Self::run_cmd("gpasswd", &["-a", &username, &group.name])
    }

    fn remove_from_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let groups = Self::parse_group_file()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;
        Self::run_cmd("gpasswd", &["-d", &username, &group.name])
    }

    fn recent_logins(&self, uid: u32, count: usize) -> Result<Vec<LoginEntry>, AccountError> {
        let username = Self::uid_to_username(uid)?;
        let output = Command::new("last")
            .args(["-n", &count.to_string(), &username])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("last command failed: {e}")))?;

        let text = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("wtmp") || line.starts_with("btmp") {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 4 || fields[0] != username {
                continue;
            }

            let terminal = fields[1];
            let ip = if fields.len() > 2 && fields[2].contains('.') {
                Some(fields[2].to_string())
            } else {
                None
            };

            let method = if ip.is_some() {
                LoginMethod::RemoteDesktop
            } else if terminal.starts_with("pts/") {
                LoginMethod::Password
            } else {
                LoginMethod::Password
            };

            entries.push(LoginEntry {
                uid,
                timestamp: 0, // `last` doesn't give unix timestamps easily
                success: true,
                method,
                ip,
            });
        }

        Ok(entries)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

struct PasswdEntry {
    username: String,
    uid: u32,
    gid: u32,
    gecos: String,
    home_dir: String,
    shell: String,
}

/// Check if a user is currently logged in (has an active session).
fn is_user_logged_in(username: &str) -> bool {
    Command::new("who")
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
        .unwrap_or(false)
}

/// Rough date parser for "Mon DD, YYYY" format from `chage` output.
fn parse_date_to_epoch(date_str: &str) -> Option<u64> {
    // Format: "Jun 15, 2024" or similar.
    let parts: Vec<&str> = date_str.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let month = match parts[0].to_lowercase().as_str() {
        "jan" => 0,
        "feb" => 1,
        "mar" => 2,
        "apr" => 3,
        "may" => 4,
        "jun" => 5,
        "jul" => 6,
        "aug" => 7,
        "sep" => 8,
        "oct" => 9,
        "nov" => 10,
        "dec" => 11,
        _ => return None,
    };
    let day: u64 = parts[1].trim_end_matches(',').parse().ok()?;
    let year: u64 = parts[2].parse().ok()?;
    if year < 1970 {
        return None;
    }
    // Rough epoch calculation (not accounting for leap seconds etc.).
    let years_since_epoch = year - 1970;
    let leap_years = (year - 1969) / 4 - (year - 1901) / 100 + (year - 1601) / 400;
    let days_per_month: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let mut day_of_year: u64 = day - 1;
    for m in 0..month {
        day_of_year += days_per_month[m as usize];
        if m == 1 && is_leap {
            day_of_year += 1;
        }
    }
    let total_days = years_since_epoch * 365 + leap_years + day_of_year;
    Some(total_days * 86400)
}
