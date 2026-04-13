//! LDAP authentication backend.
//!
//! Authenticates users by shelling out to `ldapwhoami` (from `ldap-utils` /
//! `openldap-clients`).  The password is piped through stdin via the `-y
//! /dev/stdin` flag so it never appears in `/proc/*/cmdline`.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::provider::{AuthProvider, AuthResult, Credentials};
use crate::AuthError;

/// Escape a string for use in an LDAP Distinguished Name per RFC 4514.
fn ldap_escape_dn(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for (i, ch) in s.chars().enumerate() {
        match ch {
            ',' | '+' | '"' | '\\' | '<' | '>' | ';' => {
                out.push('\\');
                out.push(ch);
            }
            '#' if i == 0 => {
                out.push('\\');
                out.push(ch);
            }
            ' ' if i == 0 || i == s.len() - 1 => {
                out.push('\\');
                out.push(ch);
            }
            '=' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// LDAP-based authentication provider.
pub struct LdapProvider {
    /// LDAP server URI (e.g. `ldaps://ldap.example.com`).
    pub uri: String,
    /// Base DN for user searches.
    pub base_dn: String,
}

impl LdapProvider {
    /// Create a new LDAP provider.
    #[must_use]
    pub fn new(uri: &str, base_dn: &str) -> Self {
        Self {
            uri: uri.to_string(),
            base_dn: base_dn.to_string(),
        }
    }

    /// Build the bind DN from a username.
    ///
    /// - If the username already contains `=` it is treated as a full DN.
    /// - If it contains `@` it is treated as a UPN (Active Directory style).
    /// - Otherwise we construct `uid=<username>,ou=People,<base_dn>`.
    fn bind_dn(&self, username: &str) -> String {
        if username.contains('=') {
            // Already a full DN
            username.to_string()
        } else if username.contains('@') {
            // UPN format (Active Directory)
            username.to_string()
        } else {
            format!("uid={},ou=People,{}", ldap_escape_dn(username), self.base_dn)
        }
    }

    /// Perform an LDAP simple-bind test using the `ldapwhoami` command.
    ///
    /// The password is written to the child process's stdin and read back via
    /// `-y /dev/stdin` so that it never appears in the process argument list.
    fn ldap_bind(&self, bind_dn: &str, password: &str) -> std::result::Result<bool, String> {
        let mut child = Command::new("ldapwhoami")
            .arg("-H")
            .arg(&self.uri)
            .arg("-D")
            .arg(bind_dn)
            .arg("-x") // simple authentication
            .arg("-y")
            .arg("/dev/stdin")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn ldapwhoami: {e}"))?;

        // Write password to stdin and close the pipe.
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(password.as_bytes());
            // stdin drops here, closing the pipe
        }

        let status = child
            .wait()
            .map_err(|e| format!("failed to wait on ldapwhoami: {e}"))?;

        Ok(status.success())
    }

    /// Try to retrieve the `cn` (Common Name) attribute for use as a display
    /// name.  Falls back to `None` on any error.
    fn ldap_get_cn(&self, bind_dn: &str, password: &str) -> Option<String> {
        let mut child = Command::new("ldapsearch")
            .arg("-H")
            .arg(&self.uri)
            .arg("-D")
            .arg(bind_dn)
            .arg("-x")
            .arg("-y")
            .arg("/dev/stdin")
            .arg("-b")
            .arg(bind_dn)
            .arg("-s")
            .arg("base")
            .arg("cn")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(password.as_bytes());
        }

        let output = child.wait_with_output().ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some(cn) = line.strip_prefix("cn: ") {
                return Some(cn.trim().to_string());
            }
        }
        None
    }
}

impl AuthProvider for LdapProvider {
    fn name(&self) -> &str {
        "ldap"
    }

    async fn authenticate(&self, credentials: &Credentials) -> crate::Result<AuthResult> {
        let (username, password) = match credentials {
            Credentials::Password { username, password } => (username.as_str(), password.as_str()),
            _ => {
                return Ok(AuthResult::Failure {
                    reason: "LDAP only supports password credentials".into(),
                });
            }
        };

        // Input validation.
        if username.is_empty() || username.len() > 256 {
            return Ok(AuthResult::Failure {
                reason: "invalid username".into(),
            });
        }
        if username.contains('\0') || username.contains('\n')
            || username.contains(',') || username.contains('+')
            || username.contains('"') || username.contains('<')
            || username.contains('>') || username.contains(';')
        {
            return Ok(AuthResult::Failure {
                reason: "invalid username characters".into(),
            });
        }

        let bind_dn = self.bind_dn(username);

        match self.ldap_bind(&bind_dn, password) {
            Ok(true) => {
                let display_name = self
                    .ldap_get_cn(&bind_dn, password)
                    .unwrap_or_else(|| username.to_string());
                Ok(AuthResult::Success {
                    user_id: username.to_string(),
                    display_name,
                })
            }
            Ok(false) => Ok(AuthResult::Failure {
                reason: "invalid LDAP credentials".into(),
            }),
            Err(e) => Err(AuthError::BackendUnavailable(format!("LDAP error: {e}"))),
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
    fn provider_name() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        assert_eq!(p.name(), "ldap");
    }

    #[test]
    fn supports_password_only() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        assert!(p.supports(&Credentials::Password {
            username: "test".into(),
            password: "pass".into(),
        }));
        assert!(!p.supports(&Credentials::OidcToken {
            token: "tok".into(),
        }));
        assert!(!p.supports(&Credentials::Certificate {
            der: vec![0x30],
        }));
    }

    #[test]
    fn bind_dn_plain_username() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        assert_eq!(
            p.bind_dn("alice"),
            "uid=alice,ou=People,dc=example,dc=com"
        );
    }

    #[test]
    fn bind_dn_full_dn_passthrough() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        let full = "cn=admin,dc=example,dc=com";
        assert_eq!(p.bind_dn(full), full);
    }

    #[test]
    fn bind_dn_upn_passthrough() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        assert_eq!(p.bind_dn("alice@example.com"), "alice@example.com");
    }

    #[tokio::test]
    async fn rejects_non_password_credentials() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        let result = p
            .authenticate(&Credentials::OidcToken {
                token: "tok".into(),
            })
            .await
            .unwrap();
        assert!(matches!(result, AuthResult::Failure { .. }));
    }

    #[tokio::test]
    async fn rejects_empty_username() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        let result = p
            .authenticate(&Credentials::Password {
                username: "".into(),
                password: "pass".into(),
            })
            .await
            .unwrap();
        match result {
            AuthResult::Failure { reason } => assert_eq!(reason, "invalid username"),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_username_with_null() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        let result = p
            .authenticate(&Credentials::Password {
                username: "alice\0bob".into(),
                password: "pass".into(),
            })
            .await
            .unwrap();
        match result {
            AuthResult::Failure { reason } => assert_eq!(reason, "invalid username characters"),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_username_with_newline() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        let result = p
            .authenticate(&Credentials::Password {
                username: "alice\nbob".into(),
                password: "pass".into(),
            })
            .await
            .unwrap();
        match result {
            AuthResult::Failure { reason } => assert_eq!(reason, "invalid username characters"),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_overlong_username() {
        let p = LdapProvider::new("ldaps://ldap.example.com", "dc=example,dc=com");
        let long_name = "a".repeat(257);
        let result = p
            .authenticate(&Credentials::Password {
                username: long_name,
                password: "pass".into(),
            })
            .await
            .unwrap();
        match result {
            AuthResult::Failure { reason } => assert_eq!(reason, "invalid username"),
            other => panic!("expected Failure, got {other:?}"),
        }
    }
}
