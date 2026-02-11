//! Tests for API endpoint definitions and pagination.

use crate::api::{ApiResponse, HttpMethod, Pagination, PaginatedResponse, default_endpoints};
use crate::config::AdminRole;

// ===========================================================================
// HttpMethod
// ===========================================================================

#[test]
fn test_http_method_display() {
    assert_eq!(HttpMethod::Get.to_string(), "GET");
    assert_eq!(HttpMethod::Post.to_string(), "POST");
    assert_eq!(HttpMethod::Put.to_string(), "PUT");
    assert_eq!(HttpMethod::Delete.to_string(), "DELETE");
}

// ===========================================================================
// ApiEndpoint
// ===========================================================================

#[test]
fn test_endpoint_display() {
    let endpoint = &default_endpoints()[0];
    let display = endpoint.to_string();
    assert!(display.contains("GET"));
    assert!(display.contains("/api/v1/"));
}

#[test]
fn test_default_endpoints_count() {
    let endpoints = default_endpoints();
    assert_eq!(endpoints.len(), 23);
}

#[test]
fn test_endpoints_have_viewer_accessible_paths() {
    let endpoints = default_endpoints();
    let viewer_endpoints: Vec<_> = endpoints
        .iter()
        .filter(|e| e.min_role == AdminRole::Viewer)
        .collect();
    assert!(viewer_endpoints.len() > 10);
}

#[test]
fn test_admin_only_endpoints_exist() {
    let endpoints = default_endpoints();
    let admin_endpoints: Vec<_> = endpoints
        .iter()
        .filter(|e| e.min_role == AdminRole::Admin)
        .collect();
    assert!(admin_endpoints.len() >= 4);
}

// ===========================================================================
// ApiResponse
// ===========================================================================

#[test]
fn test_api_response_ok() {
    let resp = ApiResponse::ok(42u32);
    assert!(resp.success);
    assert_eq!(resp.data, Some(42));
    assert!(resp.error.is_none());
}

#[test]
fn test_api_response_err() {
    let resp = ApiResponse::<u32>::err("not found".into());
    assert!(!resp.success);
    assert!(resp.data.is_none());
    assert_eq!(resp.error.as_deref(), Some("not found"));
}

#[test]
fn test_api_response_serde_roundtrip() {
    let resp = ApiResponse::ok("hello".to_string());
    let json = serde_json::to_string(&resp).unwrap();
    let recovered: ApiResponse<String> = serde_json::from_str(&json).unwrap();
    assert!(recovered.success);
    assert_eq!(recovered.data.as_deref(), Some("hello"));
}

// ===========================================================================
// Pagination
// ===========================================================================

#[test]
fn test_pagination_defaults() {
    let p = Pagination::default();
    assert_eq!(p.page, 1);
    assert_eq!(p.per_page, 25);
}

#[test]
fn test_pagination_offset_page1() {
    let p = Pagination { page: 1, per_page: 10 };
    assert_eq!(p.offset(), 0);
    assert_eq!(p.limit(), 10);
}

#[test]
fn test_pagination_offset_page3() {
    let p = Pagination { page: 3, per_page: 10 };
    assert_eq!(p.offset(), 20);
}

#[test]
fn test_pagination_offset_page0_saturates() {
    let p = Pagination { page: 0, per_page: 10 };
    assert_eq!(p.offset(), 0);
}

// ===========================================================================
// PaginatedResponse
// ===========================================================================

#[test]
fn test_paginated_response_first_page() {
    let items: Vec<u32> = (1..=50).collect();
    let page = Pagination { page: 1, per_page: 10 };
    let resp = PaginatedResponse::from_vec(items, &page);
    assert_eq!(resp.items.len(), 10);
    assert_eq!(resp.items[0], 1);
    assert_eq!(resp.items[9], 10);
    assert_eq!(resp.total, 50);
    assert_eq!(resp.total_pages, 5);
}

#[test]
fn test_paginated_response_last_page() {
    let items: Vec<u32> = (1..=53).collect();
    let page = Pagination { page: 6, per_page: 10 };
    let resp = PaginatedResponse::from_vec(items, &page);
    assert_eq!(resp.items.len(), 3);
    assert_eq!(resp.items[0], 51);
    assert_eq!(resp.total_pages, 6);
}

#[test]
fn test_paginated_response_beyond_last_page() {
    let items: Vec<u32> = (1..=10).collect();
    let page = Pagination { page: 5, per_page: 10 };
    let resp = PaginatedResponse::from_vec(items, &page);
    assert!(resp.items.is_empty());
}

#[test]
fn test_paginated_response_empty() {
    let items: Vec<u32> = vec![];
    let page = Pagination::default();
    let resp = PaginatedResponse::from_vec(items, &page);
    assert!(resp.items.is_empty());
    assert_eq!(resp.total, 0);
    assert_eq!(resp.total_pages, 0);
}
