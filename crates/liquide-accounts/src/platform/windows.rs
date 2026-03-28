//! Windows account management backend.
//!
//! Uses `net user`, `net localgroup`, and PowerShell cmdlets
//! (`Get-LocalUser`, `New-LocalUser`, `Set-LocalUser`, `Remove-LocalUser`,
//! `Add-LocalGroupMember`, `Remove-LocalGroupMember`) for account
//! management.

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::{LoginEntry, LoginMethod};
use crate::platform::PlatformBackend;
use crate::types::{AccountType, UserAccount};
use std::process::Command;

pub struct WindowsBackend {
    /// Path to user profile pictures (typically C:\Users\<user>\...)
    avatar_dir: String,
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self {
            avatar_dir: String::new(),
        }
    }

    /// Run a PowerShell command and capture stdout.
    fn powershell(script: &str) -> Result<String, AccountError> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("powershell failed: {e}")))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Access is denied") || stderr.contains("PermissionDenied") {
                Err(AccountError::PermissionDenied)
            } else {
                Err(AccountError::PlatformError(format!(
                    "powershell error: {stderr}"
                )))
            }
        }
    }

    /// Run `net user <username>` and parse the output.
    fn net_user_info(username: &str) -> Result<NetUserInfo, AccountError> {
        let output = Command::new("net")
            .args(["user", username])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net user failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not found") {
                return Err(AccountError::NotFound);
            }
            return Err(AccountError::PlatformError(format!("net user: {stderr}")));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut info = NetUserInfo {
            full_name: String::new(),
            active: true,
            last_password_set: None,
        };

        for line in text.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("Full Name") {
                info.full_name = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("Account active") {
                info.active = val.trim().eq_ignore_ascii_case("Yes");
            } else if let Some(val) = line.strip_prefix("Password last set") {
                let val = val.trim();
                if !val.is_empty() && val != "Never" {
                    // Windows date format varies by locale; we store a rough epoch.
                    info.last_password_set = parse_windows_date(val);
                }
            }
        }

        Ok(info)
    }

    /// Get the current username via environment.
    fn current_username() -> String {
        std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string())
    }

    /// Check if a user is in the Administrators group.
    fn is_admin(username: &str) -> bool {
        let script = format!(
            "(Get-LocalGroupMember -Group 'Administrators' -ErrorAction SilentlyContinue | Where-Object {{ $_.Name -like '*\\\\{username}' }}).Count -gt 0"
        );
        Self::powershell(&script)
            .map(|s| s.trim().eq_ignore_ascii_case("True"))
            .unwrap_or(false)
    }

    /// Resolve username to a pseudo-UID (RID from SID).
    fn username_to_uid(username: &str) -> Result<u32, AccountError> {
        let script = format!(
            "(Get-LocalUser -Name '{username}' | Select-Object -ExpandProperty SID).Value.Split('-')[-1]"
        );
        let rid_str = Self::powershell(&script)?;
        rid_str
            .trim()
            .parse::<u32>()
            .map_err(|e| AccountError::PlatformError(format!("cannot parse RID: {e}")))
    }

    /// Resolve a UID (RID) back to a username.
    fn uid_to_username(uid: u32) -> Result<String, AccountError> {
        let script = format!(
            "Get-LocalUser | Where-Object {{ $_.SID.Value.Split('-')[-1] -eq '{uid}' }} | Select-Object -ExpandProperty Name"
        );
        let name = Self::powershell(&script)?;
        if name.is_empty() {
            Err(AccountError::NotFound)
        } else {
            Ok(name)
        }
    }

    fn build_user_account(&self, username: &str) -> Result<UserAccount, AccountError> {
        let uid = Self::username_to_uid(username)?;
        let info = Self::net_user_info(username)?;

        let display_name = if info.full_name.is_empty() {
            username.to_string()
        } else {
            info.full_name
        };

        let home_dir = format!("C:\\Users\\{username}");
        let account_type = if Self::is_admin(username) {
            AccountType::Administrator
        } else {
            AccountType::Standard
        };

        // Check if the user has an active logon session.
        let is_logged_in = Self::powershell(&format!(
            "(query user 2>$null | Select-String '{username}').Count -gt 0"
        ))
        .map(|s| s.trim().eq_ignore_ascii_case("True"))
        .unwrap_or(false);

        // Check auto-login registry key.
        let auto_login = Self::powershell(
            "Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' -Name AutoAdminLogon -ErrorAction SilentlyContinue | Select-Object -ExpandProperty AutoAdminLogon"
        ).map(|s| s.trim() == "1").unwrap_or(false)
            && Self::powershell(
                "Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' -Name DefaultUserName -ErrorAction SilentlyContinue | Select-Object -ExpandProperty DefaultUserName"
            ).map(|s| s.trim().eq_ignore_ascii_case(username)).unwrap_or(false);

        Ok(UserAccount {
            uid,
            username: username.to_string(),
            display_name,
            home_dir,
            shell: "cmd.exe".to_string(),
            account_type,
            avatar: None,
            is_logged_in,
            is_locked: !info.active,
            password_last_changed: info.last_password_set,
            auto_login,
        })
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBackend for WindowsBackend {
    fn current_user(&self) -> Result<UserAccount, AccountError> {
        let username = Self::current_username();
        self.build_user_account(&username)
    }

    fn list_users(&self) -> Result<Vec<UserAccount>, AccountError> {
        let script = "Get-LocalUser | Where-Object { $_.Enabled -eq $true -or $_.Enabled -eq $false } | Select-Object -ExpandProperty Name";
        let output = Self::powershell(script)?;
        let mut users = Vec::new();
        for line in output.lines() {
            let username = line.trim();
            if username.is_empty() {
                continue;
            }
            // Skip well-known system accounts.
            if username.eq_ignore_ascii_case("DefaultAccount")
                || username.eq_ignore_ascii_case("WDAGUtilityAccount")
                || username.eq_ignore_ascii_case("Guest")
            {
                continue;
            }
            match self.build_user_account(username) {
                Ok(user) => users.push(user),
                Err(_) => continue,
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
        if Self::username_to_uid(username).is_ok() {
            return Err(AccountError::AlreadyExists);
        }

        // Create user.
        let script = format!(
            "$pw = ConvertTo-SecureString '{password}' -AsPlainText -Force; \
             New-LocalUser -Name '{username}' -Password $pw -FullName '{display_name}' -Description 'Created by LiquiDE'"
        );
        Self::powershell(&script)?;

        // Add to Users group.
        let _ = Self::powershell(&format!(
            "Add-LocalGroupMember -Group 'Users' -Member '{username}'"
        ));

        // Add to Administrators if needed.
        if account_type == AccountType::Administrator {
            Self::powershell(&format!(
                "Add-LocalGroupMember -Group 'Administrators' -Member '{username}'"
            ))?;
        }

        self.build_user_account(username)
    }

    fn delete_user(&mut self, uid: u32, delete_home: bool) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        Self::powershell(&format!("Remove-LocalUser -Name '{username}'"))?;

        if delete_home {
            let home = format!("C:\\Users\\{username}");
            let _ = Self::powershell(&format!(
                "Remove-Item -Path '{home}' -Recurse -Force -ErrorAction SilentlyContinue"
            ));
        }

        Ok(())
    }

    fn set_display_name(&mut self, uid: u32, name: &str) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        Self::powershell(&format!(
            "Set-LocalUser -Name '{username}' -FullName '{name}'"
        ))?;
        Ok(())
    }

    fn set_avatar(&mut self, uid: u32, path: &str) -> Result<(), AccountError> {
        let _username = Self::uid_to_username(uid)?;
        // Windows stores account pictures in the user's profile via
        // the User Account Pictures API. For simplicity we store the
        // path reference; real integration would use the Windows API.
        self.avatar_dir = path.to_string();
        Ok(())
    }

    fn change_password(
        &mut self,
        uid: u32,
        _old_password: &str,
        new_password: &str,
    ) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let script = format!(
            "$pw = ConvertTo-SecureString '{new_password}' -AsPlainText -Force; \
             Set-LocalUser -Name '{username}' -Password $pw"
        );
        Self::powershell(&script)?;
        Ok(())
    }

    fn set_auto_login(&mut self, uid: u32, enabled: bool) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        if enabled {
            Self::powershell(&format!(
                "Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' -Name AutoAdminLogon -Value 1; \
                 Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' -Name DefaultUserName -Value '{username}'"
            ))?;
        } else {
            Self::powershell(
                "Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' -Name AutoAdminLogon -Value 0"
            )?;
        }
        Ok(())
    }

    fn lock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        Self::powershell(&format!("Disable-LocalUser -Name '{username}'"))?;
        Ok(())
    }

    fn unlock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        Self::powershell(&format!("Enable-LocalUser -Name '{username}'"))?;
        Ok(())
    }

    fn set_account_type(
        &mut self,
        uid: u32,
        account_type: AccountType,
    ) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        match account_type {
            AccountType::Administrator => {
                Self::powershell(&format!(
                    "Add-LocalGroupMember -Group 'Administrators' -Member '{username}' -ErrorAction SilentlyContinue"
                ))?;
            }
            AccountType::Standard => {
                Self::powershell(&format!(
                    "Remove-LocalGroupMember -Group 'Administrators' -Member '{username}' -ErrorAction SilentlyContinue"
                ))?;
            }
        }
        Ok(())
    }

    fn list_groups(&self) -> Result<Vec<Group>, AccountError> {
        let script = "Get-LocalGroup | ForEach-Object { $g = $_; $members = (Get-LocalGroupMember -Group $g.Name -ErrorAction SilentlyContinue | ForEach-Object { $_.SID.Value.Split('-')[-1] }) -join ','; \"$($g.SID.Value.Split('-')[-1])|$($g.Name)|$members\" }";
        let output = Self::powershell(script)?;
        let mut groups = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() < 2 {
                continue;
            }
            let gid = parts[0].parse::<u32>().unwrap_or(0);
            let name = parts[1].to_string();
            let members: Vec<u32> = if parts.len() > 2 && !parts[2].is_empty() {
                parts[2]
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u32>().ok())
                    .collect()
            } else {
                Vec::new()
            };

            groups.push(Group { gid, name, members });
        }

        Ok(groups)
    }

    fn user_groups(&self, uid: u32) -> Result<Vec<Group>, AccountError> {
        let all_groups = self.list_groups()?;
        Ok(all_groups
            .into_iter()
            .filter(|g| g.members.contains(&uid))
            .collect())
    }

    fn add_to_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let groups = self.list_groups()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;
        Self::powershell(&format!(
            "Add-LocalGroupMember -Group '{}' -Member '{username}'",
            group.name
        ))?;
        Ok(())
    }

    fn remove_from_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = Self::uid_to_username(uid)?;
        let groups = self.list_groups()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;
        Self::powershell(&format!(
            "Remove-LocalGroupMember -Group '{}' -Member '{username}'",
            group.name
        ))?;
        Ok(())
    }

    fn recent_logins(&self, uid: u32, count: usize) -> Result<Vec<LoginEntry>, AccountError> {
        let username = Self::uid_to_username(uid)?;
        // Query the Security event log for logon events (Event ID 4624).
        let script = format!(
            "Get-WinEvent -FilterHashtable @{{LogName='Security';Id=4624}} -MaxEvents 100 -ErrorAction SilentlyContinue | \
             Where-Object {{ $_.Properties[5].Value -eq '{username}' }} | \
             Select-Object -First {count} | \
             ForEach-Object {{ \"$($_.TimeCreated.ToFileTimeUtc())|$($_.Properties[8].Value)|$($_.Properties[18].Value)\" }}"
        );
        let output = Self::powershell(&script).unwrap_or_default();

        let mut entries = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            let timestamp = parts
                .first()
                .and_then(|s| s.parse::<u64>().ok())
                .map(|ft| {
                    // Convert Windows FILETIME to Unix epoch.
                    ft.saturating_sub(116_444_736_000_000_000) / 10_000_000
                })
                .unwrap_or(0);

            let logon_type = parts.get(1).unwrap_or(&"");
            let method = match *logon_type {
                "2" => LoginMethod::Password,   // Interactive
                "10" => LoginMethod::RemoteDesktop,
                "11" => LoginMethod::AutoLogin,  // CachedInteractive
                _ => LoginMethod::Password,
            };

            let ip = parts.get(2).map(|s| s.to_string()).filter(|s| {
                !s.is_empty() && s != "-" && s != "127.0.0.1" && s != "::1"
            });

            entries.push(LoginEntry {
                uid,
                timestamp,
                success: true,
                method,
                ip,
            });
        }

        Ok(entries)
    }
}

struct NetUserInfo {
    full_name: String,
    active: bool,
    last_password_set: Option<u64>,
}

/// Rough parser for Windows locale-dependent date strings.
fn parse_windows_date(date_str: &str) -> Option<u64> {
    // Try to parse MM/DD/YYYY or DD/MM/YYYY or YYYY-MM-DD patterns.
    // This is best-effort; real code should use Win32 SystemTimeToFileTime.
    let parts: Vec<&str> = date_str
        .split(|c: char| c == '/' || c == '-' || c == ' ')
        .collect();
    if parts.len() >= 3 {
        let a: u64 = parts[0].parse().ok()?;
        let b: u64 = parts[1].parse().ok()?;
        let c: u64 = parts[2].parse().ok()?;

        // Heuristic: if first part > 31, it's YYYY-MM-DD.
        let (year, month, day) = if a > 31 {
            (a, b, c)
        } else if c > 31 {
            // MM/DD/YYYY or DD/MM/YYYY — assume MM/DD/YYYY.
            (c, a, b)
        } else {
            return None;
        };

        if year < 1970 || month < 1 || month > 12 || day < 1 || day > 31 {
            return None;
        }

        let years_since_epoch = year - 1970;
        let leap_years = (year - 1969) / 4 - (year - 1901) / 100 + (year - 1601) / 400;
        let days_per_month: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let mut day_of_year: u64 = day - 1;
        for m in 0..(month - 1) {
            day_of_year += days_per_month[m as usize];
            if m == 1 && is_leap {
                day_of_year += 1;
            }
        }
        let total_days = years_since_epoch * 365 + leap_years + day_of_year;
        return Some(total_days * 86400);
    }
    None
}
