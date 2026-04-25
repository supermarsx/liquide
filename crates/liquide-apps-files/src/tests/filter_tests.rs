//! Tests for FileFilter.

use crate::entry::FileEntry;
use crate::filter::FileFilter;

fn sample_entries() -> Vec<FileEntry> {
    vec![
        FileEntry::directory("docs".into(), "/docs".into(), 1000),
        FileEntry::directory(".hidden_dir".into(), "/.hidden_dir".into(), 900),
        FileEntry::file("readme.md".into(), "/readme.md".into(), 2048, 1100),
        FileEntry::file("main.rs".into(), "/main.rs".into(), 512, 1200),
        FileEntry::file(".gitignore".into(), "/.gitignore".into(), 128, 800),
        FileEntry::file("photo.png".into(), "/photo.png".into(), 65536, 1300),
        FileEntry::file("song.mp3".into(), "/song.mp3".into(), 4096000, 1400),
    ]
}

#[test]
fn test_filter_default_hides_hidden() {
    let f = FileFilter::new();
    let entries = sample_entries();
    let visible = f.apply(&entries);
    // 2 hidden entries should be filtered out.
    assert_eq!(visible.len(), 5);
}

#[test]
fn test_filter_show_hidden() {
    let mut f = FileFilter::new();
    f.show_hidden = true;
    let entries = sample_entries();
    let visible = f.apply(&entries);
    assert_eq!(visible.len(), 7);
}

#[test]
fn test_filter_text_search() {
    let f = FileFilter::with_text("main");
    let entries = sample_entries();
    let visible = f.apply(&entries);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].name, "main.rs");
}

#[test]
fn test_filter_text_case_insensitive() {
    let f = FileFilter::with_text("README");
    let entries = sample_entries();
    let visible = f.apply(&entries);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].name, "readme.md");
}

#[test]
fn test_filter_by_type() {
    let mut f = FileFilter::new();
    f.show_hidden = true;
    f.add_type("rs");
    let entries = sample_entries();
    let visible = f.apply(&entries);
    // Should include: main.rs + all directories (dirs pass type filter)
    let file_names: Vec<&str> = visible
        .iter()
        .filter(|e| !e.is_dir())
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(file_names, vec!["main.rs"]);
}

#[test]
fn test_filter_by_multiple_types() {
    let mut f = FileFilter::new();
    f.show_hidden = true;
    f.add_type("rs");
    f.add_type("md");
    let entries = sample_entries();
    let visible = f.apply(&entries);
    let file_names: Vec<&str> = visible
        .iter()
        .filter(|e| !e.is_dir())
        .map(|e| e.name.as_str())
        .collect();
    assert!(file_names.contains(&"main.rs"));
    assert!(file_names.contains(&"readme.md"));
}

#[test]
fn test_filter_remove_type() {
    let mut f = FileFilter::new();
    f.add_type("rs");
    f.add_type("md");
    f.remove_type("md");
    assert_eq!(f.file_types.len(), 1);
    assert_eq!(f.file_types[0], "rs");
}

#[test]
fn test_filter_is_active() {
    let mut f = FileFilter::new();
    assert!(!f.is_active());
    f.set_text("test");
    assert!(f.is_active());
    f.reset();
    assert!(!f.is_active());
    f.add_type("rs");
    assert!(f.is_active());
}

#[test]
fn test_filter_toggle_hidden() {
    let mut f = FileFilter::new();
    assert!(!f.show_hidden);
    f.toggle_hidden();
    assert!(f.show_hidden);
    f.toggle_hidden();
    assert!(!f.show_hidden);
}

#[test]
fn test_filter_reset() {
    let mut f = FileFilter::new();
    f.set_text("query");
    f.show_hidden = true;
    f.add_type("rs");
    f.reset();
    assert!(f.text.is_empty());
    assert!(!f.show_hidden);
    assert!(f.file_types.is_empty());
}

#[test]
fn test_filter_directories_always_pass_type_filter() {
    let mut f = FileFilter::new();
    f.show_hidden = true;
    f.add_type("rs");
    let entries = sample_entries();
    let visible = f.apply(&entries);
    let dirs: Vec<&str> = visible
        .iter()
        .filter(|e| e.is_dir())
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(dirs.len(), 2); // both dirs should pass.
}
