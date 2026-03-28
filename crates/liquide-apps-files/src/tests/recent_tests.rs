//! Tests for the recent module.

use crate::recent::{RecentEntry, RecentStore, DEFAULT_MAX_ENTRIES};

#[test]
fn test_recent_entry_new() {
    let e = RecentEntry::new(
        "file:///doc.txt".into(),
        "doc.txt".into(),
        "text/plain".into(),
        1000,
        "org.gnome.TextEditor".into(),
    );
    assert_eq!(e.uri, "file:///doc.txt");
    assert_eq!(e.access_count, 1);
    assert_eq!(e.last_accessed_ms, 1000);
}

#[test]
fn test_recent_entry_touch() {
    let mut e = RecentEntry::new(
        "file:///a.txt".into(), "a.txt".into(), "text/plain".into(), 100, "app1".into(),
    );
    e.touch(200, "app2");
    assert_eq!(e.access_count, 2);
    assert_eq!(e.last_accessed_ms, 200);
    assert_eq!(e.app_id, "app2");
}

#[test]
fn test_store_new_is_empty() {
    let store = RecentStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
    assert_eq!(store.max_entries(), DEFAULT_MAX_ENTRIES);
}

#[test]
fn test_store_add_and_list() {
    let mut store = RecentStore::new();
    store.add("file:///a", "a", "text/plain", 300, "app");
    store.add("file:///b", "b", "text/plain", 100, "app");
    store.add("file:///c", "c", "text/plain", 200, "app");
    assert_eq!(store.len(), 3);

    let list = store.list();
    // Sorted by last_accessed_ms descending.
    assert_eq!(list[0].uri, "file:///a");
    assert_eq!(list[1].uri, "file:///c");
    assert_eq!(list[2].uri, "file:///b");
}

#[test]
fn test_store_add_duplicate_increments() {
    let mut store = RecentStore::new();
    store.add("file:///a", "a", "text/plain", 100, "app1");
    store.add("file:///a", "a", "text/plain", 200, "app2");
    assert_eq!(store.len(), 1);
    let e = store.find("file:///a").unwrap();
    assert_eq!(e.access_count, 2);
    assert_eq!(e.last_accessed_ms, 200);
    assert_eq!(e.app_id, "app2");
}

#[test]
fn test_store_remove() {
    let mut store = RecentStore::new();
    store.add("file:///a", "a", "text/plain", 100, "app");
    assert!(store.remove("file:///a"));
    assert!(store.is_empty());
    assert!(!store.remove("file:///nonexistent"));
}

#[test]
fn test_store_clear() {
    let mut store = RecentStore::new();
    store.add("file:///a", "a", "text/plain", 100, "app");
    store.add("file:///b", "b", "text/plain", 200, "app");
    store.clear();
    assert!(store.is_empty());
}

#[test]
fn test_store_max_entries_enforced() {
    let mut store = RecentStore::with_max(3);
    for i in 0..5 {
        store.add(
            &format!("file:///{i}"),
            &format!("{i}"),
            "text/plain",
            i as u64 * 100,
            "app",
        );
    }
    assert_eq!(store.len(), 3);
    // Oldest entries should have been evicted.
    assert!(store.find("file:///0").is_none());
    assert!(store.find("file:///1").is_none());
    assert!(store.find("file:///2").is_some());
}

#[test]
fn test_store_set_max_entries_evicts() {
    let mut store = RecentStore::new();
    for i in 0..10 {
        store.add(
            &format!("file:///{i}"),
            &format!("{i}"),
            "text/plain",
            i as u64 * 100,
            "app",
        );
    }
    store.set_max_entries(5);
    assert_eq!(store.len(), 5);
}

#[test]
fn test_store_frequently_used() {
    let mut store = RecentStore::new();
    store.add("file:///a", "a", "text/plain", 100, "app");
    // Access "a" 4 more times.
    for t in 1..5 {
        store.add("file:///a", "a", "text/plain", 100 + t * 10, "app");
    }
    store.add("file:///b", "b", "text/plain", 200, "app");

    let freq = store.frequently_used(1);
    assert_eq!(freq.len(), 1);
    assert_eq!(freq[0].uri, "file:///a");
    assert_eq!(freq[0].access_count, 5);
}

#[test]
fn test_store_purge_older_than() {
    let mut store = RecentStore::new();
    let day_ms: u64 = 86_400_000;
    let now = 10 * day_ms;
    store.add("file:///old", "old", "text/plain", 1 * day_ms, "app"); // 9 days ago
    store.add("file:///recent", "recent", "text/plain", 8 * day_ms, "app"); // 2 days ago
    store.purge_older_than(7, now);
    assert_eq!(store.len(), 1);
    assert!(store.find("file:///recent").is_some());
    assert!(store.find("file:///old").is_none());
}

#[test]
fn test_store_find() {
    let mut store = RecentStore::new();
    store.add("file:///x", "x", "text/plain", 100, "app");
    assert!(store.find("file:///x").is_some());
    assert!(store.find("file:///y").is_none());
}

#[test]
fn test_store_serialize_deserialize() {
    let mut store = RecentStore::new();
    store.add("file:///a", "a.txt", "text/plain", 100, "editor");
    store.add("file:///b", "b.rs", "text/x-rust", 200, "ide");

    let data = store.serialize();
    assert!(!data.is_empty());

    let mut store2 = RecentStore::new();
    store2.deserialize(&data);
    assert_eq!(store2.len(), 2);
    let e = store2.find("file:///a").unwrap();
    assert_eq!(e.display_name, "a.txt");
    assert_eq!(e.mime_type, "text/plain");
    assert_eq!(e.last_accessed_ms, 100);
    assert_eq!(e.app_id, "editor");
}

#[test]
fn test_store_deserialize_bad_lines_skipped() {
    let mut store = RecentStore::new();
    let data = "bad line\nfile:///ok\tok\ttext/plain\t100\t1\tapp\n\n";
    store.deserialize(data);
    assert_eq!(store.len(), 1);
}

#[test]
fn test_store_with_max_zero() {
    let mut store = RecentStore::with_max(0);
    store.add("file:///a", "a", "text/plain", 100, "app");
    // max_entries=0 means everything gets evicted immediately.
    assert_eq!(store.len(), 0);
}
