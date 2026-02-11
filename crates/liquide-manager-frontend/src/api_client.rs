//! API client types — requests, errors, and a mock client for testing.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// HTTP method for API requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl fmt::Display for ApiMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Put => write!(f, "PUT"),
            Self::Delete => write!(f, "DELETE"),
        }
    }
}

/// An API request ready to be sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    /// HTTP method.
    pub method: ApiMethod,
    /// Request path (relative to API base URL).
    pub path: String,
    /// Query parameters.
    pub query_params: HashMap<String, String>,
    /// Optional JSON body.
    pub body: Option<String>,
}

impl ApiRequest {
    /// Create a new request.
    #[must_use]
    pub fn new(method: ApiMethod, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query_params: HashMap::new(),
            body: None,
        }
    }
}

impl fmt::Display for ApiRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.method, self.path)
    }
}

/// Errors that can occur when communicating with the manager API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiError {
    /// A network-level error (DNS, timeout, connection refused).
    Network(String),
    /// The request was rejected because the token is missing or expired.
    Unauthorized,
    /// The authenticated user lacks permission for the requested action.
    Forbidden,
    /// The requested resource does not exist.
    NotFound,
    /// Too many requests — the caller has been rate-limited.
    RateLimited,
    /// The server returned a 5xx error.
    ServerError(String),
    /// The response body could not be parsed.
    ParseError(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::Unauthorized => write!(f, "unauthorized"),
            Self::Forbidden => write!(f, "forbidden"),
            Self::NotFound => write!(f, "not found"),
            Self::RateLimited => write!(f, "rate limited"),
            Self::ServerError(msg) => write!(f, "server error: {msg}"),
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// A builder for constructing [`ApiRequest`] instances.
#[derive(Debug, Clone)]
pub struct RequestBuilder {
    method: ApiMethod,
    path: String,
    query_params: HashMap<String, String>,
    body: Option<String>,
}

impl RequestBuilder {
    /// Start building a GET request.
    #[must_use]
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: ApiMethod::Get,
            path: path.into(),
            query_params: HashMap::new(),
            body: None,
        }
    }

    /// Start building a POST request.
    #[must_use]
    pub fn post(path: impl Into<String>) -> Self {
        Self {
            method: ApiMethod::Post,
            path: path.into(),
            query_params: HashMap::new(),
            body: None,
        }
    }

    /// Start building a PUT request.
    #[must_use]
    pub fn put(path: impl Into<String>) -> Self {
        Self {
            method: ApiMethod::Put,
            path: path.into(),
            query_params: HashMap::new(),
            body: None,
        }
    }

    /// Start building a DELETE request.
    #[must_use]
    pub fn delete(path: impl Into<String>) -> Self {
        Self {
            method: ApiMethod::Delete,
            path: path.into(),
            query_params: HashMap::new(),
            body: None,
        }
    }

    /// Set the request path.
    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Add a query parameter.
    #[must_use]
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.insert(key.into(), value.into());
        self
    }

    /// Set the JSON body.
    #[must_use]
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Build the final [`ApiRequest`].
    #[must_use]
    pub fn build(self) -> ApiRequest {
        ApiRequest {
            method: self.method,
            path: self.path,
            query_params: self.query_params,
            body: self.body,
        }
    }
}

/// A mock API client for testing — queues canned responses.
#[derive(Debug, Clone)]
pub struct MockApiClient {
    /// Pre-loaded responses (FIFO).
    responses: Vec<Result<String, ApiError>>,
    /// Recorded requests (in order).
    recorded: Vec<ApiRequest>,
}

impl MockApiClient {
    /// Create a new empty mock client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
            recorded: Vec::new(),
        }
    }

    /// Enqueue a successful JSON response.
    pub fn enqueue_ok(&mut self, json: impl Into<String>) {
        self.responses.push(Ok(json.into()));
    }

    /// Enqueue an error response.
    pub fn enqueue_err(&mut self, error: ApiError) {
        self.responses.push(Err(error));
    }

    /// Send a request and return the next queued response.
    pub fn send(&mut self, request: ApiRequest) -> Result<String, ApiError> {
        self.recorded.push(request);
        if self.responses.is_empty() {
            Err(ApiError::Network("no queued response".to_string()))
        } else {
            self.responses.remove(0)
        }
    }

    /// Return recorded requests so far.
    #[must_use]
    pub fn recorded(&self) -> &[ApiRequest] {
        &self.recorded
    }

    /// Number of remaining queued responses.
    #[must_use]
    pub fn pending_responses(&self) -> usize {
        self.responses.len()
    }
}

impl Default for MockApiClient {
    fn default() -> Self {
        Self::new()
    }
}
