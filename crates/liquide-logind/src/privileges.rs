//! Privilege and environment management for standalone compositor startup.

use std::collections::HashMap;

use crate::error::{LogindError, Result};

/// Privilege management utilities for compositor session setup.
///
/// All methods are static — this is a namespace, not a stateful object.
pub struct Privileges;

impl Privileges {
    /// Check whether the current process is running as root (euid == 0).
    pub fn is_root() -> bool {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: geteuid is always safe to call.
            unsafe { libc::geteuid() == 0 }
        }

        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Return the effective user ID of the current process.
    pub fn effective_uid() -> u32 {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: geteuid is always safe to call.
            unsafe { libc::geteuid() }
        }

        #[cfg(not(target_os = "linux"))]
        {
            1000
        }
    }

    /// Drop privileges from root to the specified user/group.
    ///
    /// Calls `setgid` then `setuid` so the process runs as the target user.
    /// On non-Linux platforms this returns `NotSupported`.
    pub fn drop_to_user(uid: u32, gid: u32) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: setgid/setuid with valid ids.
            let ret = unsafe { libc::setgid(gid) };
            if ret != 0 {
                return Err(LogindError::Privilege(format!(
                    "setgid({gid}) failed: {}",
                    std::io::Error::last_os_error()
                )));
            }
            let ret = unsafe { libc::setuid(uid) };
            if ret != 0 {
                return Err(LogindError::Privilege(format!(
                    "setuid({uid}) failed: {}",
                    std::io::Error::last_os_error()
                )));
            }
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (uid, gid);
            Err(LogindError::NotSupported)
        }
    }

    /// Ensure the XDG_RUNTIME_DIR for the given user exists and return its path.
    ///
    /// On Linux, the standard location is `/run/user/{uid}`.
    /// On non-Linux, returns a placeholder path.
    pub fn setup_runtime_dir(uid: u32) -> Result<String> {
        let path = format!("/run/user/{uid}");

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::os::unix::fs::PermissionsExt;

            fs::create_dir_all(&path).map_err(|e| {
                LogindError::Privilege(format!("failed to create {path}: {e}"))
            })?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(|e| {
                LogindError::Privilege(format!("failed to chmod {path}: {e}"))
            })?;
        }

        #[cfg(not(target_os = "linux"))]
        {
            tracing::debug!("setup_runtime_dir({uid}): stub, returning {path}");
        }

        Ok(path)
    }

    /// Build the standard environment variables map for a compositor session.
    ///
    /// Includes `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `XDG_SESSION_TYPE`, etc.
    pub fn setup_environment(uid: u32) -> HashMap<String, String> {
        let runtime_dir = format!("/run/user/{uid}");

        let mut env = HashMap::new();
        env.insert("XDG_RUNTIME_DIR".to_string(), runtime_dir);
        env.insert("XDG_SESSION_TYPE".to_string(), "wayland".to_string());
        env.insert("WAYLAND_DISPLAY".to_string(), "wayland-0".to_string());
        env.insert(
            "XDG_CURRENT_DESKTOP".to_string(),
            "LiquiDE".to_string(),
        );
        env.insert(
            "XDG_SESSION_DESKTOP".to_string(),
            "liquide".to_string(),
        );
        env
    }
}
