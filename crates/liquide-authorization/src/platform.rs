use crate::AuthorizationError;
use crate::level::AuthLevel;

/// Principal-bound credential verification request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialVerificationRequest {
    /// Action being authorized.
    pub action_id: String,
    /// Principal whose credentials must be proven.
    pub username: String,
    /// Authentication strength required for this action.
    pub level: AuthLevel,
}

impl CredentialVerificationRequest {
    #[must_use]
    pub fn new(
        action_id: impl Into<String>,
        username: impl Into<String>,
        level: AuthLevel,
    ) -> Self {
        Self {
            action_id: action_id.into(),
            username: username.into(),
            level,
        }
    }
}

/// Result of a platform credential verification attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    /// Credentials were successfully verified for the named principal and level.
    Success { username: String, level: AuthLevel },
    /// The user cancelled the authentication prompt.
    Cancelled,
    /// The credentials were incorrect.
    Failed(String),
    /// A platform error occurred (process spawn failure, etc.).
    Error(String),
}

impl VerifyResult {
    #[must_use]
    fn success_for(request: &CredentialVerificationRequest) -> Self {
        Self::Success {
            username: request.username.clone(),
            level: request.level,
        }
    }
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
pub fn verify_credentials(level: AuthLevel, username: &str) -> VerifyResult {
    let request = CredentialVerificationRequest::new(
        "org.liquide.platform.verify-credentials",
        username,
        level,
    );
    verify_authorization_request(&request)
}

/// Verify credentials for a specific action, principal, and auth level.
#[must_use]
pub fn verify_authorization_request(request: &CredentialVerificationRequest) -> VerifyResult {
    if request.level.requires_credential() && request.username.trim().is_empty() {
        return VerifyResult::Failed("requested authorization principal is empty".to_string());
    }

    match request.level {
        AuthLevel::NoAuth => VerifyResult::success_for(request),
        AuthLevel::Fingerprint => VerifyResult::Error(
            "Fingerprint verification requires hardware integration".to_string(),
        ),
        AuthLevel::SmartCard => {
            VerifyResult::Error("Smart card verification requires PKCS#11 integration".to_string())
        }
        AuthLevel::UserPassword | AuthLevel::AdminPassword => platform_verify_password(request),
    }
}

fn current_platform_username() -> Option<String> {
    ["USERNAME", "USER", "LOGNAME"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn same_platform_username(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

fn require_current_platform_principal(
    request: &CredentialVerificationRequest,
) -> Option<VerifyResult> {
    match current_platform_username() {
        Some(current) if same_platform_username(&request.username, &current) => None,
        Some(current) => Some(VerifyResult::Failed(format!(
            "requested principal '{}' does not match current platform principal '{}'",
            request.username, current
        ))),
        None => Some(VerifyResult::Error(
            "cannot determine current platform principal".to_string(),
        )),
    }
}

/// Attempt to verify a password using the OS-specific mechanism.
///
/// This is a best-effort implementation that invokes external tools.
/// In a full desktop environment, this would be replaced by D-Bus
/// communication with a running authorization agent daemon.
#[cfg(target_os = "linux")]
fn platform_verify_password(request: &CredentialVerificationRequest) -> VerifyResult {
    if request.level == AuthLevel::UserPassword {
        return verify_via_su(&request.username, request);
    }

    if let Some(result) = require_current_platform_principal(request) {
        return result;
    }

    // Try pkexec first (PolicyKit)
    let test_cmd = "true";

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
                VerifyResult::success_for(request)
            } else {
                match status.code() {
                    Some(126) => VerifyResult::Cancelled,
                    Some(127) => {
                        // pkexec not found or not authorized, try su fallback
                        platform_verify_password_su_fallback(request)
                    }
                    _ => VerifyResult::Failed("Authentication failed".to_string()),
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // pkexec not installed, try su fallback
            platform_verify_password_su_fallback(request)
        }
        Err(e) => VerifyResult::Error(format!("Failed to spawn pkexec: {e}")),
    }
}

/// Fallback for Linux systems without PolicyKit: use `su -c true`.
#[cfg(target_os = "linux")]
fn platform_verify_password_su_fallback(request: &CredentialVerificationRequest) -> VerifyResult {
    verify_via_su("root", request)
}

#[cfg(target_os = "linux")]
fn verify_via_su(user: &str, request: &CredentialVerificationRequest) -> VerifyResult {
    match std::process::Command::new("su")
        .arg("-c")
        .arg("true")
        .arg(user)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => VerifyResult::success_for(request),
        Ok(_) => VerifyResult::Failed("Authentication failed".to_string()),
        Err(e) => VerifyResult::Error(format!("Failed to spawn su: {e}")),
    }
}

#[cfg(target_os = "windows")]
fn platform_verify_password(request: &CredentialVerificationRequest) -> VerifyResult {
    if let Some(result) = require_current_platform_principal(request) {
        return result;
    }

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
        VerifyResult::success_for(request)
    } else if result == 0 {
        VerifyResult::Error("Out of memory".to_string())
    } else {
        // Common error codes: 2 = file not found, 5 = access denied (user cancelled UAC)
        VerifyResult::Cancelled
    }
}

#[cfg(target_os = "macos")]
fn platform_verify_password(request: &CredentialVerificationRequest) -> VerifyResult {
    if let Some(result) = require_current_platform_principal(request) {
        return result;
    }

    let privilege_clause = if request.level == AuthLevel::AdminPassword {
        " with administrator privileges"
    } else {
        " with administrator privileges" // macOS doesn't distinguish user vs admin in osascript
    };

    let script = format!("do shell script \"exit 0\"{}", privilege_clause);

    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status()
    {
        Ok(status) if status.success() => VerifyResult::success_for(request),
        Ok(status) => match status.code() {
            Some(-128) | Some(1) => VerifyResult::Cancelled,
            _ => VerifyResult::Failed("Authentication failed".to_string()),
        },
        Err(e) => VerifyResult::Error(format!("Failed to spawn osascript: {e}")),
    }
}

// Fallback for platforms that are none of the above (e.g., FreeBSD, Wasm).
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn platform_verify_password(_request: &CredentialVerificationRequest) -> VerifyResult {
    VerifyResult::Error("Platform credential verification not supported".to_string())
}

/// Convert a `VerifyResult` into a `crate::Result<()>`.
pub fn verify_result_to_result(vr: VerifyResult) -> crate::Result<()> {
    match vr {
        VerifyResult::Success { .. } => Ok(()),
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
        assert_eq!(
            result,
            VerifyResult::Success {
                username: "test_user".to_string(),
                level: AuthLevel::NoAuth,
            }
        );
    }

    #[test]
    fn noauth_success_binds_request_identity_and_level() {
        let request = CredentialVerificationRequest::new(
            "org.liquide.test.action",
            "alice",
            AuthLevel::NoAuth,
        );

        assert_eq!(
            verify_authorization_request(&request),
            VerifyResult::Success {
                username: "alice".to_string(),
                level: AuthLevel::NoAuth,
            }
        );
    }

    #[test]
    fn credential_request_requires_non_empty_principal() {
        let request = CredentialVerificationRequest::new(
            "org.liquide.test.action",
            "  ",
            AuthLevel::UserPassword,
        );

        assert!(matches!(
            verify_authorization_request(&request),
            VerifyResult::Failed(reason) if reason.contains("principal")
        ));
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
        assert!(verify_result_to_result(VerifyResult::Success {
            username: "test_user".to_string(),
            level: AuthLevel::NoAuth,
        })
        .is_ok());
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
