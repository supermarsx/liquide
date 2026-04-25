//! Tests for setting entries and values.

use crate::apply::ChangeTracker;
use crate::category::Category;
use crate::entry::{SettingEntry, SettingKind, SettingValue};
use crate::notify::NotificationQueue;
use crate::page;
use crate::policy::{PolicyConstraint, PolicyEngine};
use crate::search::SettingsSearch;

// ===========================================================================
// SettingEntry
// ===========================================================================

#[test]
fn test_toggle_entry() {
    let e = SettingEntry::toggle("a.b", "Test", "Desc", Category::Display, "Sec", true);
    assert_eq!(e.key, "a.b");
    assert!(!e.is_modified());
    assert!(matches!(e.value, SettingValue::Bool(true)));
}

#[test]
fn test_slider_entry() {
    let e = SettingEntry::slider(
        "a.b",
        "Test",
        "Desc",
        Category::Audio,
        "Sec",
        0.0,
        100.0,
        1.0,
        50.0,
    );
    assert!(matches!(e.kind, SettingKind::Slider { .. }));
    assert!(!e.is_modified());
}

#[test]
fn test_choice_entry() {
    let e = SettingEntry::choice(
        "a.b",
        "Test",
        "Desc",
        Category::Input,
        "Sec",
        vec!["x".into(), "y".into()],
        "x",
    );
    assert!(matches!(e.kind, SettingKind::Choice { .. }));
}

#[test]
fn test_text_entry() {
    let e = SettingEntry::text(
        "a.b",
        "Test",
        "Desc",
        Category::Network,
        "Sec",
        128,
        "hello",
    );
    assert_eq!(e.value, SettingValue::Text("hello".into()));
}

#[test]
fn test_entry_is_modified() {
    let mut e = SettingEntry::toggle("a.b", "Test", "Desc", Category::Display, "Sec", false);
    assert!(!e.is_modified());
    e.value = SettingValue::Bool(true);
    assert!(e.is_modified());
}

#[test]
fn test_entry_reset() {
    let mut e = SettingEntry::toggle("a.b", "Test", "Desc", Category::Display, "Sec", false);
    e.value = SettingValue::Bool(true);
    e.reset();
    assert!(!e.is_modified());
}

#[test]
fn test_validate_toggle_ok() {
    let e = SettingEntry::toggle("a.b", "Test", "Desc", Category::Display, "Sec", false);
    assert!(e.validate(&SettingValue::Bool(true)).is_ok());
}

#[test]
fn test_validate_toggle_type_mismatch() {
    let e = SettingEntry::toggle("a.b", "Test", "Desc", Category::Display, "Sec", false);
    assert!(e.validate(&SettingValue::Number(1.0)).is_err());
}

#[test]
fn test_validate_slider_ok() {
    let e = SettingEntry::slider(
        "a.b",
        "Test",
        "Desc",
        Category::Audio,
        "Sec",
        0.0,
        100.0,
        1.0,
        50.0,
    );
    assert!(e.validate(&SettingValue::Number(75.0)).is_ok());
}

#[test]
fn test_validate_slider_out_of_range() {
    let e = SettingEntry::slider(
        "a.b",
        "Test",
        "Desc",
        Category::Audio,
        "Sec",
        0.0,
        100.0,
        1.0,
        50.0,
    );
    assert!(e.validate(&SettingValue::Number(150.0)).is_err());
}

#[test]
fn test_validate_choice_ok() {
    let e = SettingEntry::choice(
        "a.b",
        "Test",
        "Desc",
        Category::Input,
        "Sec",
        vec!["x".into(), "y".into()],
        "x",
    );
    assert!(e.validate(&SettingValue::Text("y".into())).is_ok());
}

#[test]
fn test_validate_choice_invalid() {
    let e = SettingEntry::choice(
        "a.b",
        "Test",
        "Desc",
        Category::Input,
        "Sec",
        vec!["x".into(), "y".into()],
        "x",
    );
    assert!(e.validate(&SettingValue::Text("z".into())).is_err());
}

#[test]
fn test_validate_text_ok() {
    let e = SettingEntry::text("a.b", "Test", "Desc", Category::Network, "Sec", 10, "");
    assert!(e.validate(&SettingValue::Text("short".into())).is_ok());
}

#[test]
fn test_validate_text_too_long() {
    let e = SettingEntry::text("a.b", "Test", "Desc", Category::Network, "Sec", 5, "");
    assert!(e.validate(&SettingValue::Text("too long".into())).is_err());
}

#[test]
fn test_setting_value_display() {
    assert_eq!(format!("{}", SettingValue::Bool(true)), "true");
    assert_eq!(format!("{}", SettingValue::Number(42.5)), "42.5");
    assert_eq!(format!("{}", SettingValue::Text("hi".into())), "hi");
}

// ===========================================================================
// ChangeTracker
// ===========================================================================

#[test]
fn test_change_tracker_new() {
    let ct = ChangeTracker::new();
    assert!(!ct.has_pending());
    assert!(!ct.can_undo());
    assert!(!ct.can_redo());
}

#[test]
fn test_change_tracker_record() {
    let mut ct = ChangeTracker::new();
    ct.record(crate::apply::SettingChange {
        key: "a".into(),
        old_value: SettingValue::Bool(false),
        new_value: SettingValue::Bool(true),
    });
    assert!(ct.has_pending());
    assert_eq!(ct.pending_count(), 1);
}

#[test]
fn test_change_tracker_apply() {
    let mut ct = ChangeTracker::new();
    ct.record(crate::apply::SettingChange {
        key: "a".into(),
        old_value: SettingValue::Bool(false),
        new_value: SettingValue::Bool(true),
    });
    let applied = ct.apply();
    assert_eq!(applied.len(), 1);
    assert!(!ct.has_pending());
    assert!(ct.can_undo());
}

#[test]
fn test_change_tracker_undo_redo() {
    let mut ct = ChangeTracker::new();
    ct.record(crate::apply::SettingChange {
        key: "a".into(),
        old_value: SettingValue::Bool(false),
        new_value: SettingValue::Bool(true),
    });
    ct.apply();

    let undone = ct.undo().unwrap();
    assert_eq!(undone.new_value, SettingValue::Bool(false));
    assert!(ct.can_redo());

    let redone = ct.redo().unwrap();
    assert_eq!(redone.new_value, SettingValue::Bool(true));
}

#[test]
fn test_change_tracker_nothing_to_undo() {
    let mut ct = ChangeTracker::new();
    assert!(ct.undo().is_err());
}

#[test]
fn test_change_tracker_nothing_to_redo() {
    let mut ct = ChangeTracker::new();
    assert!(ct.redo().is_err());
}

#[test]
fn test_change_tracker_discard() {
    let mut ct = ChangeTracker::new();
    ct.record(crate::apply::SettingChange {
        key: "a".into(),
        old_value: SettingValue::Bool(false),
        new_value: SettingValue::Bool(true),
    });
    ct.discard();
    assert!(!ct.has_pending());
}

// ===========================================================================
// PolicyEngine
// ===========================================================================

#[test]
fn test_policy_empty() {
    let pe = PolicyEngine::new();
    assert!(pe.is_editable("any.key"));
    assert!(pe.is_visible("any.key"));
    assert_eq!(pe.constraint_count(), 0);
}

#[test]
fn test_policy_locked() {
    let mut pe = PolicyEngine::new();
    pe.set_constraint("a.b", PolicyConstraint::Locked);
    assert!(!pe.is_editable("a.b"));
    assert!(pe.is_visible("a.b"));
}

#[test]
fn test_policy_hidden() {
    let mut pe = PolicyEngine::new();
    pe.set_constraint("a.b", PolicyConstraint::Hidden);
    assert!(!pe.is_visible("a.b"));
}

#[test]
fn test_policy_read_only() {
    let mut pe = PolicyEngine::new();
    pe.set_constraint("a.b", PolicyConstraint::ReadOnly);
    assert!(!pe.is_editable("a.b"));
    assert!(pe.is_visible("a.b"));
}

#[test]
fn test_policy_remove() {
    let mut pe = PolicyEngine::new();
    pe.set_constraint("a.b", PolicyConstraint::Locked);
    pe.remove_constraint("a.b");
    assert!(pe.is_editable("a.b"));
}

// ===========================================================================
// Search
// ===========================================================================

#[test]
fn test_search_basic() {
    let mut s = SettingsSearch::new(10);
    let entries = vec![
        SettingEntry::toggle(
            "a.b",
            "Volume",
            "Master volume",
            Category::Audio,
            "Sec",
            false,
        ),
        SettingEntry::toggle("c.d", "Mute", "Mute", Category::Audio, "Sec", false),
    ];
    s.search("volume", &entries);
    assert_eq!(s.result_count(), 1);
    assert_eq!(s.results()[0].key, "a.b");
}

#[test]
fn test_search_empty_query() {
    let mut s = SettingsSearch::new(10);
    let entries = vec![SettingEntry::toggle(
        "a.b",
        "Volume",
        "Desc",
        Category::Audio,
        "Sec",
        false,
    )];
    s.search("", &entries);
    assert_eq!(s.result_count(), 0);
}

#[test]
fn test_search_clear() {
    let mut s = SettingsSearch::new(10);
    let entries = vec![SettingEntry::toggle(
        "a.b",
        "Volume",
        "Desc",
        Category::Audio,
        "Sec",
        false,
    )];
    s.search("volume", &entries);
    s.clear();
    assert_eq!(s.result_count(), 0);
    assert!(s.query().is_empty());
}

#[test]
fn test_search_history() {
    let mut s = SettingsSearch::new(3);
    let entries = vec![SettingEntry::toggle(
        "a.b",
        "Volume",
        "Desc",
        Category::Audio,
        "Sec",
        false,
    )];
    s.search("volume", &entries);
    s.commit_to_history();
    s.search("mute", &entries);
    s.commit_to_history();
    assert_eq!(s.history().len(), 2);
}

#[test]
fn test_search_history_limit() {
    let mut s = SettingsSearch::new(2);
    let entries: Vec<SettingEntry> = Vec::new();
    s.search("a", &entries);
    s.commit_to_history();
    s.search("b", &entries);
    s.commit_to_history();
    s.search("c", &entries);
    s.commit_to_history();
    assert_eq!(s.history().len(), 2);
}

// ===========================================================================
// Notification queue
// ===========================================================================

#[test]
fn test_notification_queue() {
    let mut q = NotificationQueue::new();
    assert!(q.is_empty());
    q.push("a.b", SettingValue::Bool(true), 1000);
    assert_eq!(q.len(), 1);
    let drained = q.drain();
    assert_eq!(drained.len(), 1);
    assert!(q.is_empty());
}

#[test]
fn test_notification_batching() {
    let mut q = NotificationQueue::new();
    assert!(!q.is_batching());
    q.set_batching(true);
    assert!(q.is_batching());
}

// ===========================================================================
// Default pages
// ===========================================================================

#[test]
fn test_default_pages() {
    let (pages, entries) = page::default_pages();
    assert_eq!(pages.len(), 8); // One per category.
    assert!(entries.len() >= 25); // Plenty of settings.
}

#[test]
fn test_default_pages_all_categories() {
    let (pages, _) = page::default_pages();
    for cat in crate::category::Category::ALL {
        assert!(
            pages.iter().any(|p| p.category == *cat),
            "Missing page for {:?}",
            cat
        );
    }
}
