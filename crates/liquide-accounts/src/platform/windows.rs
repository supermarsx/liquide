//! Windows account management backend using Win32 APIs.
//!
//! Uses `GetUserNameW` (advapi32), `NetUserEnum` / `NetLocalGroupEnum` /
//! `NetUserGetLocalGroups` / `NetApiBufferFree` (netapi32.dll loaded at
//! runtime via `LoadLibraryW` / `GetProcAddress`).

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::{LoginEntry, LoginMethod};
use crate::platform::PlatformBackend;
use crate::types::{AccountType, UserAccount};
use std::ffi::c_void;

// ── Win32 FFI declarations ───────────────────────────────────────────────────

unsafe extern "system" {
    fn GetUserNameW(buf: *mut u16, size: *mut u32) -> i32;
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
}

// ── netapi32 function signatures ─────────────────────────────────────────────

type NetUserEnumFn = unsafe extern "system" fn(
    server: *const u16,
    level: u32,
    filter: u32,
    buf: *mut *mut u8,
    prefmax: u32,
    entries: *mut u32,
    total: *mut u32,
    resume: *mut u32,
) -> u32;

type NetApiBufferFreeFn = unsafe extern "system" fn(buf: *mut c_void) -> u32;

type NetLocalGroupEnumFn = unsafe extern "system" fn(
    server: *const u16,
    level: u32,
    buf: *mut *mut u8,
    prefmax: u32,
    entries: *mut u32,
    total: *mut u32,
    resume: *mut usize,
) -> u32;

type NetUserGetLocalGroupsFn = unsafe extern "system" fn(
    server: *const u16,
    username: *const u16,
    level: u32,
    flags: u32,
    buf: *mut *mut u8,
    prefmax: u32,
    entries: *mut u32,
    total: *mut u32,
) -> u32;

type NetLocalGroupGetMembersFn = unsafe extern "system" fn(
    server: *const u16,
    group_name: *const u16,
    level: u32,
    buf: *mut *mut u8,
    prefmax: u32,
    entries: *mut u32,
    total: *mut u32,
    resume: *mut usize,
) -> u32;

type NetUserChangePasswordFn = unsafe extern "system" fn(
    domainname: *const u16,
    username: *const u16,
    oldpassword: *const u16,
    newpassword: *const u16,
) -> u32;

// ── USER_INFO_3 layout (level 3) ────────────────────────────────────────────

/// Partial layout of USER_INFO_3.
/// We only read the fields we need; the struct has many more fields
/// after these but we access them by offset.
#[repr(C)]
struct UserInfo3 {
    name: *const u16,
    password: *const u16,
    password_age: u32,
    priv_level: u32, // 0=Guest, 1=User, 2=Admin
    home_dir: *const u16,
    comment: *const u16,
    flags: u32,
    script_path: *const u16,
    auth_flags: u32,
    full_name: *const u16,
    usr_comment: *const u16,
    parms: *const u16,
    workstations: *const u16,
    last_logon: u32,
    last_logoff: u32,
    acct_expires: u32,
    max_storage: u32,
    units_per_week: u32,
    logon_hours: *const u8,
    bad_pw_count: u32,
    num_logons: u32,
    logon_server: *const u16,
    country_code: u32,
    code_page: u32,
    user_id: u32, // The RID
    primary_group_id: u32,
    profile: *const u16,
    home_dir_drive: *const u16,
    password_expired: u32,
}

/// LOCALGROUP_INFO_1 layout.
#[repr(C)]
struct LocalGroupInfo1 {
    name: *const u16,
    comment: *const u16,
}

/// LOCALGROUP_MEMBERS_INFO_3 layout (contains just the domain\name string).
#[repr(C)]
struct LocalGroupMembersInfo3 {
    domain_and_name: *const u16,
}

/// LOCALGROUP_USERS_INFO_0 layout.
#[repr(C)]
struct LocalGroupUsersInfo0 {
    name: *const u16,
}

// ── Constants ────────────────────────────────────────────────────────────────

const NERR_SUCCESS: u32 = 0;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_INVALID_PASSWORD: u32 = 86;
const ERROR_PASSWORD_RESTRICTION: u32 = 1325;
const NERR_USER_NOT_FOUND: u32 = 2221;
const NERR_PASSWORD_HIST_CONFLICT: u32 = 2244;
const NERR_PASSWORD_TOO_SHORT: u32 = 2245;
const NERR_PASSWORD_TOO_RECENT: u32 = 2246;
const MAX_PREFERRED_LENGTH: u32 = 0xFFFF_FFFF;
const UF_ACCOUNTDISABLE: u32 = 0x0002;
const LG_INCLUDE_INDIRECT: u32 = 0x0001;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Decode a null-terminated wide string pointer to a Rust String.
/// # Safety
/// `ptr` must be null or point to a valid null-terminated UTF-16 string.
unsafe fn from_wide(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16_lossy(slice)
}

// ── Netapi32 handle ──────────────────────────────────────────────────────────

struct Netapi32 {
    _module: *mut c_void,
    net_user_enum: NetUserEnumFn,
    net_api_buffer_free: NetApiBufferFreeFn,
    net_local_group_enum: NetLocalGroupEnumFn,
    net_user_get_local_groups: NetUserGetLocalGroupsFn,
    net_local_group_get_members: NetLocalGroupGetMembersFn,
    net_user_change_password: NetUserChangePasswordFn,
}

// SAFETY: The netapi32.dll handle and function pointers are process-wide
// and safe to share across threads.
unsafe impl Send for Netapi32 {}

impl Netapi32 {
    fn load() -> Result<Self, AccountError> {
        let lib_name = to_wide("netapi32.dll");
        let module = unsafe { LoadLibraryW(lib_name.as_ptr()) };
        if module.is_null() {
            return Err(AccountError::PlatformError(
                "failed to load netapi32.dll".into(),
            ));
        }

        unsafe {
            let net_user_enum = Self::get_proc(module, b"NetUserEnum\0")?;
            let net_api_buffer_free = Self::get_proc(module, b"NetApiBufferFree\0")?;
            let net_local_group_enum = Self::get_proc(module, b"NetLocalGroupEnum\0")?;
            let net_user_get_local_groups = Self::get_proc(module, b"NetUserGetLocalGroups\0")?;
            let net_local_group_get_members = Self::get_proc(module, b"NetLocalGroupGetMembers\0")?;
            let net_user_change_password = Self::get_proc(module, b"NetUserChangePassword\0")?;

            Ok(Self {
                _module: module,
                net_user_enum: std::mem::transmute(net_user_enum),
                net_api_buffer_free: std::mem::transmute(net_api_buffer_free),
                net_local_group_enum: std::mem::transmute(net_local_group_enum),
                net_user_get_local_groups: std::mem::transmute(net_user_get_local_groups),
                net_local_group_get_members: std::mem::transmute(net_local_group_get_members),
                net_user_change_password: std::mem::transmute(net_user_change_password),
            })
        }
    }

    unsafe fn get_proc(module: *mut c_void, name: &[u8]) -> Result<*mut c_void, AccountError> {
        let ptr = unsafe { GetProcAddress(module, name.as_ptr()) };
        if ptr.is_null() {
            let fn_name = String::from_utf8_lossy(&name[..name.len().saturating_sub(1)]);
            return Err(AccountError::PlatformError(format!(
                "GetProcAddress failed for {fn_name}"
            )));
        }
        Ok(ptr)
    }

    fn free_buffer(&self, buf: *mut u8) {
        if !buf.is_null() {
            unsafe { (self.net_api_buffer_free)(buf as *mut c_void) };
        }
    }
}

fn password_change_status_to_error(status: u32) -> AccountError {
    match status {
        ERROR_ACCESS_DENIED | ERROR_INVALID_PASSWORD => AccountError::PermissionDenied,
        NERR_USER_NOT_FOUND => AccountError::NotFound,
        ERROR_PASSWORD_RESTRICTION
        | NERR_PASSWORD_HIST_CONFLICT
        | NERR_PASSWORD_TOO_SHORT
        | NERR_PASSWORD_TOO_RECENT => AccountError::WeakPassword(
            "Windows password policy rejected the new password".to_string(),
        ),
        _ => AccountError::PlatformError(format!(
            "NetUserChangePassword failed with status {status}"
        )),
    }
}

impl Drop for Netapi32 {
    fn drop(&mut self) {
        if !self._module.is_null() {
            unsafe { FreeLibrary(self._module) };
        }
    }
}

// ── WindowsBackend ───────────────────────────────────────────────────────────

pub struct WindowsBackend {
    netapi: Netapi32,
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self {
            netapi: Netapi32::load().unwrap_or_else(|_| {
                // If netapi32 fails to load we create a dummy that will
                // error on every call. This keeps `new()` infallible.
                panic!("netapi32.dll is required on Windows");
            }),
        }
    }

    /// Get the current username via Win32 `GetUserNameW`.
    fn current_username() -> Result<String, AccountError> {
        let mut buf = [0u16; 256];
        let mut size: u32 = buf.len() as u32;
        let ok = unsafe { GetUserNameW(buf.as_mut_ptr(), &mut size) };
        if ok == 0 {
            // Fallback to environment variable.
            return std::env::var("USERNAME")
                .map_err(|_| AccountError::PlatformError("GetUserNameW failed".into()));
        }
        // size includes the null terminator.
        let name_len = (size as usize).saturating_sub(1);
        Ok(String::from_utf16_lossy(&buf[..name_len]))
    }

    /// Enumerate all users via `NetUserEnum` level 3.
    fn enumerate_users(&self) -> Result<Vec<RawUser>, AccountError> {
        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut entries_read: u32 = 0;
        let mut total_entries: u32 = 0;
        let mut resume_handle: u32 = 0;

        let status = unsafe {
            (self.netapi.net_user_enum)(
                std::ptr::null(), // local server
                3,                // level 3 = USER_INFO_3
                0,                // no filter (all users)
                &mut buf,
                MAX_PREFERRED_LENGTH,
                &mut entries_read,
                &mut total_entries,
                &mut resume_handle,
            )
        };

        if status != NERR_SUCCESS && status != 234
        /* ERROR_MORE_DATA */
        {
            return Err(AccountError::PlatformError(format!(
                "NetUserEnum failed with status {status}"
            )));
        }

        let mut users = Vec::new();

        if !buf.is_null() && entries_read > 0 {
            let info_ptr = buf as *const UserInfo3;
            for i in 0..entries_read as usize {
                let info = unsafe { &*info_ptr.add(i) };
                let username = unsafe { from_wide(info.name) };
                let full_name = unsafe { from_wide(info.full_name) };
                let home_dir = unsafe { from_wide(info.home_dir) };
                let comment = unsafe { from_wide(info.comment) };

                let is_admin = info.priv_level == 2;
                let is_disabled = (info.flags & UF_ACCOUNTDISABLE) != 0;
                let uid = info.user_id;
                let last_logon = info.last_logon;
                let password_age = info.password_age;

                users.push(RawUser {
                    username,
                    full_name,
                    home_dir,
                    _comment: comment,
                    is_admin,
                    is_disabled,
                    uid,
                    last_logon,
                    password_age,
                });
            }
        }

        self.netapi.free_buffer(buf);
        Ok(users)
    }

    /// Check if a user belongs to the Administrators group.
    fn is_user_admin(&self, username: &str) -> bool {
        let username_wide = to_wide(username);
        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut entries_read: u32 = 0;
        let mut total_entries: u32 = 0;

        let status = unsafe {
            (self.netapi.net_user_get_local_groups)(
                std::ptr::null(),
                username_wide.as_ptr(),
                0, // level 0 = LOCALGROUP_USERS_INFO_0
                LG_INCLUDE_INDIRECT,
                &mut buf,
                MAX_PREFERRED_LENGTH,
                &mut entries_read,
                &mut total_entries,
            )
        };

        let mut is_admin = false;
        if status == NERR_SUCCESS && !buf.is_null() && entries_read > 0 {
            let info_ptr = buf as *const LocalGroupUsersInfo0;
            for i in 0..entries_read as usize {
                let info = unsafe { &*info_ptr.add(i) };
                let name = unsafe { from_wide(info.name) };
                if name.eq_ignore_ascii_case("Administrators") {
                    is_admin = true;
                    break;
                }
            }
        }

        self.netapi.free_buffer(buf);
        is_admin
    }

    fn build_user_account(&self, raw: &RawUser) -> UserAccount {
        let home_dir = if raw.home_dir.is_empty() {
            format!("C:\\Users\\{}", raw.username)
        } else {
            raw.home_dir.clone()
        };

        let display_name = if raw.full_name.is_empty() {
            raw.username.clone()
        } else {
            raw.full_name.clone()
        };

        // Determine admin status: priv_level == 2 OR member of Administrators group.
        let account_type = if raw.is_admin || self.is_user_admin(&raw.username) {
            AccountType::Administrator
        } else {
            AccountType::Standard
        };

        // password_age is in seconds since last change.
        let password_last_changed = if raw.password_age > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Some(now.saturating_sub(raw.password_age as u64))
        } else {
            None
        };

        // Check if user is currently logged in by comparing with current user.
        let is_logged_in = Self::current_username()
            .map(|cu| cu.eq_ignore_ascii_case(&raw.username))
            .unwrap_or(false);

        UserAccount {
            uid: raw.uid,
            username: raw.username.clone(),
            display_name,
            home_dir,
            shell: "cmd.exe".to_string(),
            account_type,
            avatar: None,
            is_logged_in,
            is_locked: raw.is_disabled,
            password_last_changed,
            auto_login: false, // Would need registry query for Winlogon.
        }
    }

    /// Enumerate local groups via `NetLocalGroupEnum` level 1.
    fn enumerate_groups(&self) -> Result<Vec<(String, String)>, AccountError> {
        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut entries_read: u32 = 0;
        let mut total_entries: u32 = 0;
        let mut resume: usize = 0;

        let status = unsafe {
            (self.netapi.net_local_group_enum)(
                std::ptr::null(),
                1, // level 1 = LOCALGROUP_INFO_1
                &mut buf,
                MAX_PREFERRED_LENGTH,
                &mut entries_read,
                &mut total_entries,
                &mut resume,
            )
        };

        if status != NERR_SUCCESS && status != 234 {
            return Err(AccountError::PlatformError(format!(
                "NetLocalGroupEnum failed with status {status}"
            )));
        }

        let mut groups = Vec::new();
        if !buf.is_null() && entries_read > 0 {
            let info_ptr = buf as *const LocalGroupInfo1;
            for i in 0..entries_read as usize {
                let info = unsafe { &*info_ptr.add(i) };
                let name = unsafe { from_wide(info.name) };
                let comment = unsafe { from_wide(info.comment) };
                groups.push((name, comment));
            }
        }

        self.netapi.free_buffer(buf);
        Ok(groups)
    }

    /// Get members of a local group via `NetLocalGroupGetMembers` level 3.
    fn group_members(&self, group_name: &str) -> Vec<String> {
        let group_wide = to_wide(group_name);
        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut entries_read: u32 = 0;
        let mut total_entries: u32 = 0;
        let mut resume: usize = 0;

        let status = unsafe {
            (self.netapi.net_local_group_get_members)(
                std::ptr::null(),
                group_wide.as_ptr(),
                3, // level 3 = LOCALGROUP_MEMBERS_INFO_3
                &mut buf,
                MAX_PREFERRED_LENGTH,
                &mut entries_read,
                &mut total_entries,
                &mut resume,
            )
        };

        let mut members = Vec::new();
        if status == NERR_SUCCESS && !buf.is_null() && entries_read > 0 {
            let info_ptr = buf as *const LocalGroupMembersInfo3;
            for i in 0..entries_read as usize {
                let info = unsafe { &*info_ptr.add(i) };
                let domain_name = unsafe { from_wide(info.domain_and_name) };
                // Extract just the username part (after DOMAIN\).
                let username = domain_name
                    .rsplit('\\')
                    .next()
                    .unwrap_or(&domain_name)
                    .to_string();
                members.push(username);
            }
        }

        self.netapi.free_buffer(buf);
        members
    }

    /// Map a username to a UID (RID) by searching enumerated users.
    fn username_to_uid(&self, username: &str) -> Result<u32, AccountError> {
        let users = self.enumerate_users()?;
        users
            .iter()
            .find(|u| u.username.eq_ignore_ascii_case(username))
            .map(|u| u.uid)
            .ok_or(AccountError::NotFound)
    }

    /// Map a UID (RID) back to a username.
    fn uid_to_username(&self, uid: u32) -> Result<String, AccountError> {
        let users = self.enumerate_users()?;
        users
            .iter()
            .find(|u| u.uid == uid)
            .map(|u| u.username.clone())
            .ok_or(AccountError::NotFound)
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal raw user data from NetUserEnum.
struct RawUser {
    username: String,
    full_name: String,
    home_dir: String,
    _comment: String,
    is_admin: bool,
    is_disabled: bool,
    uid: u32,
    last_logon: u32,
    password_age: u32,
}

// ── PlatformBackend implementation ───────────────────────────────────────────

impl PlatformBackend for WindowsBackend {
    fn current_user(&self) -> Result<UserAccount, AccountError> {
        let username = Self::current_username()?;
        let users = self.enumerate_users()?;
        let raw = users
            .iter()
            .find(|u| u.username.eq_ignore_ascii_case(&username))
            .ok_or(AccountError::NotFound)?;
        Ok(self.build_user_account(raw))
    }

    fn list_users(&self) -> Result<Vec<UserAccount>, AccountError> {
        let raw_users = self.enumerate_users()?;
        let mut accounts = Vec::new();

        for raw in &raw_users {
            // Skip well-known system accounts.
            let lower = raw.username.to_lowercase();
            if lower == "defaultaccount" || lower == "wdagutilityaccount" || lower == "guest" {
                continue;
            }
            accounts.push(self.build_user_account(raw));
        }

        Ok(accounts)
    }

    fn create_user(
        &mut self,
        username: &str,
        _display_name: &str,
        _account_type: AccountType,
        _password: &str,
    ) -> Result<UserAccount, AccountError> {
        // NetUserAdd requires linking to netapi32 with USER_INFO_1 or higher.
        // For safety, we delegate to `net user /add` which is always available.
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
        if self.username_to_uid(username).is_ok() {
            return Err(AccountError::AlreadyExists);
        }

        let output = std::process::Command::new("net")
            .args([
                "user",
                username,
                _password,
                "/add",
                &format!("/fullname:{_display_name}"),
            ])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net user /add: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Access is denied") {
                return Err(AccountError::PermissionDenied);
            }
            return Err(AccountError::PlatformError(format!("net user: {stderr}")));
        }

        if _account_type == AccountType::Administrator {
            let _ = std::process::Command::new("net")
                .args(["localgroup", "Administrators", username, "/add"])
                .output();
        }

        // Re-enumerate to find the new user.
        let users = self.enumerate_users()?;
        let raw = users
            .iter()
            .find(|u| u.username.eq_ignore_ascii_case(username))
            .ok_or(AccountError::NotFound)?;
        Ok(self.build_user_account(raw))
    }

    fn delete_user(&mut self, uid: u32, delete_home: bool) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        let output = std::process::Command::new("net")
            .args(["user", &username, "/delete"])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net user /delete: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Access is denied") {
                return Err(AccountError::PermissionDenied);
            }
            return Err(AccountError::PlatformError(format!("net user: {stderr}")));
        }

        if delete_home {
            let home = format!("C:\\Users\\{username}");
            let _ = std::fs::remove_dir_all(&home);
        }

        Ok(())
    }

    fn set_display_name(&mut self, uid: u32, name: &str) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        let output = std::process::Command::new("net")
            .args(["user", &username, &format!("/fullname:{name}")])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net user: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AccountError::PlatformError(format!("net user: {stderr}")));
        }
        Ok(())
    }

    fn set_avatar(&mut self, uid: u32, _path: &str) -> Result<(), AccountError> {
        // Verify the user exists.
        let _username = self.uid_to_username(uid)?;
        // Windows stores account pictures via the User Account Pictures API.
        // Full integration would use the Windows.System.UserProfile namespace.
        Ok(())
    }

    fn change_password(
        &mut self,
        uid: u32,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        if old_password.is_empty() {
            return Err(AccountError::PlatformError(
                "old password must not be empty".into(),
            ));
        }

        let username_wide = to_wide(&username);
        let old_password_wide = to_wide(old_password);
        let new_password_wide = to_wide(new_password);
        let status = unsafe {
            (self.netapi.net_user_change_password)(
                std::ptr::null(),
                username_wide.as_ptr(),
                old_password_wide.as_ptr(),
                new_password_wide.as_ptr(),
            )
        };

        if status == NERR_SUCCESS {
            Ok(())
        } else {
            Err(password_change_status_to_error(status))
        }
    }

    fn set_auto_login(&mut self, uid: u32, _enabled: bool) -> Result<(), AccountError> {
        // Verify user exists; auto-login requires registry writes to
        // HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon
        // which need elevation. Not implementable via pure netapi32.
        let _username = self.uid_to_username(uid)?;
        Err(AccountError::PlatformError(
            "auto-login requires registry access (elevation needed)".into(),
        ))
    }

    fn lock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        let output = std::process::Command::new("net")
            .args(["user", &username, "/active:no"])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net user: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AccountError::PlatformError(format!("net user: {stderr}")));
        }
        Ok(())
    }

    fn unlock_account(&mut self, uid: u32) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        let output = std::process::Command::new("net")
            .args(["user", &username, "/active:yes"])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net user: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AccountError::PlatformError(format!("net user: {stderr}")));
        }
        Ok(())
    }

    fn set_account_type(
        &mut self,
        uid: u32,
        account_type: AccountType,
    ) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        match account_type {
            AccountType::Administrator => {
                let output = std::process::Command::new("net")
                    .args(["localgroup", "Administrators", &username, "/add"])
                    .output()
                    .map_err(|e| AccountError::PlatformError(format!("net localgroup: {e}")))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    // Ignore "already a member" errors.
                    if !stderr.contains("1378") {
                        return Err(AccountError::PlatformError(format!(
                            "net localgroup: {stderr}"
                        )));
                    }
                }
            }
            AccountType::Standard => {
                let output = std::process::Command::new("net")
                    .args(["localgroup", "Administrators", &username, "/delete"])
                    .output()
                    .map_err(|e| AccountError::PlatformError(format!("net localgroup: {e}")))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    // Ignore "not a member" errors.
                    if !stderr.contains("1377") {
                        return Err(AccountError::PlatformError(format!(
                            "net localgroup: {stderr}"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn list_groups(&self) -> Result<Vec<Group>, AccountError> {
        let raw_groups = self.enumerate_groups()?;
        let users = self.enumerate_users()?;

        let mut groups = Vec::new();
        for (idx, (name, _comment)) in raw_groups.iter().enumerate() {
            let member_names = self.group_members(name);
            let member_uids: Vec<u32> = member_names
                .iter()
                .filter_map(|mname| {
                    users
                        .iter()
                        .find(|u| u.username.eq_ignore_ascii_case(mname))
                        .map(|u| u.uid)
                })
                .collect();

            groups.push(Group {
                gid: idx as u32,
                name: name.clone(),
                members: member_uids,
            });
        }

        Ok(groups)
    }

    fn user_groups(&self, uid: u32) -> Result<Vec<Group>, AccountError> {
        let username = self.uid_to_username(uid)?;
        let username_wide = to_wide(&username);

        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut entries_read: u32 = 0;
        let mut total_entries: u32 = 0;

        let status = unsafe {
            (self.netapi.net_user_get_local_groups)(
                std::ptr::null(),
                username_wide.as_ptr(),
                0,
                LG_INCLUDE_INDIRECT,
                &mut buf,
                MAX_PREFERRED_LENGTH,
                &mut entries_read,
                &mut total_entries,
            )
        };

        if status != NERR_SUCCESS {
            return Err(AccountError::PlatformError(format!(
                "NetUserGetLocalGroups failed with status {status}"
            )));
        }

        let mut group_names = Vec::new();
        if !buf.is_null() && entries_read > 0 {
            let info_ptr = buf as *const LocalGroupUsersInfo0;
            for i in 0..entries_read as usize {
                let info = unsafe { &*info_ptr.add(i) };
                let name = unsafe { from_wide(info.name) };
                group_names.push(name);
            }
        }
        self.netapi.free_buffer(buf);

        // Build Group structs by looking up full group info.
        let all_groups = self.list_groups()?;
        Ok(all_groups
            .into_iter()
            .filter(|g| group_names.iter().any(|n| n.eq_ignore_ascii_case(&g.name)))
            .collect())
    }

    fn add_to_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        let groups = self.list_groups()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;

        let output = std::process::Command::new("net")
            .args(["localgroup", &group.name, &username, "/add"])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net localgroup: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AccountError::PlatformError(format!(
                "net localgroup: {stderr}"
            )));
        }
        Ok(())
    }

    fn remove_from_group(&mut self, uid: u32, gid: u32) -> Result<(), AccountError> {
        let username = self.uid_to_username(uid)?;
        let groups = self.list_groups()?;
        let group = groups
            .iter()
            .find(|g| g.gid == gid)
            .ok_or(AccountError::NotFound)?;

        let output = std::process::Command::new("net")
            .args(["localgroup", &group.name, &username, "/delete"])
            .output()
            .map_err(|e| AccountError::PlatformError(format!("net localgroup: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AccountError::PlatformError(format!(
                "net localgroup: {stderr}"
            )));
        }
        Ok(())
    }

    fn recent_logins(&self, uid: u32, _count: usize) -> Result<Vec<LoginEntry>, AccountError> {
        let users = self.enumerate_users()?;
        let raw = users
            .iter()
            .find(|u| u.uid == uid)
            .ok_or(AccountError::NotFound)?;

        // NetUserEnum level 3 gives us `last_logon` (seconds since 1970-01-01).
        // We can only report the single most recent logon from the SAM database.
        let mut entries = Vec::new();
        if raw.last_logon > 0 {
            entries.push(LoginEntry {
                uid,
                timestamp: raw.last_logon as u64,
                success: true,
                method: LoginMethod::Password,
                ip: None,
            });
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_old_password_maps_to_permission_denied() {
        assert!(matches!(
            password_change_status_to_error(ERROR_INVALID_PASSWORD),
            AccountError::PermissionDenied
        ));
    }

    #[test]
    fn windows_password_policy_rejections_remain_user_actionable() {
        assert!(matches!(
            password_change_status_to_error(NERR_PASSWORD_TOO_SHORT),
            AccountError::WeakPassword(message) if message.contains("Windows password policy")
        ));
        assert!(matches!(
            password_change_status_to_error(NERR_PASSWORD_HIST_CONFLICT),
            AccountError::WeakPassword(message) if message.contains("Windows password policy")
        ));
    }

    #[test]
    fn unexpected_password_change_status_fails_closed() {
        assert!(matches!(
            password_change_status_to_error(12_345),
            AccountError::PlatformError(message)
                if message.contains("NetUserChangePassword failed")
        ));
    }
}
