//! Tests for ColumnView, SortOrder, and ColumnConfig.

use crate::column_view::{ColumnConfig, ColumnViewConfig, SortField, SortOrder, ViewMode};

#[test]
fn test_view_mode_display() {
    assert_eq!(ViewMode::Icons.to_string(), "icons");
    assert_eq!(ViewMode::List.to_string(), "list");
    assert_eq!(ViewMode::Compact.to_string(), "compact");
    assert_eq!(ViewMode::Details.to_string(), "details");
}

#[test]
fn test_view_mode_default() {
    assert_eq!(ViewMode::default(), ViewMode::List);
}

#[test]
fn test_sort_order_toggle() {
    assert_eq!(SortOrder::Ascending.toggled(), SortOrder::Descending);
    assert_eq!(SortOrder::Descending.toggled(), SortOrder::Ascending);
}

#[test]
fn test_sort_order_is_ascending() {
    assert!(SortOrder::Ascending.is_ascending());
    assert!(!SortOrder::Descending.is_ascending());
}

#[test]
fn test_sort_order_display() {
    assert_eq!(SortOrder::Ascending.to_string(), "ascending");
    assert_eq!(SortOrder::Descending.to_string(), "descending");
}

#[test]
fn test_sort_field_display() {
    assert_eq!(SortField::Name.to_string(), "name");
    assert_eq!(SortField::Size.to_string(), "size");
    assert_eq!(SortField::Modified.to_string(), "modified");
    assert_eq!(SortField::Type.to_string(), "type");
}

#[test]
fn test_column_config_new() {
    let c = ColumnConfig::new(SortField::Name, 250.0);
    assert_eq!(c.field, SortField::Name);
    assert_eq!(c.width, 250.0);
    assert!(c.visible);
}

#[test]
fn test_column_config_hidden() {
    let c = ColumnConfig::hidden(SortField::Type);
    assert_eq!(c.field, SortField::Type);
    assert!(!c.visible);
}

#[test]
fn test_column_view_config_default() {
    let cfg = ColumnViewConfig::new();
    assert_eq!(cfg.columns.len(), 4);
    assert_eq!(cfg.sort_field, SortField::Name);
    assert_eq!(cfg.sort_order, SortOrder::Ascending);
}

#[test]
fn test_column_view_set_sort_toggle() {
    let mut cfg = ColumnViewConfig::new();
    // Click on Name again should toggle to descending.
    cfg.set_sort(SortField::Name);
    assert_eq!(cfg.sort_order, SortOrder::Descending);
    // Click on Name again should toggle back.
    cfg.set_sort(SortField::Name);
    assert_eq!(cfg.sort_order, SortOrder::Ascending);
}

#[test]
fn test_column_view_set_sort_different_field() {
    let mut cfg = ColumnViewConfig::new();
    cfg.set_sort(SortField::Size);
    assert_eq!(cfg.sort_field, SortField::Size);
    assert_eq!(cfg.sort_order, SortOrder::Ascending);
}

#[test]
fn test_column_view_visible_columns() {
    let mut cfg = ColumnViewConfig::new();
    assert_eq!(cfg.visible_columns().len(), 4);
    cfg.toggle_column(SortField::Type);
    assert_eq!(cfg.visible_columns().len(), 3);
}

#[test]
fn test_column_view_total_width() {
    let cfg = ColumnViewConfig::new();
    let total = cfg.total_width();
    // Default: 300 + 100 + 180 + 120 = 700
    assert!((total - 700.0).abs() < 0.1);
}

#[test]
fn test_column_view_set_column_width() {
    let mut cfg = ColumnViewConfig::new();
    cfg.set_column_width(SortField::Name, 400.0);
    let name_col = cfg
        .columns
        .iter()
        .find(|c| c.field == SortField::Name)
        .unwrap();
    assert_eq!(name_col.width, 400.0);
}
