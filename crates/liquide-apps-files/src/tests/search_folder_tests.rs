//! Tests for the search_folder module.

use crate::entry::FileEntry;
use crate::search_folder::*;

fn make_file(name: &str, ext: &str, size: u64, modified: u64, mime: &str) -> FileEntry {
    FileEntry {
        name: name.to_string(),
        path: format!("/home/user/{name}"),
        kind: crate::entry::EntryKind::File,
        size,
        modified,
        extension: ext.to_string(),
        hidden: name.starts_with('.'),
        permissions: crate::entry::Permissions::from_mode(0o644),
        symlink_target: None,
        mime_type: mime.to_string(),
    }
}

// -----------------------------------------------------------------------
// SearchFilter tests
// -----------------------------------------------------------------------

#[test]
fn test_filter_empty_matches_everything() {
    let f = SearchFilter::new();
    assert!(!f.is_active());
    let entry = make_file("test.txt", "txt", 100, 1000, "text/plain");
    assert!(f.matches(&entry));
}

#[test]
fn test_filter_name_pattern_star_ext() {
    let f = SearchFilter {
        name_pattern: "*.rs".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file("main.rs", "rs", 100, 1000, "text/x-rust")));
    assert!(!f.matches(&make_file("main.py", "py", 100, 1000, "text/x-python")));
}

#[test]
fn test_filter_name_pattern_prefix() {
    let f = SearchFilter {
        name_pattern: "readme*".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file("readme.md", "md", 100, 1000, "text/plain")));
    assert!(!f.matches(&make_file("license.md", "md", 100, 1000, "text/plain")));
}

#[test]
fn test_filter_name_pattern_contains() {
    let f = SearchFilter {
        name_pattern: "*config*".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file(
        "my_config_file.toml",
        "toml",
        100,
        1000,
        "text/x-config"
    )));
    assert!(!f.matches(&make_file("readme.md", "md", 100, 1000, "text/plain")));
}

#[test]
fn test_filter_name_pattern_exact() {
    let f = SearchFilter {
        name_pattern: "Cargo.toml".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file("Cargo.toml", "toml", 100, 1000, "text/x-config")));
    assert!(f.matches(&make_file("cargo.toml", "toml", 100, 1000, "text/x-config"))); // case-insensitive
    assert!(!f.matches(&make_file(
        "Cargo.lock",
        "lock",
        100,
        1000,
        "application/octet-stream"
    )));
}

#[test]
fn test_filter_extension() {
    let f = SearchFilter {
        extension_filter: "png".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file("photo.png", "png", 100, 1000, "image/png")));
    assert!(f.matches(&make_file("photo.PNG", "PNG", 100, 1000, "image/png")));
    assert!(!f.matches(&make_file("photo.jpg", "jpg", 100, 1000, "image/jpeg")));
}

#[test]
fn test_filter_min_size() {
    let f = SearchFilter {
        min_size: 1000,
        ..Default::default()
    };
    assert!(!f.matches(&make_file("small.txt", "txt", 500, 1000, "text/plain")));
    assert!(f.matches(&make_file("big.txt", "txt", 5000, 1000, "text/plain")));
}

#[test]
fn test_filter_max_size() {
    let f = SearchFilter {
        max_size: 1000,
        ..Default::default()
    };
    assert!(f.matches(&make_file("small.txt", "txt", 500, 1000, "text/plain")));
    assert!(!f.matches(&make_file("big.txt", "txt", 5000, 1000, "text/plain")));
}

#[test]
fn test_filter_size_range() {
    let f = SearchFilter {
        min_size: 100,
        max_size: 1000,
        ..Default::default()
    };
    assert!(!f.matches(&make_file("tiny.txt", "txt", 50, 1000, "text/plain")));
    assert!(f.matches(&make_file("mid.txt", "txt", 500, 1000, "text/plain")));
    assert!(!f.matches(&make_file("huge.txt", "txt", 5000, 1000, "text/plain")));
}

#[test]
fn test_filter_modified_after() {
    let f = SearchFilter {
        modified_after: 500,
        ..Default::default()
    };
    assert!(!f.matches(&make_file("old.txt", "txt", 100, 400, "text/plain")));
    assert!(f.matches(&make_file("new.txt", "txt", 100, 600, "text/plain")));
}

#[test]
fn test_filter_modified_before() {
    let f = SearchFilter {
        modified_before: 500,
        ..Default::default()
    };
    assert!(f.matches(&make_file("old.txt", "txt", 100, 400, "text/plain")));
    assert!(!f.matches(&make_file("new.txt", "txt", 100, 600, "text/plain")));
}

#[test]
fn test_filter_mime_type_prefix() {
    let f = SearchFilter {
        mime_type: "image/".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file("photo.png", "png", 100, 1000, "image/png")));
    assert!(f.matches(&make_file("photo.jpg", "jpg", 100, 1000, "image/jpeg")));
    assert!(!f.matches(&make_file("doc.txt", "txt", 100, 1000, "text/plain")));
}

#[test]
fn test_filter_combined() {
    let f = SearchFilter {
        extension_filter: "rs".into(),
        min_size: 100,
        mime_type: "text/".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file("lib.rs", "rs", 500, 1000, "text/x-rust")));
    assert!(!f.matches(&make_file("lib.rs", "rs", 50, 1000, "text/x-rust"))); // too small
    assert!(!f.matches(&make_file("lib.py", "py", 500, 1000, "text/x-python"))); // wrong ext
}

#[test]
fn test_filter_is_active() {
    let mut f = SearchFilter::new();
    assert!(!f.is_active());
    f.name_pattern = "*.rs".into();
    assert!(f.is_active());
}

// -----------------------------------------------------------------------
// SearchFolder tests
// -----------------------------------------------------------------------

#[test]
fn test_search_folder_matches_query() {
    let sf = SearchFolder::new("Test", "main", "", SearchFilter::new());
    assert!(sf.matches(&make_file("main.rs", "rs", 100, 1000, "text/x-rust")));
    assert!(!sf.matches(&make_file("lib.rs", "rs", 100, 1000, "text/x-rust")));
}

#[test]
fn test_search_folder_matches_query_case_insensitive() {
    let sf = SearchFolder::new("Test", "MAIN", "", SearchFilter::new());
    assert!(sf.matches(&make_file("main.rs", "rs", 100, 1000, "text/x-rust")));
}

#[test]
fn test_search_folder_matches_both_query_and_filter() {
    let filter = SearchFilter {
        extension_filter: "rs".into(),
        ..Default::default()
    };
    let sf = SearchFolder::new("Rust files with main", "main", "", filter);
    assert!(sf.matches(&make_file("main.rs", "rs", 100, 1000, "text/x-rust")));
    assert!(!sf.matches(&make_file("main.py", "py", 100, 1000, "text/x-python"))); // wrong ext
    assert!(!sf.matches(&make_file("lib.rs", "rs", 100, 1000, "text/x-rust"))); // wrong name
}

// -----------------------------------------------------------------------
// SearchFolderStore tests
// -----------------------------------------------------------------------

#[test]
fn test_store_new_is_empty() {
    let store = SearchFolderStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_save_and_load() {
    let mut store = SearchFolderStore::new();
    store.save(SearchFolder::new(
        "Images",
        "",
        "",
        SearchFilter {
            mime_type: "image/".into(),
            ..Default::default()
        },
    ));
    assert_eq!(store.len(), 1);
    let loaded = store.load("Images").unwrap();
    assert_eq!(loaded.filters.mime_type, "image/");
}

#[test]
fn test_store_save_update() {
    let mut store = SearchFolderStore::new();
    store.save(SearchFolder::new(
        "Test",
        "old query",
        "",
        SearchFilter::new(),
    ));
    store.save(SearchFolder::new(
        "Test",
        "new query",
        "",
        SearchFilter::new(),
    ));
    assert_eq!(store.len(), 1);
    assert_eq!(store.load("Test").unwrap().query, "new query");
}

#[test]
fn test_store_delete() {
    let mut store = SearchFolderStore::new();
    store.save(SearchFolder::new("A", "", "", SearchFilter::new()));
    assert!(store.delete("A"));
    assert!(store.is_empty());
    assert!(!store.delete("A")); // already gone
}

#[test]
fn test_store_list() {
    let mut store = SearchFolderStore::new();
    store.save(SearchFolder::new("A", "", "", SearchFilter::new()));
    store.save(SearchFolder::new("B", "", "", SearchFilter::new()));
    assert_eq!(store.list().len(), 2);
}

// -----------------------------------------------------------------------
// Smart folders
// -----------------------------------------------------------------------

#[test]
fn test_smart_folders_count() {
    let folders = smart_folders();
    assert_eq!(folders.len(), 4);
    let names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"Large Files"));
    assert!(names.contains(&"Recent Documents"));
    assert!(names.contains(&"Images"));
    assert!(names.contains(&"Videos"));
}

#[test]
fn test_smart_folder_large_files() {
    let folders = smart_folders();
    let large = folders.iter().find(|f| f.name == "Large Files").unwrap();
    let big = make_file(
        "huge.bin",
        "bin",
        200 * 1024 * 1024,
        1000,
        "application/octet-stream",
    );
    let small = make_file("tiny.txt", "txt", 100, 1000, "text/plain");
    assert!(large.matches(&big));
    assert!(!large.matches(&small));
}

#[test]
fn test_smart_folder_images() {
    let folders = smart_folders();
    let images = folders.iter().find(|f| f.name == "Images").unwrap();
    assert!(images.matches(&make_file("photo.png", "png", 100, 1000, "image/png")));
    assert!(!images.matches(&make_file("song.mp3", "mp3", 100, 1000, "audio/mpeg")));
}

#[test]
fn test_smart_folder_videos() {
    let folders = smart_folders();
    let videos = folders.iter().find(|f| f.name == "Videos").unwrap();
    assert!(videos.matches(&make_file("clip.mp4", "mp4", 100, 1000, "video/mp4")));
    assert!(!videos.matches(&make_file("photo.png", "png", 100, 1000, "image/png")));
}

#[test]
fn test_set_recent_window() {
    let mut folders = smart_folders();
    let recent = folders
        .iter_mut()
        .find(|f| f.name == "Recent Documents")
        .unwrap();
    let now = 1_000_000u64;
    set_recent_window(recent, now);
    assert_eq!(recent.filters.modified_after, now - 7 * 86_400);
    // A file modified 3 days ago should match.
    let recent_file = make_file("doc.txt", "txt", 100, now - 3 * 86_400, "text/plain");
    assert!(recent.matches(&recent_file));
    // A file modified 10 days ago should not.
    let old_file = make_file("old.txt", "txt", 100, now - 10 * 86_400, "text/plain");
    assert!(!recent.matches(&old_file));
}

#[test]
fn test_glob_match_wildcard() {
    let f = SearchFilter {
        name_pattern: "*".into(),
        ..Default::default()
    };
    assert!(f.matches(&make_file("anything.txt", "txt", 100, 1000, "text/plain")));
}
