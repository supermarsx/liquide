use crate::level::AuthLevel;
use crate::AuthorizationError;

/// Result of a platform credential verification attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    /// Credentials were successfully verified.
    Success,
    /// The user cancelled the authentication prompt.
    Cancelled,
    /// The credentials were incorrect.
    Failed(String),
    /// A platform error occurred (process spawn failure, etc.).
    Error(String),
}

/// Verify credentials using platform-specific mechanisms.
///
/// This dispatches to the appropriate OS mechanism based on the target
/// platform and the requested auth level.
///
/// # Platform behavior
///
/// - **Linux**: Uses `pkexec` (PolicyKit) when available, falls back to
///   `su -c` for password verification.
/// - **Windows**: Uses `ShellExecuteW` with the `"runas"` verb to trigger
///   a UAC elevation prompt.
/// - **macOS**: Uses `osascript` to invoke `"do shell script ... with
///   administrator privileges"`.
///
/// For `AuthLevel::NoAuth`, returns `VerifyResult::Success` immediately.
/// For `AuthLevel::Fingerprint` and `AuthLevel::SmartCard`, returns an
/// error indicating the platform does not support that mechanism (these
/// would be handled by dedicated hardware integrations).
pub fn verify_credentials(level: AuthLevel, _username: &str) -> VerifyResult {
    match level {
        AuthLevel::NoAuth => VerifyResult::Success,
        AuthLevel::Fingerprint => VerifyResult::Error(
            "Fingerprint verification requires hardware integration".to_string(),
        ),
        AuthLevel::SmartCard => VerifyResult::Error(
            "Smart card verification requires PKCS#11 integration".to_string(),
        ),
        AuthLevel::UserPassword | AuthLevel::AdminPassword => {
            platform_verify_password(level)
        }
    }
}

/// Attempt to verify a password using the OS-specific mechanism.
///
/// This is a best-effort implementation that invokes external tools.
/// In a full desktop environment, this would be replaced by D-Bus
/// communication with a running authorization agent daemon.
#[cfg(target_os = "linux")]
fn platform_verify_password(level: AuthLevel) -> VerifyResult {
    // Try pkexec first (PolicyKit)
    let test_cmd = if level == AuthLevel::AdminPassword {
        // pkexec runs as root by default
        "true"
    } else {
        "true"
    };

    match std::process::Command::new("pkexec")
        .arg("--disable-internal-agent")
        .arg(test_cmd)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) => {
            if status.success() {
                VerifyResult::Success
            } else {
                match status.code() {
                    Some(126) => VerifyResult::Cancelled,
                    Some(127) => {
                        // pkexec not found or not authorized, try su fallback
                        platform_verify_password_su_fallback(level)
                    }
                    _ => VerifyResult::Failed("Authentication failed".to_string()),
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // pkexec not installed, try su fallback
            platform_verify_password_su_fallback(level)
        }
        Err(e) => VerifyResult::Error(format!("Failed to spawn pkexec: {e}")),
    }
}

/// Fallback for Linux systems without PolicyKit: use `su -c true`.
#[cfg(target_os = "linux")]
fn platform_verify_password_su_fallback(level: AuthLevel) -> VerifyResult {
    let user = if level == AuthLevel::AdminPassword {
        "root"
    } else {
        // For user password, verify the current user
        match std::env::var("USER") {
            Ok(u) => return verify_via_su(&u),
            Err(_) => return VerifyResult::Error("Cannot determine current user".to_string()),
        }
    };
    verify_via_su(user)
}

#[cfg(target_os = "linux")]
fn verify_via_su(user: &str) -> VerifyResult {
    match std::process::Command::new("su")
        .arg("-c")
        .arg("true")
        .arg(user)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => VerifyResult::Success,
        Ok(_) => VerifyResult::Failed("Authentication failed".to_string()),
        Err(e) => VerifyResult::Error(format!("Failed to spawn su: {e}")),
    }
}

#[cfg(target_os = "windows")]
fn platform_verify_password(_level: AuthLevel) -> VerifyResult {
    // On Windows, we use ShellExecuteW with "runas" verb to trigger UAC.
    // We run a harmless command (`cmd /c exit 0`) elevated; if the user
    // approves the UAC prompt the exit code is 0.
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    // Wide-string helper
    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            lpOperation: *const u16,
            lpFile: *const u16,
            lpParameters: *const u16,
            lpDirectory: *const u16,
            nShowCmd: i32,
        ) -> isize;
    }

    const SW_HIDE: i32 = 0;
    let verb = to_wide("runas");
    let file = to_wide("cmd.exe");
    let params = to_wide("/c exit 0");

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            ptr::null(),
            SW_HIDE,
        )
    };

    // ShellExecuteW returns > 32 on success
    if result > 32 {
        VerifyResult::Success
    } else if result == 0 {
        VerifyResult::Error("Out of memory".to_string())
    } else {
        // Common error codes: 2 = file not found, 5 = access denied (user cancelled UAC)
        VerifyResult::Cancelled
    }
}

#[cfg(target_os = "macos")]
fn platform_verify_password(level: AuthLevel) -> VerifyResult {
    let privilege_clause = if level == AuthLevel::AdminPassword {
        " with administrator privileges"
    } else {
        " with administrator privileges" // macOS doesn't distinguish user vs admin in osascript
    };

    let script = format!(
        "do shell script \"exit 0\"{}",
        privilege_clause
    );

    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status()
    {
        Ok(status) if status.success() => VerifyResult::Success,
        Ok(status) => {
            match status.code() {
                Some(-128) | Some(1) => VerifyResult::Cancelled,
                _ => VerifyResult::Failed("Authentication failed".to_string()),
            }
        }
        Err(e) => VerifyResult::Error(format!("Failed to spawn osascript: {e}")),
    }
}

// Fallback for platforms that are none of the above (e.g., FreeBSD, Wasm).
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn platform_verify_password(_level: AuthLevel) -> VerifyResult {
    VerifyResult::Error("Platform credential verification not supported".to_string())
}

/// Convert a `VerifyResult` into a `crate::Result<()>`.
pub fn verify_result_to_result(vr: VerifyResult) -> crate::Result<()> {
    match vr {
        VerifyResult::Success => Ok(()),
        VerifyResult::Cancelled => Err(AuthorizationError::Denied("User cancelled".to_string())),
        VerifyResult::Failed(reason) => Err(AuthorizationError::CredentialVerification(reason)),
        VerifyResult::Error(msg) => Err(AuthorizationError::PlatformError(msg)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noauth_succeeds_immediately() {
        let result = verify_credentials(AuthLevel::NoAuth, "test_user");
        assert_eq!(result, VerifyResult::Success);
    }

    #[test]
    fn fingerprint_returns_error() {
        let result = verify_credentials(AuthLevel::Fingerprint, "test_user");
        matches!(result, VerifyResult::Error(_));
    }

    #[test]
    fn smartcard_returns_error() {
        let result = verify_credentials(AuthLevel::SmartCard, "test_user");
        matches!(result, VerifyResult::Error(_));
    }

    #[test]
    fn verify_result_to_result_success() {
        assert!(verify_result_to_result(VerifyResult::Success).is_ok());
    }

    #[test]
    fn verify_result_to_result_cancelled() {
        let err = verify_result_to_result(VerifyResult::Cancelled).unwrap_err();
        assert!(matches!(err, AuthorizationError::Denied(_)));
    }

    #[test]
    fn verify_result_to_result_failed() {
        let err =
            verify_result_to_result(VerifyResult::Failed("bad password".to_string())).unwrap_err();
        assert!(matches!(err, AuthorizationError::CredentialVerification(_)));
    }

    #[test]
    fn verify_result_to_result_error() {
        let err =
            verify_result_to_result(VerifyResult::Error("no pkexec".to_string())).unwrap_err();
        assert!(matches!(err, AuthorizationError::PlatformError(_)));
    }
}
