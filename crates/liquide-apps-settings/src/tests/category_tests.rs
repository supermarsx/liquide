//! Tests for setting categories.

use crate::category::{Category, CategoryInfo};

#[test]
fn test_category_all() {
    assert_eq!(Category::ALL.len(), 8);
}

#[test]
fn test_category_label() {
    assert_eq!(Category::Display.label(), "Display");
    assert_eq!(Category::Users.label(), "Users & Accounts");
}

#[test]
fn test_category_icon() {
    assert_eq!(Category::Audio.icon(), "audio-volume");
    assert_eq!(Category::Network.icon(), "network-wired");
}

#[test]
fn test_category_description() {
    let desc = Category::Display.description();
    assert!(desc.contains("Resolution"));
}

#[test]
fn test_category_from_id() {
    assert_eq!(Category::from_id("display"), Some(Category::Display));
    assert_eq!(Category::from_id("audio"), Some(Category::Audio));
    assert!(Category::from_id("nonexistent").is_none());
}

#[test]
fn test_category_id() {
    assert_eq!(Category::Privacy.id(), "privacy");
    assert_eq!(Category::System.id(), "system");
}

#[test]
fn test_category_display() {
    assert_eq!(format!("{}", Category::Input), "Input");
}

#[test]
fn test_category_info() {
    let info = CategoryInfo::new(Category::Display);
    assert_eq!(info.category, Category::Display);
    assert_eq!(info.entry_count, 0);
    assert!(!info.has_pending_changes);
}
