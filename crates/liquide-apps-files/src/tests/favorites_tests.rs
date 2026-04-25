//! Tests for the favorites module.

use crate::favorites::{Favorite, FavoriteStore};

#[test]
fn test_favorite_new() {
    let f = Favorite::new("file:///a".into(), "A".into(), "folder".into(), 0);
    assert_eq!(f.uri, "file:///a");
    assert_eq!(f.display_name, "A");
    assert_eq!(f.icon, "folder");
    assert_eq!(f.position, 0);
}

#[test]
fn test_store_new_has_defaults() {
    let store = FavoriteStore::new();
    assert!(store.len() >= 6); // Home, Documents, Downloads, Music, Pictures, Videos
    assert!(!store.is_empty());
}

#[test]
fn test_store_empty() {
    let store = FavoriteStore::empty();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_add() {
    let mut store = FavoriteStore::empty();
    assert!(store.add("file:///projects", "Projects", "folder-code"));
    assert_eq!(store.len(), 1);
    let f = store.find("file:///projects").unwrap();
    assert_eq!(f.display_name, "Projects");
    assert_eq!(f.position, 0);
}

#[test]
fn test_store_add_duplicate() {
    let mut store = FavoriteStore::empty();
    assert!(store.add("file:///a", "A", "folder"));
    assert!(!store.add("file:///a", "A2", "folder"));
    assert_eq!(store.len(), 1);
}

#[test]
fn test_store_remove() {
    let mut store = FavoriteStore::empty();
    store.add("file:///a", "A", "folder");
    store.add("file:///b", "B", "folder");
    assert!(store.remove("file:///a"));
    assert_eq!(store.len(), 1);
    assert!(!store.is_favorite("file:///a"));
    // Position should be reindexed.
    assert_eq!(store.list()[0].position, 0);
}

#[test]
fn test_store_remove_nonexistent() {
    let mut store = FavoriteStore::empty();
    assert!(!store.remove("file:///nope"));
}

#[test]
fn test_store_reorder() {
    let mut store = FavoriteStore::empty();
    store.add("file:///a", "A", "folder");
    store.add("file:///b", "B", "folder");
    store.add("file:///c", "C", "folder");
    store.reorder(0, 2);
    assert_eq!(store.list()[0].display_name, "B");
    assert_eq!(store.list()[1].display_name, "C");
    assert_eq!(store.list()[2].display_name, "A");
    // Positions should be updated.
    assert_eq!(store.list()[0].position, 0);
    assert_eq!(store.list()[1].position, 1);
    assert_eq!(store.list()[2].position, 2);
}

#[test]
fn test_store_reorder_out_of_bounds() {
    let mut store = FavoriteStore::empty();
    store.add("file:///a", "A", "folder");
    store.reorder(0, 99); // should be a no-op
    assert_eq!(store.list()[0].display_name, "A");
}

#[test]
fn test_store_is_favorite() {
    let mut store = FavoriteStore::empty();
    store.add("file:///a", "A", "folder");
    assert!(store.is_favorite("file:///a"));
    assert!(!store.is_favorite("file:///b"));
}

#[test]
fn test_store_find() {
    let mut store = FavoriteStore::empty();
    store.add("file:///a", "A", "folder");
    assert!(store.find("file:///a").is_some());
    assert!(store.find("file:///z").is_none());
}

#[test]
fn test_store_serialize_deserialize() {
    let mut store = FavoriteStore::empty();
    store.add("file:///home/user/Projects", "Projects", "folder-code");
    store.add("file:///tmp", "Temp", "folder-temp");

    let data = store.serialize();
    assert!(!data.is_empty());

    let mut store2 = FavoriteStore::empty();
    store2.deserialize(&data);
    assert_eq!(store2.len(), 2);
    assert_eq!(store2.list()[0].uri, "file:///home/user/Projects");
    assert_eq!(store2.list()[0].display_name, "Projects");
    assert_eq!(store2.list()[0].icon, "folder-code");
}

#[test]
fn test_store_deserialize_empty_lines_skipped() {
    let mut store = FavoriteStore::empty();
    store.deserialize("file:///a A folder\n\nfile:///b B folder\n");
    assert_eq!(store.len(), 2);
}

#[test]
fn test_store_default_favorites_have_uris() {
    let store = FavoriteStore::new();
    for f in store.list() {
        assert!(
            f.uri.starts_with("file://"),
            "uri should start with file://: {}",
            f.uri
        );
        assert!(!f.display_name.is_empty());
        assert!(!f.icon.is_empty());
    }
}
