//! Tests for the files runtime coordinator.

use crate::config::FilesConfig;
use crate::entry::FileEntry;
use crate::runtime::FilesRuntime;

fn make_runtime() -> FilesRuntime {
    FilesRuntime::new(FilesConfig::default())
}

fn sample_entries() -> Vec<FileEntry> {
    vec![
        FileEntry::directory("docs".into(), "/home/user/docs".into(), 1000),
        FileEntry::file("readme.md".into(), "/home/user/readme.md".into(), 2048, 1100),
        FileEntry::file("main.rs".into(), "/home/user/main.rs".into(), 512, 1200),
    ]
}

#[test]
fn test_runtime_new() {
    let rt = make_runtime();
    assert_eq!(rt.current_listing().path, "~");
}

#[test]
fn test_runtime_navigate() {
    let mut rt = make_runtime();
    rt.navigate("/home/user".into(), sample_entries());
    assert_eq!(rt.current_listing().path, "/home/user");
    assert_eq!(rt.current_listing().visible_count(), 3);
}

#[test]
fn test_runtime_navigation_history() {
    let mut rt = make_runtime();
    rt.navigate("/a".into(), vec![]);
    rt.navigate("/b".into(), vec![]);
    rt.navigate("/c".into(), vec![]);
    assert!(rt.can_go_back());
    let path = rt.go_back().unwrap().to_string();
    assert_eq!(path, "/b");
    assert!(rt.can_go_forward());
    let path = rt.go_forward().unwrap().to_string();
    assert_eq!(path, "/c");
}

#[test]
fn test_runtime_cannot_go_back_at_start() {
    let rt = make_runtime();
    assert!(!rt.can_go_back());
}

#[test]
fn test_runtime_selection() {
    let mut rt = make_runtime();
    rt.navigate("/home".into(), sample_entries());
    rt.set_selection(vec![0, 2]);
    assert_eq!(rt.selection().len(), 2);
    let selected = rt.selected_entries();
    assert_eq!(selected.len(), 2);
}

#[test]
fn test_runtime_select_all() {
    let mut rt = make_runtime();
    rt.navigate("/home".into(), sample_entries());
    rt.select_all();
    assert_eq!(rt.selection().len(), 3);
}

#[test]
fn test_runtime_clear_selection() {
    let mut rt = make_runtime();
    rt.navigate("/home".into(), sample_entries());
    rt.select_all();
    rt.clear_selection();
    assert!(rt.selection().is_empty());
}

#[test]
fn test_runtime_sidebar_bookmark() {
    let mut rt = make_runtime();
    rt.sidebar_mut().add_bookmark("Projects".into(), "~/Projects".into());
    assert!(rt.sidebar().find("Projects").is_some());
}

#[test]
fn test_runtime_clipboard() {
    let mut rt = make_runtime();
    rt.navigate("/home".into(), sample_entries());
    rt.set_selection(vec![1]);
    let entries: Vec<_> = rt.selected_entries().into_iter().cloned().collect();
    rt.clipboard_mut().copy(entries);
    assert!(rt.clipboard().has_entries());
}

#[test]
fn test_runtime_navigate_clears_selection() {
    let mut rt = make_runtime();
    rt.navigate("/a".into(), sample_entries());
    rt.select_all();
    rt.navigate("/b".into(), vec![]);
    assert!(rt.selection().is_empty());
}

#[test]
fn test_runtime_navigate_up() {
    let mut rt = make_runtime();
    rt.navigate("/home/user/docs".into(), vec![]);
    let parent = rt.navigate_up();
    assert_eq!(parent, Some("/home/user".to_string()));
}
