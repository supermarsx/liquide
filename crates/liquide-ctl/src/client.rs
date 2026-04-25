use anyhow::{Context, Result};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

/// Client for communicating with the LiquiDE server daemon.
///
/// Connects via local Unix socket or remote HTTPS API.
pub struct Client {
    server: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

/// Generic API response wrapper.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn build_url(server: &str, path: &str) -> String {
    format!(
        "{}/{}",
        server.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

impl Client {
    /// Create a new client connected to the given server address.
    pub fn new(server: String, api_key: Option<String>) -> Self {
        let http = reqwest::Client::new();
        Self {
            server,
            api_key,
            http,
        }
    }

    /// The server address this client is configured to connect to.
    pub fn server(&self) -> &str {
        &self.server
    }

    /// Whether this client uses a local Unix socket connection.
    #[allow(dead_code)]
    pub fn is_local(&self) -> bool {
        self.server.starts_with("unix://")
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(key) = &self.api_key {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key}")) {
                headers.insert(AUTHORIZATION, val);
            }
        }
        headers
    }

    /// Send a GET request to the server API.
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = build_url(&self.server, path);
        let resp = self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GET {url} failed ({status}): {body}");
        }
        resp.json::<T>()
            .await
            .with_context(|| format!("deserializing response from GET {url}"))
    }

    /// Send a POST request to the server API.
    pub async fn post<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = build_url(&self.server, path);
        let resp = self
            .http
            .post(&url)
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("POST {url} failed ({status}): {body}");
        }
        resp.json::<T>()
            .await
            .with_context(|| format!("deserializing response from POST {url}"))
    }

    /// Send a DELETE request to the server API.
    pub async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = build_url(&self.server, path);
        let resp = self
            .http
            .delete(&url)
            .headers(self.auth_headers())
            .send()
            .await
            .with_context(|| format!("DELETE {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("DELETE {url} failed ({status}): {body}");
        }
        resp.json::<T>()
            .await
            .with_context(|| format!("deserializing response from DELETE {url}"))
    }

    /// Send a PUT request to the server API.
    pub async fn put<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = build_url(&self.server, path);
        let resp = self
            .http
            .put(&url)
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await
            .with_context(|| format!("PUT {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("PUT {url} failed ({status}): {body}");
        }
        resp.json::<T>()
            .await
            .with_context(|| format!("deserializing response from PUT {url}"))
    }

    /// Check connectivity to the server.
    pub async fn ping(&self) -> Result<()> {
        let url = build_url(&self.server, "/health");
        let resp = self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await
            .with_context(|| format!("ping {url}"))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ping {url} failed ({status}): {body}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_no_double_slash() {
        assert_eq!(
            build_url("https://server.example.com/", "/api/v1/status"),
            "https://server.example.com/api/v1/status"
        );
    }

    #[test]
    fn build_url_adds_slash() {
        assert_eq!(
            build_url("https://server.example.com", "api/v1/status"),
            "https://server.example.com/api/v1/status"
        );
    }

    #[test]
    fn build_url_both_clean() {
        assert_eq!(build_url("https://s", "health"), "https://s/health");
    }

    #[test]
    fn client_new_without_api_key() {
        let c = Client::new("https://localhost:8443".into(), None);
        assert_eq!(c.server(), "https://localhost:8443");
        assert!(c.auth_headers().is_empty());
    }

    #[test]
    fn client_new_with_api_key() {
        let c = Client::new("https://localhost:8443".into(), Some("tok123".into()));
        let h = c.auth_headers();
        assert_eq!(h.get(AUTHORIZATION).unwrap(), "Bearer tok123");
    }

    #[test]
    fn is_local_unix() {
        let c = Client::new("unix:///run/liquide.sock".into(), None);
        assert!(c.is_local());
    }

    #[test]
    fn is_local_https() {
        let c = Client::new("https://remote:8443".into(), None);
        assert!(!c.is_local());
    }

    #[test]
    fn api_response_serialization() {
        let resp = ApiResponse {
            success: true,
            data: Some("hello".to_string()),
            error: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["data"], "hello");
        assert!(json.get("error").is_none());
    }

    #[test]
    fn api_response_deserialization_with_error() {
        let json = r#"{"success":false,"error":"not found"}"#;
        let resp: ApiResponse<String> = serde_json::from_str(json).unwrap();
        assert!(!resp.success);
        assert_eq!(resp.error.as_deref(), Some("not found"));
        assert!(resp.data.is_none());
    }
}
