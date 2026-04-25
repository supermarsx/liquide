//! PAM (Pluggable Authentication Modules) backend.
//!
//! Uses the `unix_chkpwd` helper binary (present on most Linux distros)
//! for password verification. This avoids a direct `libpam` FFI dependency
//! while still leveraging the system PAM stack.

#[cfg(unix)]
use crate::AuthError;
use crate::provider::{AuthProvider, AuthResult, Credentials};

/// PAM-based authentication provider for local Unix accounts.
pub struct PamProvider {
    /// The PAM service name (e.g. `"liquide"`).
    pub service: String,
}

impl PamProvider {
    /// Create a new PAM provider using the given service name.
    #[must_use]
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }

    /// Locate the `unix_chkpwd` binary on the filesystem.
    #[cfg(unix)]
    fn find_chkpwd() -> Option<&'static str> {
        const PATHS: &[&str] = &[
            "/usr/sbin/unix_chkpwd",
            "/sbin/unix_chkpwd",
            "/usr/bin/unix_chkpwd",
        ];
        for p in PATHS {
            if std::path::Path::new(p).exists() {
                return Some(p);
            }
        }
        None
    }

    /// Verify a password using `unix_chkpwd`.
    ///
    /// `unix_chkpwd` is a setuid helper that reads the password from stdin
    /// and exits 0 on success. It is the standard mechanism used by PAM's
    /// `pam_unix` module.
    #[cfg(unix)]
    fn verify_password_unix(username: &str, password: &str) -> std::result::Result<bool, String> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let chkpwd = Self::find_chkpwd()
            .ok_or_else(|| "unix_chkpwd not found on this system".to_string())?;

        let mut child = Command::new(chkpwd)
            .arg(username)
            .arg("nullok")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn unix_chkpwd: {e}"))?;

        if let Some(ref mut stdin) = child.stdin {
            // unix_chkpwd expects the password followed by a NUL byte.
            let _ = stdin.write_all(password.as_bytes());
            let _ = stdin.write_all(b"\0");
        }
        // Drop stdin so the child sees EOF.
        drop(child.stdin.take());

        let status = child
            .wait()
            .map_err(|e| format!("unix_chkpwd wait failed: {e}"))?;

        Ok(status.success())
    }

    /// Look up the GECOS display name for a user.
    ///
    /// On Unix, reads the GECOS field from `/etc/passwd`.
    /// On other platforms, returns the username as-is.
    #[cfg_attr(not(unix), allow(dead_code))]
    fn get_display_name(username: &str) -> String {
        #[cfg(unix)]
        {
            if let Ok(content) = std::fs::read_to_string("/etc/passwd") {
                for line in content.lines() {
                    let fields: Vec<&str> = line.split(':').collect();
                    if fields.len() >= 5 && fields[0] == username {
                        let gecos = fields[4];
                        let display = gecos.split(',').next().unwrap_or(username);
                        if !display.is_empty() {
                            return display.to_string();
                        }
                    }
                }
            }
        }
        username.to_string()
    }

    /// Validate that a username is safe to pass to `unix_chkpwd`.
    fn validate_username(username: &str) -> std::result::Result<(), &'static str> {
        if username.is_empty() {
            return Err("username is empty");
        }
        if username.len() > 256 {
            return Err("username too long");
        }
        if username.contains('\0') || username.contains('\n') || username.contains('\r') {
            return Err("username contains invalid characters");
        }
        Ok(())
    }
}

impl AuthProvider for PamProvider {
    fn name(&self) -> &str {
        "pam"
    }

    async fn authenticate(&self, credentials: &Credentials) -> crate::Result<AuthResult> {
        let (username, password) = match credentials {
            Credentials::Password { username, password } => (username.as_str(), password.as_str()),
            _ => {
                return Ok(AuthResult::Failure {
                    reason: "PAM provider only supports password credentials".into(),
                });
            }
        };

        if let Err(msg) = Self::validate_username(username) {
            return Ok(AuthResult::Failure {
                reason: msg.to_string(),
            });
        }

        #[cfg(unix)]
        {
            match Self::verify_password_unix(username, password) {
                Ok(true) => Ok(AuthResult::Success {
                    user_id: username.to_string(),
                    display_name: Self::get_display_name(username),
                }),
                Ok(false) => Ok(AuthResult::Failure {
                    reason: "invalid credentials".into(),
                }),
                Err(e) => Err(AuthError::BackendUnavailable(format!(
                    "PAM backend error: {e}"
                ))),
            }
        }

        #[cfg(not(unix))]
        {
            let _ = password;
            Ok(AuthResult::Failure {
                reason: "PAM authentication is only supported on Unix systems".into(),
            })
        }
    }

    fn supports(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::Password { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_service_name() {
        let p = PamProvider::new("login");
        assert_eq!(p.service, "login");
    }

    #[test]
    fn name_returns_pam() {
        let p = PamProvider::new("liquide");
        assert_eq!(p.name(), "pam");
    }

    #[test]
    fn supports_password_only() {
        let p = PamProvider::new("liquide");
        assert!(p.supports(&Credentials::Password {
            username: "u".into(),
            password: "p".into(),
        }));
        assert!(!p.supports(&Credentials::OidcToken { token: "t".into() }));
        assert!(!p.supports(&Credentials::Certificate { der: vec![] }));
    }

    #[test]
    fn validate_username_rejects_empty() {
        assert!(PamProvider::validate_username("").is_err());
    }

    #[test]
    fn validate_username_rejects_null_bytes() {
        assert!(PamProvider::validate_username("user\0name").is_err());
    }

    #[test]
    fn validate_username_rejects_newlines() {
        assert!(PamProvider::validate_username("user\nname").is_err());
        assert!(PamProvider::validate_username("user\rname").is_err());
    }

    #[test]
    fn validate_username_rejects_too_long() {
        let long = "a".repeat(257);
        assert!(PamProvider::validate_username(&long).is_err());
    }

    #[test]
    fn validate_username_accepts_normal() {
        assert!(PamProvider::validate_username("alice").is_ok());
        assert!(PamProvider::validate_username("bob_smith").is_ok());
        assert!(PamProvider::validate_username("user-123").is_ok());
    }

    #[tokio::test]
    async fn authenticate_rejects_non_password() {
        let p = PamProvider::new("liquide");
        let creds = Credentials::OidcToken {
            token: "tok".into(),
        };
        let result = p.authenticate(&creds).await.unwrap();
        assert!(matches!(result, AuthResult::Failure { .. }));
    }

    #[tokio::test]
    async fn authenticate_rejects_empty_username() {
        let p = PamProvider::new("liquide");
        let creds = Credentials::Password {
            username: "".into(),
            password: "pw".into(),
        };
        let result = p.authenticate(&creds).await.unwrap();
        assert!(matches!(result, AuthResult::Failure { .. }));
    }

    #[tokio::test]
    async fn authenticate_rejects_null_in_username() {
        let p = PamProvider::new("liquide");
        let creds = Credentials::Password {
            username: "bad\0user".into(),
            password: "pw".into(),
        };
        let result = p.authenticate(&creds).await.unwrap();
        assert!(matches!(result, AuthResult::Failure { .. }));
    }

    #[test]
    fn get_display_name_fallback() {
        // When user is not in /etc/passwd, falls back to username
        let name = PamProvider::get_display_name("nonexistent_test_user_12345");
        assert_eq!(name, "nonexistent_test_user_12345");
    }
}
