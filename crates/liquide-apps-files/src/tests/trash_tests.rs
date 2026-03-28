//! Tests for the TrashManager.

use crate::trash::{TrashEntry, TrashManager};

#[test]
fn test_trash_entry_original_name() {
    let e = TrashEntry::new("/home/user/doc.txt".into(), "/trash/1_doc.txt".into(), 1000, 500);
    assert_eq!(e.original_name(), "doc.txt");
}

#[test]
fn test_trash_entry_original_name_no_slash() {
    let e = TrashEntry::new("file.txt".into(), "trash".into(), 0, 0);
    assert_eq!(e.original_name(), "file.txt");
}

#[test]
fn test_trash_manager_new_is_empty() {
    let tm = TrashManager::new();
    assert_eq!(tm.count(), 0);
    assert_eq!(tm.total_size(), 0);
}

#[test]
fn test_trash_manager_trash_file() {
    let mut tm = TrashManager::with_dir("/tmp/trash".into());
    let entry = tm.trash("/home/user/file.txt", 1024).unwrap();
    assert_eq!(entry.original_path, "/home/user/file.txt");
    assert_eq!(entry.size, 1024);
    assert!(entry.trash_path.starts_with("/tmp/trash/files/"));
    assert_eq!(tm.count(), 1);
    assert_eq!(tm.total_size(), 1024);
}

#[test]
fn test_trash_manager_trash_multiple() {
    let mut tm = TrashManager::with_dir("/tmp/trash".into());
    tm.trash("/a", 100).unwrap();
    tm.trash("/b", 200).unwrap();
    tm.trash("/c", 300).unwrap();
    assert_eq!(tm.count(), 3);
    assert_eq!(tm.total_size(), 600);
}

#[test]
fn test_trash_manager_restore() {
    let mut tm = TrashManager::with_dir("/tmp/trash".into());
    let entry = tm.trash("/home/user/file.txt", 512).unwrap();
    tm.restore(&entry).unwrap();
    assert_eq!(tm.count(), 0);
}

#[test]
fn test_trash_manager_restore_nonexistent() {
    let mut tm = TrashManager::with_dir("/tmp/trash".into());
    let fake = TrashEntry::new("/x".into(), "/nonexistent".into(), 0, 0);
    let result = tm.restore(&fake);
    assert!(result.is_err());
}

#[test]
fn test_trash_manager_empty_trash() {
    let mut tm = TrashManager::with_dir("/tmp/trash".into());
    tm.trash("/a", 100).unwrap();
    tm.trash("/b", 200).unwrap();
    tm.empty_trash();
    assert_eq!(tm.count(), 0);
    assert_eq!(tm.total_size(), 0);
}

#[test]
fn test_trash_manager_list_trash() {
    let mut tm = TrashManager::with_dir("/tmp/trash".into());
    tm.trash("/a", 100).unwrap();
    tm.trash("/b", 200).unwrap();
    let items = tm.list_trash();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].original_path, "/a");
    assert_eq!(items[1].original_path, "/b");
}

#[test]
fn test_trash_manager_find_by_original() {
    let mut tm = TrashManager::with_dir("/tmp/trash".into());
    tm.trash("/home/doc.txt", 512).unwrap();
    let found = tm.find_by_original("/home/doc.txt");
    assert!(found.is_some());
    assert!(tm.find_by_original("/nonexistent").is_none());
}

#[test]
fn test_trash_manager_platform_dir() {
    let dir = TrashManager::platform_trash_dir();
    // Should return a non-empty string on any platform.
    assert!(!dir.is_empty());
}
