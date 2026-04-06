//! OpenID Connect (OIDC) authentication backend.

use crate::provider::{AuthProvider, AuthResult, Credentials};

/// OIDC provider configuration.
pub struct OidcProvider {
    /// The OIDC issuer URL.
    pub issuer: String,
    /// Client ID registered with the identity provider.
    pub client_id: String,
}

impl OidcProvider {
    /// Create a new OIDC provider.
    #[must_use]
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
        }
    }
}

impl AuthProvider for OidcProvider {
    fn name(&self) -> &str {
        "oidc"
    }

    async fn authenticate(&self, credentials: &Credentials) -> crate::Result<AuthResult> {
        let token = match credentials {
            Credentials::OidcToken { token } => token.as_str(),
            _ => {
                return Ok(AuthResult::Failure {
                    reason: "OIDC only supports token credentials".into(),
                })
            }
        };

        // Parse JWT (header.payload.signature)
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Ok(AuthResult::Failure {
                reason: "invalid JWT format".into(),
            });
        }

        // Decode payload (base64url -> JSON)
        let payload_json = match base64url_decode(parts[1]) {
            Some(bytes) => match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => {
                    return Ok(AuthResult::Failure {
                        reason: "invalid JWT payload encoding".into(),
                    })
                }
            },
            None => {
                return Ok(AuthResult::Failure {
                    reason: "invalid JWT base64url".into(),
                })
            }
        };

        // Parse claims (minimal JSON parsing without serde_json dependency)
        let claims = parse_jwt_claims(&payload_json);

        // Validate issuer
        if let Some(iss) = claims.get("iss") {
            if iss != &self.issuer {
                return Ok(AuthResult::Failure {
                    reason: format!("issuer mismatch: expected {}, got {iss}", self.issuer),
                });
            }
        } else {
            return Ok(AuthResult::Failure {
                reason: "missing issuer claim".into(),
            });
        }

        // Validate audience (client_id)
        if let Some(aud) = claims.get("aud") {
            if aud != &self.client_id {
                return Ok(AuthResult::Failure {
                    reason: "audience mismatch".into(),
                });
            }
        }

        // Check expiration
        if let Some(exp) = claims.get("exp") {
            if let Ok(exp_ts) = exp.parse::<u64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now > exp_ts {
                    return Ok(AuthResult::Failure {
                        reason: "token expired".into(),
                    });
                }
            }
        }

        // Extract user info
        let user_id = claims
            .get("sub")
            .cloned()
            .unwrap_or_else(|| "unknown".into());
        let display_name = claims
            .get("name")
            .or_else(|| claims.get("preferred_username"))
            .cloned()
            .unwrap_or_else(|| user_id.clone());

        // NOTE: In production, we should also verify the JWT signature against
        // the issuer's JWKS endpoint. For now, we validate structure + claims only.
        // Full signature verification requires fetching keys from {issuer}/.well-known/jwks.json
        tracing::warn!("OIDC signature verification not yet implemented — validating claims only");

        Ok(AuthResult::Success {
            user_id,
            display_name,
        })
    }

    fn supports(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::OidcToken { .. })
    }
}

/// Decode base64url (no padding) to bytes.
fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut lookup = [255u8; 256];
    for (i, &b) in ALPHABET.iter().enumerate() {
        lookup[b as usize] = i as u8;
    }

    // Pad to multiple of 4
    let padded = match input.len() % 4 {
        2 => format!("{input}=="),
        3 => format!("{input}="),
        _ => input.to_string(),
    };

    let mut result = Vec::with_capacity(padded.len() * 3 / 4);
    for chunk in padded.as_bytes().chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let mut block = 0u32;
        let mut pad_count = 0;
        for (i, &b) in chunk.iter().enumerate() {
            if b == b'=' {
                pad_count += 1;
            } else {
                let val = lookup[b as usize];
                if val == 255 {
                    return None;
                } // Invalid character
                block |= (val as u32) << (18 - 6 * i);
            }
        }
        result.push((block >> 16) as u8);
        if pad_count < 2 {
            result.push((block >> 8) as u8);
        }
        if pad_count < 1 {
            result.push(block as u8);
        }
    }
    Some(result)
}

/// Minimal JSON claim extraction (no serde dependency).
fn parse_jwt_claims(json: &str) -> std::collections::HashMap<String, String> {
    let mut claims = std::collections::HashMap::new();
    let trimmed = json.trim().trim_start_matches('{').trim_end_matches('}');
    // Simple key-value extraction for string and numeric values
    for pair in split_json_pairs(trimmed) {
        let pair = pair.trim();
        if let Some(colon_pos) = pair.find(':') {
            let key = pair[..colon_pos].trim().trim_matches('"');
            let val = pair[colon_pos + 1..].trim().trim_matches('"');
            claims.insert(key.to_string(), val.to_string());
        }
    }
    claims
}

/// Split JSON object into key:value pairs, respecting nested objects/arrays.
fn split_json_pairs(s: &str) -> Vec<&str> {
    let mut pairs = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut in_string = false;
    let mut prev_char = ' ';

    for (i, ch) in s.char_indices() {
        match ch {
            '"' if prev_char != '\\' => in_string = !in_string,
            '{' | '[' if !in_string => depth += 1,
            '}' | ']' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                pairs.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        prev_char = ch;
    }
    if start < s.len() {
        pairs.push(&s[start..]);
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64url_decode_basic() {
        // "hello" in base64url is "aGVsbG8"
        let decoded = base64url_decode("aGVsbG8").unwrap();
        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn test_base64url_decode_empty() {
        let decoded = base64url_decode("").unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_base64url_decode_invalid_char() {
        assert!(base64url_decode("!!!").is_none());
    }

    #[test]
    fn test_base64url_decode_json_payload() {
        // {"sub":"user1","iss":"https://example.com"}
        let encoded = "eyJzdWIiOiJ1c2VyMSIsImlzcyI6Imh0dHBzOi8vZXhhbXBsZS5jb20ifQ";
        let decoded = base64url_decode(encoded).unwrap();
        let s = String::from_utf8(decoded).unwrap();
        assert!(s.contains("\"sub\":\"user1\""));
        assert!(s.contains("\"iss\":\"https://example.com\""));
    }

    #[test]
    fn test_parse_jwt_claims() {
        let json = r#"{"sub":"user123","iss":"https://idp.example.com","name":"Alice","exp":9999999999}"#;
        let claims = parse_jwt_claims(json);
        assert_eq!(claims.get("sub").unwrap(), "user123");
        assert_eq!(claims.get("iss").unwrap(), "https://idp.example.com");
        assert_eq!(claims.get("name").unwrap(), "Alice");
        assert_eq!(claims.get("exp").unwrap(), "9999999999");
    }

    #[test]
    fn test_parse_jwt_claims_empty() {
        let claims = parse_jwt_claims("{}");
        assert!(claims.is_empty());
    }

    #[test]
    fn test_split_json_pairs_nested() {
        let s = r#""a":"1","b":{"nested":"val"},"c":"3""#;
        let pairs = split_json_pairs(s);
        assert_eq!(pairs.len(), 3);
    }

    #[tokio::test]
    async fn test_oidc_invalid_credential_type() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        let creds = Credentials::Password {
            username: "user".into(),
            password: "pass".into(),
        };
        let result = provider.authenticate(&creds).await.unwrap();
        assert!(matches!(result, AuthResult::Failure { .. }));
    }

    #[tokio::test]
    async fn test_oidc_malformed_jwt() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        let creds = Credentials::OidcToken {
            token: "not-a-jwt".into(),
        };
        let result = provider.authenticate(&creds).await.unwrap();
        match result {
            AuthResult::Failure { reason } => assert!(reason.contains("invalid JWT format")),
            _ => panic!("expected failure"),
        }
    }

    #[tokio::test]
    async fn test_oidc_missing_issuer() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        // Payload: {"sub":"user1"} (no iss claim)
        let payload = "eyJzdWIiOiJ1c2VyMSJ9";
        let token = format!("eyJhbGciOiJSUzI1NiJ9.{payload}.fakesig");
        let creds = Credentials::OidcToken { token };
        let result = provider.authenticate(&creds).await.unwrap();
        match result {
            AuthResult::Failure { reason } => assert!(reason.contains("missing issuer")),
            _ => panic!("expected failure"),
        }
    }

    #[tokio::test]
    async fn test_oidc_issuer_mismatch() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        // Payload: {"sub":"user1","iss":"https://wrong.com"}
        let payload = "eyJzdWIiOiJ1c2VyMSIsImlzcyI6Imh0dHBzOi8vd3JvbmcuY29tIn0";
        let token = format!("eyJhbGciOiJSUzI1NiJ9.{payload}.fakesig");
        let creds = Credentials::OidcToken { token };
        let result = provider.authenticate(&creds).await.unwrap();
        match result {
            AuthResult::Failure { reason } => assert!(reason.contains("issuer mismatch")),
            _ => panic!("expected failure"),
        }
    }

    #[tokio::test]
    async fn test_oidc_expired_token() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        // Payload: {"sub":"user1","iss":"https://idp.example.com","exp":1000000000}
        let payload = "eyJzdWIiOiJ1c2VyMSIsImlzcyI6Imh0dHBzOi8vaWRwLmV4YW1wbGUuY29tIiwiZXhwIjoxMDAwMDAwMDAwfQ";
        let token = format!("eyJhbGciOiJSUzI1NiJ9.{payload}.fakesig");
        let creds = Credentials::OidcToken { token };
        let result = provider.authenticate(&creds).await.unwrap();
        match result {
            AuthResult::Failure { reason } => assert!(reason.contains("expired")),
            _ => panic!("expected failure"),
        }
    }

    #[tokio::test]
    async fn test_oidc_success() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        // Payload: {"sub":"user1","iss":"https://idp.example.com","aud":"my-client","name":"Alice","exp":9999999999}
        let payload = "eyJzdWIiOiJ1c2VyMSIsImlzcyI6Imh0dHBzOi8vaWRwLmV4YW1wbGUuY29tIiwiYXVkIjoibXktY2xpZW50IiwibmFtZSI6IkFsaWNlIiwiZXhwIjo5OTk5OTk5OTk5fQ";
        let token = format!("eyJhbGciOiJSUzI1NiJ9.{payload}.fakesig");
        let creds = Credentials::OidcToken { token };
        let result = provider.authenticate(&creds).await.unwrap();
        match result {
            AuthResult::Success {
                user_id,
                display_name,
            } => {
                assert_eq!(user_id, "user1");
                assert_eq!(display_name, "Alice");
            }
            _ => panic!("expected success"),
        }
    }

    #[tokio::test]
    async fn test_oidc_audience_mismatch() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        // Payload: {"sub":"user1","iss":"https://idp.example.com","aud":"wrong-client","exp":9999999999}
        let payload = "eyJzdWIiOiJ1c2VyMSIsImlzcyI6Imh0dHBzOi8vaWRwLmV4YW1wbGUuY29tIiwiYXVkIjoid3JvbmctY2xpZW50IiwiZXhwIjo5OTk5OTk5OTk5fQ";
        let token = format!("eyJhbGciOiJSUzI1NiJ9.{payload}.fakesig");
        let creds = Credentials::OidcToken { token };
        let result = provider.authenticate(&creds).await.unwrap();
        match result {
            AuthResult::Failure { reason } => assert!(reason.contains("audience mismatch")),
            _ => panic!("expected failure"),
        }
    }

    #[test]
    fn test_supports() {
        let provider = OidcProvider::new("https://idp.example.com", "my-client");
        assert!(provider.supports(&Credentials::OidcToken {
            token: "x".into()
        }));
        assert!(!provider.supports(&Credentials::Password {
            username: "u".into(),
            password: "p".into(),
        }));
    }
}
