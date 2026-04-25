//! Tests for Bookmark, BookmarkManager, and default_bookmarks.

use crate::sidebar::{Bookmark, BookmarkManager, default_bookmarks};

#[test]
fn test_bookmark_new() {
    let b = Bookmark::new("Projects".into(), "/home/user/Projects".into());
    assert_eq!(b.name, "Projects");
    assert!(!b.is_system);
    assert!(b.icon.is_none());
}

#[test]
fn test_bookmark_system() {
    let b = Bookmark::system("Home", "/home/user", "folder-home");
    assert!(b.is_system);
    assert_eq!(b.icon, Some("folder-home".to_string()));
}

#[test]
fn test_bookmark_icon_name_fallback() {
    let b = Bookmark::new("Custom".into(), "/custom".into());
    assert_eq!(b.icon_name(), "folder"); // default fallback
}

#[test]
fn test_bookmark_icon_name_set() {
    let b = Bookmark::system("Home", "/home", "folder-home");
    assert_eq!(b.icon_name(), "folder-home");
}

#[test]
fn test_default_bookmarks() {
    let bookmarks = default_bookmarks();
    assert_eq!(bookmarks.len(), 7);
    let names: Vec<&str> = bookmarks.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"Home"));
    assert!(names.contains(&"Desktop"));
    assert!(names.contains(&"Documents"));
    assert!(names.contains(&"Downloads"));
    assert!(names.contains(&"Music"));
    assert!(names.contains(&"Pictures"));
    assert!(names.contains(&"Videos"));
    // All default bookmarks are system bookmarks.
    assert!(bookmarks.iter().all(|b| b.is_system));
}

#[test]
fn test_bookmark_manager_new_has_defaults() {
    let bm = BookmarkManager::new();
    assert_eq!(bm.count(), 7);
}

#[test]
fn test_bookmark_manager_empty() {
    let bm = BookmarkManager::empty();
    assert_eq!(bm.count(), 0);
}

#[test]
fn test_bookmark_manager_add() {
    let mut bm = BookmarkManager::empty();
    assert!(bm.add("Proj".into(), "/projects".into(), None));
    assert_eq!(bm.count(), 1);
    assert_eq!(bm.user_count(), 1);
}

#[test]
fn test_bookmark_manager_add_with_icon() {
    let mut bm = BookmarkManager::empty();
    bm.add(
        "Proj".into(),
        "/projects".into(),
        Some("folder-code".into()),
    );
    let b = bm.find("Proj").unwrap();
    assert_eq!(b.icon_name(), "folder-code");
}

#[test]
fn test_bookmark_manager_no_duplicate() {
    let mut bm = BookmarkManager::empty();
    assert!(bm.add("A".into(), "/a".into(), None));
    assert!(!bm.add("B".into(), "/a".into(), None)); // same path
    assert_eq!(bm.count(), 1);
}

#[test]
fn test_bookmark_manager_remove() {
    let mut bm = BookmarkManager::empty();
    bm.add("A".into(), "/a".into(), None);
    bm.remove("/a").unwrap();
    assert_eq!(bm.count(), 0);
}

#[test]
fn test_bookmark_manager_cannot_remove_system() {
    let mut bm = BookmarkManager::new();
    let home_path = bm.find("Home").unwrap().path.clone();
    // System bookmarks should not be removed.
    bm.remove(&home_path).ok();
    assert!(bm.find("Home").is_some());
}

#[test]
fn test_bookmark_manager_reorder() {
    let mut bm = BookmarkManager::empty();
    bm.add("A".into(), "/a".into(), None);
    bm.add("B".into(), "/b".into(), None);
    bm.add("C".into(), "/c".into(), None);
    bm.reorder(0, 2);
    assert_eq!(bm.bookmarks()[0].name, "B");
    assert_eq!(bm.bookmarks()[1].name, "C");
    assert_eq!(bm.bookmarks()[2].name, "A");
}

#[test]
fn test_bookmark_manager_move_up() {
    let mut bm = BookmarkManager::empty();
    bm.add("A".into(), "/a".into(), None);
    bm.add("B".into(), "/b".into(), None);
    bm.move_up(1);
    assert_eq!(bm.bookmarks()[0].name, "B");
    assert_eq!(bm.bookmarks()[1].name, "A");
}

#[test]
fn test_bookmark_manager_move_down() {
    let mut bm = BookmarkManager::empty();
    bm.add("A".into(), "/a".into(), None);
    bm.add("B".into(), "/b".into(), None);
    bm.move_down(0);
    assert_eq!(bm.bookmarks()[0].name, "B");
    assert_eq!(bm.bookmarks()[1].name, "A");
}

#[test]
fn test_bookmark_manager_user_count() {
    let mut bm = BookmarkManager::new();
    assert_eq!(bm.user_count(), 0);
    bm.add("Custom".into(), "/custom".into(), None);
    assert_eq!(bm.user_count(), 1);
}
