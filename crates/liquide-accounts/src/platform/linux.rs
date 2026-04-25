//! Linux account management backend.
//!
//! Reads `/etc/passwd`, `/etc/group`, `/etc/shadow`, and `/var/log/wtmp`
//! for enumeration and login history. Shells out to `useradd`, `userdel`,
//! `usermod`, `chpasswd`, and `gpasswd` only for mutations (where PAM/SELinux
//! integration requires system tools).

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

// ── utmp/wtmp binary record layout (x86_64 glibc) ───────────────────

/// Size of a single `struct utmp` record (384 bytes on x86_64 Linux).
const UTMP_SIZE: usize = 384;
/// Offset of `ut_type` field (int32).
const UT_TYPE_OFFSET: usize = 0;
/// Offset of `ut_line` field (char[32]).
const UT_LINE_OFFSET: usize = 8;
/// Length of `ut_line`.
const UT_LINE_LEN: usize = 32;
/// Offset of `ut_user` field (char[32]).
const UT_USER_OFFSET: usize = 44;
/// Length of `ut_user`.
const UT_USER_LEN: usize = 32;
/// Offset of `ut_host` field (char[256]).
const UT_HOST_OFFSET: usize = 76;
/// Length of `ut_host`.
const UT_HOST_LEN: usize = 256;
/// Offset of `ut_tv.tv_sec` (int64 on x86_64).
const UT_TV_OFFSET: usize = 340;
/// `ut_type` value indicating a normal user login.
const USER_PROCESS: i32 = 7;

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
        let content = fs::read_to_string("/etc/passwd")
            .map_err(|e| AccountError::PlatformError(format!("failed to read /etc/passwd: {e}")))?;

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

    /// Parse `/etc/group` and return all entries, resolving member names
    /// to UIDs using the supplied passwd entries.
    fn parse_group_file_with(passwd_entries: &[PasswdEntry]) -> Result<Vec<Group>, AccountError> {
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

    /// Parse `/etc/group` and return all entries.
    fn parse_group_file() -> Result<Vec<Group>, AccountError> {
        let passwd_entries = Self::parse_passwd().unwrap_or_default();
        Self::parse_group_file_with(&passwd_entries)
    }

    /// Check if a user is in the `sudo` or `wheel` group by parsing
    /// `/etc/group` directly.
    fn is_admin(username: &str) -> bool {
        let content = fs::read_to_string("/etc/group").unwrap_or_default();
        for line in content.lines() {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() >= 4
                && (fields[0] == "sudo" || fields[0] == "wheel")
                && fields[3].split(',').any(|m| m == username)
            {
                return true;
            }
        }
        false
    }

    /// Check if the account is locked by inspecting `/etc/shadow`.
    ///
    /// A locked account has a password hash starting with `!` or `*`.
    fn is_account_locked(username: &str) -> bool {
        if let Ok(content) = fs::read_to_string("/etc/shadow") {
            for line in content.lines() {
                let fields: Vec<&str> = line.split(':').collect();
                if fields.len() >= 2 && fields[0] == username {
                    let hash = fields[1];
                    return hash.starts_with('!') || hash.starts_with('*');
                }
            }
        }
        // If we cannot read /etc/shadow (no root), assume unlocked.
        false
    }

    /// Read password last-changed date from `/etc/shadow`.
    ///
    /// The third field in shadow is the number of days since epoch
    /// (1970-01-01) when the password was last changed.
    fn password_last_changed(username: &str) -> Option<u64> {
        let content = fs::read_to_string("/etc/shadow").ok()?;
        for line in content.lines() {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() >= 3 && fields[0] == username {
                let days: u64 = fields[2].parse().ok()?;
                if days == 0 {
                    return None; // 0 means "must change on next login"
                }
                return Some(days * 86400);
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

    /// Check if a user is currently logged in by parsing `/var/run/utmp`.
    ///
    /// Same binary format as wtmp; we look for USER_PROCESS records
    /// matching the username.
    fn is_user_logged_in(username: &str) -> bool {
        let data = fs::read("/var/run/utmp")
            .or_else(|_| fs::read("/run/utmp"))
            .unwrap_or_default();

        let mut offset = 0;
        while offset + UTMP_SIZE <= data.len() {
            let record = &data[offset..offset + UTMP_SIZE];
            let ut_type = i32::from_le_bytes(
                record[UT_TYPE_OFFSET..UT_TYPE_OFFSET + 4]
                    .try_into()
                    .unwrap_or([0; 4]),
            );

            if ut_type == USER_PROCESS {
                let user = extract_c_string(&record[UT_USER_OFFSET..UT_USER_OFFSET + UT_USER_LEN]);
                if user == username {
                    return true;
                }
            }
            offset += UTMP_SIZE;
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
            entry
                .gecos
                .split(',')
                .next()
                .unwrap_or(&entry.username)
                .to_string()
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
            is_logged_in: Self::is_user_logged_in(&entry.username),
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
        // Use getuid() syscall directly — no libc crate dependency needed.
        extern "C" {
            fn getuid() -> u32;
        }
        let uid = unsafe { getuid() };
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
        // Validate avatar source path to prevent reading arbitrary files.
        let src = std::path::Path::new(path);
        if !src.exists() {
            return Err(AccountError::PlatformError(
                "avatar source file does not exist".into(),
            ));
        }
        if !src.is_file() {
            return Err(AccountError::PlatformError(
                "avatar source must be a regular file".into(),
            ));
        }
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(
            ext.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "bmp" | "svg"
        ) {
            return Err(AccountError::PlatformError(
                "avatar must be an image file (png, jpg, jpeg, bmp, svg)".into(),
            ));
        }
        let dest = format!("{}/{}", self.avatar_dir, username);
        Self::run_cmd("cp", &[path, &dest])
    }

    fn change_password(
        &mut self,
        uid: u32,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;

        // Verify old password first using su.
        let verify = Command::new("su")
            .args(["-c", "true", &username])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(old_password.as_bytes())?;
                }
                child.wait_with_output()
            })
            .map_err(|e| {
                AccountError::PlatformError(format!("old password verification failed: {e}"))
            })?;
        if !verify.status.success() {
            return Err(AccountError::PlatformError(
                "old password is incorrect".into(),
            ));
        }

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

            fs::write(gdm_path, new_lines.join("\n") + "\n").map_err(|e| {
                AccountError::PlatformError(format!("cannot write {gdm_path}: {e}"))
            })?;
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
        let passwd_entries = Self::parse_passwd()?;
        let entry = passwd_entries
            .iter()
            .find(|e| e.uid == uid)
            .ok_or(AccountError::NotFound)?;
        let username = &entry.username;
        let primary_gid = entry.gid;

        let all_groups = Self::parse_group_file_with(&passwd_entries)?;

        Ok(all_groups
            .into_iter()
            .filter(|g| g.gid == primary_gid || g.members.contains(&uid))
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

        // Read wtmp binary file directly.
        let data = fs::read("/var/log/wtmp").unwrap_or_default();

        let mut entries = Vec::new();
        let mut offset = 0;
        while offset + UTMP_SIZE <= data.len() {
            let record = &data[offset..offset + UTMP_SIZE];
            let ut_type = i32::from_le_bytes(
                record[UT_TYPE_OFFSET..UT_TYPE_OFFSET + 4]
                    .try_into()
                    .unwrap_or([0; 4]),
            );

            if ut_type == USER_PROCESS {
                let user = extract_c_string(&record[UT_USER_OFFSET..UT_USER_OFFSET + UT_USER_LEN]);

                if user == username {
                    let line =
                        extract_c_string(&record[UT_LINE_OFFSET..UT_LINE_OFFSET + UT_LINE_LEN]);
                    let host =
                        extract_c_string(&record[UT_HOST_OFFSET..UT_HOST_OFFSET + UT_HOST_LEN]);
                    let tv_sec = i64::from_le_bytes(
                        record[UT_TV_OFFSET..UT_TV_OFFSET + 8]
                            .try_into()
                            .unwrap_or([0; 8]),
                    );

                    let method = if host.contains('.') || host.contains(':') {
                        // Contains an IP address (v4 or v6) — remote session.
                        LoginMethod::RemoteDesktop
                    } else if line.starts_with(":") {
                        // X11 / Wayland local display.
                        LoginMethod::Password
                    } else {
                        LoginMethod::Password
                    };

                    let ip = if host.is_empty() { None } else { Some(host) };

                    entries.push(LoginEntry {
                        uid,
                        timestamp: tv_sec as u64,
                        success: true,
                        method,
                        ip,
                    });
                }
            }
            offset += UTMP_SIZE;
        }

        // Most recent first.
        entries.reverse();
        entries.truncate(count);
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

/// Extract a NUL-terminated C string from a byte slice.
fn extract_c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}
