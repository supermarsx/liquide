use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Client for communicating with the LiquiDE server daemon.
///
/// Connects via local Unix socket or remote HTTPS API.
pub struct Client {
    server: String,
    api_key: Option<String>,
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

impl Client {
    /// Create a new client connected to the given server address.
    pub fn new(server: String, api_key: Option<String>) -> Self {
        Self { server, api_key }
    }

    /// The server address this client is configured to connect to.
    pub fn server(&self) -> &str {
        &self.server
    }

    /// Whether this client uses a local Unix socket connection.
    pub fn is_local(&self) -> bool {
        self.server.starts_with("unix://")
    }

    /// Send a GET request to the server API.
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        // TODO: Implement actual HTTP/socket communication
        let _ = path;
        let _ = &self.api_key;
        anyhow::bail!(
            "Not yet implemented: GET {} on {}",
            path,
            self.server
        )
    }

    /// Send a POST request to the server API.
    pub async fn post<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        // TODO: Implement actual HTTP/socket communication
        let _ = (path, body, &self.api_key);
        anyhow::bail!(
            "Not yet implemented: POST {} on {}",
            path,
            self.server
        )
    }

    /// Send a DELETE request to the server API.
    pub async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let _ = (path, &self.api_key);
        anyhow::bail!(
            "Not yet implemented: DELETE {} on {}",
            path,
            self.server
        )
    }

    /// Send a PUT request to the server API.
    pub async fn put<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let _ = (path, body, &self.api_key);
        anyhow::bail!(
            "Not yet implemented: PUT {} on {}",
            path,
            self.server
        )
    }

    /// Check connectivity to the server.
    pub async fn ping(&self) -> Result<()> {
        // TODO: Implement actual health check
        anyhow::bail!("Not yet implemented: ping {}", self.server)
    }
}
