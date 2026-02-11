//! REST API endpoint definitions and routing.

use std::fmt;

use serde::{Deserialize, Serialize};

/// HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Put => write!(f, "PUT"),
            Self::Delete => write!(f, "DELETE"),
        }
    }
}

/// A registered API endpoint.
#[derive(Debug, Clone)]
pub struct ApiEndpoint {
    /// HTTP method.
    pub method: HttpMethod,
    /// Path pattern (e.g. `/api/v1/sessions/{id}`).
    pub path: String,
    /// Required minimum role.
    pub min_role: crate::config::AdminRole,
    /// Human-readable description.
    pub description: String,
}

impl fmt::Display for ApiEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.method, self.path)
    }
}

/// API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    /// Whether the request succeeded.
    pub success: bool,
    /// Response payload.
    pub data: Option<T>,
    /// Error message when `success` is false.
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a success response.
    #[must_use]
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn err(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Pagination parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Page number (1-based).
    pub page: u32,
    /// Items per page.
    pub per_page: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 25,
        }
    }
}

impl Pagination {
    /// Compute the offset for database/list queries.
    #[must_use]
    pub fn offset(&self) -> usize {
        ((self.page.saturating_sub(1)) * self.per_page) as usize
    }

    /// Compute the limit.
    #[must_use]
    pub fn limit(&self) -> usize {
        self.per_page as usize
    }
}

/// Paginated response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T: Serialize> {
    /// Items on this page.
    pub items: Vec<T>,
    /// Total item count across all pages.
    pub total: usize,
    /// Current page number.
    pub page: u32,
    /// Items per page.
    pub per_page: u32,
    /// Total number of pages.
    pub total_pages: u32,
}

impl<T: Serialize> PaginatedResponse<T> {
    /// Build a paginated response from a full list and pagination params.
    #[must_use]
    pub fn from_vec(all: Vec<T>, pagination: &Pagination) -> Self {
        let total = all.len();
        let total_pages = if pagination.per_page == 0 {
            0
        } else {
            ((total as u32) + pagination.per_page - 1) / pagination.per_page
        };
        let start = pagination.offset().min(total);
        let end = (start + pagination.limit()).min(total);
        let items = all.into_iter().skip(start).take(end - start).collect();
        Self {
            items,
            total,
            page: pagination.page,
            per_page: pagination.per_page,
            total_pages,
        }
    }
}

/// Build the default list of API endpoints.
#[must_use]
pub fn default_endpoints() -> Vec<ApiEndpoint> {
    use crate::config::AdminRole;
    vec![
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/dashboard".into(), min_role: AdminRole::Viewer, description: "Aggregate dashboard data".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/servers".into(), min_role: AdminRole::Viewer, description: "List managed servers".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/servers/{name}".into(), min_role: AdminRole::Viewer, description: "Server details".into() },
        ApiEndpoint { method: HttpMethod::Put, path: "/api/v1/servers/{name}/config".into(), min_role: AdminRole::Admin, description: "Update server config".into() },
        ApiEndpoint { method: HttpMethod::Post, path: "/api/v1/servers/{name}/restart".into(), min_role: AdminRole::Admin, description: "Restart server".into() },
        ApiEndpoint { method: HttpMethod::Post, path: "/api/v1/servers/{name}/drain".into(), min_role: AdminRole::Admin, description: "Drain server".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/sessions".into(), min_role: AdminRole::Viewer, description: "List sessions".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/sessions/{id}".into(), min_role: AdminRole::Viewer, description: "Session details".into() },
        ApiEndpoint { method: HttpMethod::Delete, path: "/api/v1/sessions/{id}".into(), min_role: AdminRole::Operator, description: "Disconnect session".into() },
        ApiEndpoint { method: HttpMethod::Post, path: "/api/v1/sessions/{id}/lock".into(), min_role: AdminRole::Operator, description: "Lock session".into() },
        ApiEndpoint { method: HttpMethod::Post, path: "/api/v1/sessions/{id}/unlock".into(), min_role: AdminRole::Operator, description: "Unlock session".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/users".into(), min_role: AdminRole::Viewer, description: "List users".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/users/{name}".into(), min_role: AdminRole::Viewer, description: "User details".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/policies".into(), min_role: AdminRole::Viewer, description: "List policies".into() },
        ApiEndpoint { method: HttpMethod::Put, path: "/api/v1/policies".into(), min_role: AdminRole::Admin, description: "Update policies".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/metrics".into(), min_role: AdminRole::Viewer, description: "Metrics snapshot".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/gateways".into(), min_role: AdminRole::Viewer, description: "List gateways".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/gateways/{name}".into(), min_role: AdminRole::Viewer, description: "Gateway details".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/audit".into(), min_role: AdminRole::Viewer, description: "Audit log".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/crashes".into(), min_role: AdminRole::Viewer, description: "Crash reports".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/crashes/{id}".into(), min_role: AdminRole::Viewer, description: "Crash report details".into() },
        ApiEndpoint { method: HttpMethod::Get, path: "/api/v1/plugins".into(), min_role: AdminRole::Viewer, description: "List plugins".into() },
        ApiEndpoint { method: HttpMethod::Post, path: "/api/v1/plugins/install".into(), min_role: AdminRole::Admin, description: "Install plugin".into() },
    ]
}
