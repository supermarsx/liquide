use crate::retention::{RecordingEntry, RetentionPolicy};

#[test]
fn test_unlimited_policy() {
    let p = RetentionPolicy::unlimited();
    let entries = vec![
        RecordingEntry::new("a", 0, 1_000_000),
        RecordingEntry::new("b", 1000, 2_000_000),
    ];
    let to_delete = p.enforce(&entries, 999_999_999_999);
    assert!(to_delete.is_empty());
}

#[test]
fn test_max_age_deletes() {
    let p = RetentionPolicy {
        max_age_hours: Some(1),
        max_size_bytes: None,
        max_recordings: None,
    };
    let one_hour_us = 3_600_000_000u64;
    let entries = vec![
        RecordingEntry::new("old", 0, 1000),
        RecordingEntry::new("new", one_hour_us + 1000, 1000),
    ];
    let to_delete = p.enforce(&entries, one_hour_us + 2000);
    assert_eq!(to_delete, vec!["old"]);
}

#[test]
fn test_max_size_deletes() {
    let p = RetentionPolicy {
        max_age_hours: None,
        max_size_bytes: Some(5000),
        max_recordings: None,
    };
    let entries = vec![
        RecordingEntry::new("a", 0, 3000),
        RecordingEntry::new("b", 1000, 3000),
    ];
    let to_delete = p.enforce(&entries, 2000);
    assert_eq!(to_delete, vec!["a"]);
}

#[test]
fn test_max_count_deletes() {
    let p = RetentionPolicy {
        max_age_hours: None,
        max_size_bytes: None,
        max_recordings: Some(2),
    };
    let entries = vec![
        RecordingEntry::new("a", 0, 100),
        RecordingEntry::new("b", 1000, 100),
        RecordingEntry::new("c", 2000, 100),
    ];
    let to_delete = p.enforce(&entries, 3000);
    assert_eq!(to_delete, vec!["a"]);
}

#[test]
fn test_combined_policy() {
    let p = RetentionPolicy {
        max_age_hours: Some(1),
        max_size_bytes: None,
        max_recordings: Some(1),
    };
    let one_hour_us = 3_600_000_000u64;
    let entries = vec![
        RecordingEntry::new("ancient", 0, 100),
        RecordingEntry::new("recent1", one_hour_us + 100, 100),
        RecordingEntry::new("recent2", one_hour_us + 200, 100),
    ];
    let to_delete = p.enforce(&entries, one_hour_us + 300);
    assert!(to_delete.contains(&"ancient".to_string()));
    assert!(to_delete.contains(&"recent1".to_string()));
    assert!(!to_delete.contains(&"recent2".to_string()));
}

#[test]
fn test_empty_entries() {
    let p = RetentionPolicy {
        max_age_hours: Some(1),
        max_size_bytes: Some(1000),
        max_recordings: Some(5),
    };
    let to_delete = p.enforce(&[], 999_999);
    assert!(to_delete.is_empty());
}
