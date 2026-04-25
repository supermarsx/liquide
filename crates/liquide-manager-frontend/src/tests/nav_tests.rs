//! Tests for navigation state, sections, and breadcrumbs.

use crate::auth::AuthRole;
use crate::nav::{NavItem, NavSection, NavState};

// ===========================================================================
// NavSection properties
// ===========================================================================

#[test]
fn test_section_count() {
    assert_eq!(NavSection::ALL.len(), 9);
}

#[test]
fn test_section_labels() {
    assert_eq!(NavSection::Dashboard.label(), "Dashboard");
    assert_eq!(NavSection::Servers.label(), "Servers");
    assert_eq!(NavSection::Sessions.label(), "Sessions");
    assert_eq!(NavSection::Users.label(), "Users");
    assert_eq!(NavSection::Policies.label(), "Policies");
    assert_eq!(NavSection::Gateways.label(), "Gateways");
    assert_eq!(NavSection::Metrics.label(), "Metrics");
    assert_eq!(NavSection::Audit.label(), "Audit Log");
    assert_eq!(NavSection::Plugins.label(), "Plugins");
}

#[test]
fn test_section_icons_are_non_empty() {
    for section in NavSection::ALL {
        assert!(!section.icon().is_empty(), "{section:?} has empty icon");
    }
}

#[test]
fn test_section_min_role() {
    assert_eq!(NavSection::Dashboard.min_role(), AuthRole::Viewer);
    assert_eq!(NavSection::Users.min_role(), AuthRole::Operator);
    assert_eq!(NavSection::Policies.min_role(), AuthRole::Admin);
}

#[test]
fn test_section_display() {
    assert_eq!(NavSection::Dashboard.to_string(), "Dashboard");
    assert_eq!(NavSection::Audit.to_string(), "Audit Log");
}

// ===========================================================================
// NavItem
// ===========================================================================

#[test]
fn test_nav_item_from_section() {
    let item = NavItem::from_section(NavSection::Servers);
    assert_eq!(item.label, "Servers");
    assert_eq!(item.icon, "dns");
    assert!(item.badge.is_none());
}

#[test]
fn test_nav_item_with_badge() {
    let item = NavItem::from_section(NavSection::Sessions).with_badge("42");
    assert_eq!(item.badge, Some("42".to_string()));
}

// ===========================================================================
// NavState — navigation
// ===========================================================================

#[test]
fn test_initial_section_is_dashboard() {
    let state = NavState::new();
    assert_eq!(state.current(), NavSection::Dashboard);
    assert_eq!(state.history_len(), 0);
}

#[test]
fn test_navigate_changes_current() {
    let mut state = NavState::new();
    state.navigate(NavSection::Servers);
    assert_eq!(state.current(), NavSection::Servers);
    assert_eq!(state.history_len(), 1);
}

#[test]
fn test_navigate_same_section_is_noop() {
    let mut state = NavState::new();
    state.navigate(NavSection::Dashboard);
    assert_eq!(state.history_len(), 0);
}

// ===========================================================================
// NavState — history and go_back
// ===========================================================================

#[test]
fn test_go_back_returns_previous() {
    let mut state = NavState::new();
    state.navigate(NavSection::Servers);
    state.navigate(NavSection::Sessions);
    let prev = state.go_back();
    assert_eq!(prev, Some(NavSection::Servers));
    assert_eq!(state.current(), NavSection::Servers);
}

#[test]
fn test_go_back_empty_history_returns_none() {
    let mut state = NavState::new();
    assert_eq!(state.go_back(), None);
    assert_eq!(state.current(), NavSection::Dashboard);
}

// ===========================================================================
// NavState — breadcrumbs
// ===========================================================================

#[test]
fn test_breadcrumbs() {
    let mut state = NavState::new();
    state.navigate(NavSection::Servers);
    state.navigate(NavSection::Sessions);
    let crumbs = state.breadcrumbs();
    assert_eq!(
        crumbs,
        vec![
            NavSection::Dashboard,
            NavSection::Servers,
            NavSection::Sessions,
        ]
    );
}

#[test]
fn test_breadcrumbs_initial() {
    let state = NavState::new();
    let crumbs = state.breadcrumbs();
    assert_eq!(crumbs, vec![NavSection::Dashboard]);
}

// ===========================================================================
// NavState — sidebar filtering
// ===========================================================================

#[test]
fn test_sidebar_items_viewer_role() {
    let state = NavState::new();
    let items = state.sidebar_items(AuthRole::Viewer);
    // Viewer should see Dashboard, Servers, Sessions, Gateways, Metrics (5 items).
    // Users (Operator), Audit (Operator), Policies (Admin), Plugins (Admin) are excluded.
    assert_eq!(items.len(), 5);
    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    assert!(ids.contains(&"dashboard"));
    assert!(ids.contains(&"servers"));
    assert!(!ids.contains(&"users"));
    assert!(!ids.contains(&"policies"));
}

#[test]
fn test_sidebar_items_superadmin_role() {
    let state = NavState::new();
    let items = state.sidebar_items(AuthRole::SuperAdmin);
    // SuperAdmin sees everything.
    assert_eq!(items.len(), 9);
}
