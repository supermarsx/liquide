//! Tests for directory listing and sort.

use crate::config::SortField;
use crate::entry::FileEntry;
use crate::listing::DirectoryListing;
use crate::sort::{sort_entries, sort_natural};
use crate::sidebar::Sidebar;
use crate::clipboard::{FileClipboard, ClipboardOp};
use crate::search::FileSearch;
use crate::operations::{FileOperation, OperationKind, OperationState};
use crate::preview::{Preview, is_text_mime};

fn sample_entries() -> Vec<FileEntry> {
    vec![
        FileEntry::directory("docs".into(), "/docs".into(), 1000),
        FileEntry::directory(".hidden".into(), "/.hidden".into(), 900),
        FileEntry::file("readme.md".into(), "/readme.md".into(), 2048, 1100),
        FileEntry::file("main.rs".into(), "/main.rs".into(), 512, 1200),
        FileEntry::file(".gitignore".into(), "/.gitignore".into(), 128, 800),
    ]
}

// ===========================================================================
// DirectoryListing
// ===========================================================================

#[test]
fn test_listing_new() {
    let listing = DirectoryListing::new("/home".into());
    assert_eq!(listing.path, "/home");
    assert!(listing.entries.is_empty());
}

#[test]
fn test_listing_set_entries_hides_hidden() {
    let mut listing = DirectoryListing::new("/".into());
    listing.show_hidden = false;
    listing.set_entries(sample_entries());
    assert_eq!(listing.total_count, 5);
    assert_eq!(listing.visible_count(), 3); // 2 hidden files filtered
}

#[test]
fn test_listing_show_hidden() {
    let mut listing = DirectoryListing::new("/".into());
    listing.show_hidden = true;
    listing.set_entries(sample_entries());
    assert_eq!(listing.visible_count(), 5);
}

#[test]
fn test_listing_dirs_before_files() {
    let mut listing = DirectoryListing::new("/".into());
    listing.show_hidden = true;
    listing.set_entries(sample_entries());
    // First entries should be directories.
    assert!(listing.get(0).unwrap().is_dir());
    assert!(listing.get(1).unwrap().is_dir());
}

#[test]
fn test_listing_sort_by_size() {
    let mut listing = DirectoryListing::new("/".into());
    listing.show_hidden = true;
    listing.set_entries(sample_entries());
    listing.set_sort(SortField::Size, true);
    // After dirs, files should be sorted by size ascending.
    let files: Vec<_> = listing.entries.iter().filter(|e| !e.is_dir()).collect();
    assert!(files[0].size <= files[1].size);
}

#[test]
fn test_listing_find_by_name() {
    let mut listing = DirectoryListing::new("/".into());
    listing.show_hidden = true;
    listing.set_entries(sample_entries());
    let found = listing.find_by_name("main.rs");
    assert!(found.is_some());
    assert!(listing.find_by_name("nonexistent").is_none());
}

#[test]
fn test_listing_counts() {
    let mut listing = DirectoryListing::new("/".into());
    listing.show_hidden = true;
    listing.set_entries(sample_entries());
    assert_eq!(listing.dir_count(), 2);
    assert_eq!(listing.file_count(), 3);
}

#[test]
fn test_listing_parent() {
    let listing = DirectoryListing::new("/home/user/docs".into());
    assert_eq!(listing.parent(), Some("/home/user".to_string()));
}

#[test]
fn test_listing_parent_root() {
    let listing = DirectoryListing::new("/".into());
    assert!(listing.parent().is_none());
}

// ===========================================================================
// Sort
// ===========================================================================

#[test]
fn test_sort_by_name() {
    let mut entries = sample_entries();
    sort_entries(&mut entries, SortField::Name, true);
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    // Dirs first, then files, both alphabetical.
    assert_eq!(names[0], ".hidden");
    assert_eq!(names[1], "docs");
}

#[test]
fn test_sort_descending() {
    let mut entries = vec![
        FileEntry::file("a.txt".into(), "a".into(), 100, 0),
        FileEntry::file("z.txt".into(), "z".into(), 200, 0),
    ];
    sort_entries(&mut entries, SortField::Name, false);
    assert_eq!(entries[0].name, "z.txt");
}

#[test]
fn test_natural_sort() {
    let mut entries = vec![
        FileEntry::file("file10.txt".into(), "a".into(), 0, 0),
        FileEntry::file("file2.txt".into(), "b".into(), 0, 0),
        FileEntry::file("file1.txt".into(), "c".into(), 0, 0),
    ];
    sort_natural(&mut entries, true);
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["file1.txt", "file2.txt", "file10.txt"]);
}

// ===========================================================================
// Sidebar
// ===========================================================================

#[test]
fn test_sidebar_defaults() {
    let sb = Sidebar::new();
    assert!(sb.bookmarks().len() >= 7);
    assert!(sb.find("Home").is_some());
}

#[test]
fn test_sidebar_add_bookmark() {
    let mut sb = Sidebar::new();
    let before = sb.bookmarks().len();
    sb.add_bookmark("Projects".into(), "~/Projects".into());
    assert_eq!(sb.bookmarks().len(), before + 1);
    assert!(sb.find("Projects").is_some());
}

#[test]
fn test_sidebar_remove_bookmark() {
    let mut sb = Sidebar::new();
    sb.add_bookmark("Custom".into(), "/custom".into());
    sb.remove_bookmark("/custom").unwrap();
    assert!(sb.find("Custom").is_none());
}

#[test]
fn test_sidebar_cannot_remove_system() {
    let mut sb = Sidebar::new();
    // System bookmarks should not be removed.
    sb.remove_bookmark("~").ok();
    assert!(sb.find("Home").is_some());
}

#[test]
fn test_sidebar_no_duplicate() {
    let mut sb = Sidebar::new();
    sb.add_bookmark("A".into(), "/a".into());
    sb.add_bookmark("B".into(), "/a".into());
    let count = sb.bookmarks().iter().filter(|b| b.path == "/a").count();
    assert_eq!(count, 1);
}

// ===========================================================================
// Clipboard
// ===========================================================================

#[test]
fn test_clipboard_empty() {
    let cb = FileClipboard::new();
    assert!(!cb.has_entries());
    assert!(cb.operation().is_none());
}

#[test]
fn test_clipboard_copy() {
    let mut cb = FileClipboard::new();
    cb.copy(vec![FileEntry::file("a".into(), "/a".into(), 100, 0)]);
    assert!(cb.has_entries());
    assert_eq!(cb.operation(), Some(ClipboardOp::Copy));
    assert_eq!(cb.count(), 1);
}

#[test]
fn test_clipboard_cut() {
    let mut cb = FileClipboard::new();
    cb.cut(vec![FileEntry::file("a".into(), "/a".into(), 100, 0)]);
    assert_eq!(cb.operation(), Some(ClipboardOp::Cut));
}

#[test]
fn test_clipboard_take() {
    let mut cb = FileClipboard::new();
    cb.copy(vec![FileEntry::file("a".into(), "/a".into(), 100, 0)]);
    let (entries, op) = cb.take();
    assert_eq!(entries.len(), 1);
    assert_eq!(op, Some(ClipboardOp::Copy));
    assert!(!cb.has_entries());
}

// ===========================================================================
// Search
// ===========================================================================

#[test]
fn test_search_basic() {
    let mut s = FileSearch::new();
    let files = vec![
        ("/a/readme.md".into(), "readme.md".into(), false, 100u64),
        ("/a/main.rs".into(), "main.rs".into(), false, 200),
        ("/a/readme.txt".into(), "readme.txt".into(), false, 50),
    ];
    s.search("readme", &files);
    assert_eq!(s.result_count(), 2);
}

#[test]
fn test_search_case_insensitive() {
    let mut s = FileSearch::new();
    let files = vec![("a".into(), "README.md".into(), false, 100u64)];
    s.search("readme", &files);
    assert_eq!(s.result_count(), 1);
}

#[test]
fn test_search_exact_match_highest_score() {
    let mut s = FileSearch::new();
    let files = vec![
        ("a".into(), "test".into(), false, 100u64),
        ("b".into(), "testing".into(), false, 200),
    ];
    s.search("test", &files);
    assert_eq!(s.results()[0].name, "test");
    assert!(s.results()[0].score > s.results()[1].score);
}

#[test]
fn test_search_clear() {
    let mut s = FileSearch::new();
    let files = vec![("a".into(), "test".into(), false, 100u64)];
    s.search("test", &files);
    s.clear();
    assert_eq!(s.result_count(), 0);
    assert!(s.query().is_empty());
}

// ===========================================================================
// Operations
// ===========================================================================

#[test]
fn test_operation_copy() {
    let op = FileOperation::copy(vec!["/a".into()], "/b".into());
    assert_eq!(op.kind, OperationKind::Copy);
    assert_eq!(op.state, OperationState::Pending);
    assert!(!op.is_done());
}

#[test]
fn test_operation_progress() {
    let mut op = FileOperation::copy(vec!["/a".into()], "/b".into());
    op.update_progress(500, 1000, 1);
    assert!((op.progress - 0.5).abs() < 0.01);
}

#[test]
fn test_operation_complete() {
    let mut op = FileOperation::delete(vec!["/a".into()]);
    op.complete();
    assert!(op.is_done());
    assert_eq!(op.state, OperationState::Completed);
}

#[test]
fn test_operation_kind_display() {
    assert_eq!(OperationKind::Copy.to_string(), "copy");
    assert_eq!(OperationKind::Move.to_string(), "move");
    assert_eq!(OperationKind::Delete.to_string(), "delete");
}

// ===========================================================================
// Preview
// ===========================================================================

#[test]
fn test_preview_text() {
    let p = Preview::text("/a.txt".into(), "line1\nline2\nline3", 2);
    assert!(p.has_content());
    if let crate::preview::PreviewContent::Text { lines, truncated, total_lines } = p.content {
        assert_eq!(lines.len(), 2);
        assert!(truncated);
        assert_eq!(total_lines, 3);
    } else {
        panic!("expected text preview");
    }
}

#[test]
fn test_preview_directory_summary() {
    let p = Preview::directory_summary("/dir".into(), 10, 3, 1024);
    assert!(p.has_content());
}

#[test]
fn test_is_text_mime() {
    assert!(is_text_mime("text/plain"));
    assert!(is_text_mime("text/x-source"));
    assert!(is_text_mime("application/json"));
    assert!(!is_text_mime("image/png"));
}
