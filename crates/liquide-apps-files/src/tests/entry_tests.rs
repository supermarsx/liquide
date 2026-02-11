//! Tests for file entry types.

use crate::entry::{EntryKind, FileEntry, Permissions, guess_mime};

#[test]
fn test_file_entry() {
    let e = FileEntry::file("readme.md".into(), "/home/user/readme.md".into(), 1024, 1000);
    assert_eq!(e.kind, EntryKind::File);
    assert_eq!(e.extension, "md");
    assert!(!e.hidden);
    assert_eq!(e.mime_type, "text/plain");
}

#[test]
fn test_directory_entry() {
    let e = FileEntry::directory("docs".into(), "/home/user/docs".into(), 1000);
    assert_eq!(e.kind, EntryKind::Directory);
    assert!(e.is_dir());
    assert!(e.extension.is_empty());
}

#[test]
fn test_hidden_file() {
    let e = FileEntry::file(".gitignore".into(), "/home/.gitignore".into(), 100, 1000);
    assert!(e.hidden);
}

#[test]
fn test_hidden_directory() {
    let e = FileEntry::directory(".config".into(), "/home/.config".into(), 1000);
    assert!(e.hidden);
}

#[test]
fn test_human_size_bytes() {
    let e = FileEntry::file("a".into(), "a".into(), 500, 0);
    assert_eq!(e.human_size(), "500 B");
}

#[test]
fn test_human_size_kb() {
    let e = FileEntry::file("a".into(), "a".into(), 2048, 0);
    assert_eq!(e.human_size(), "2.0 KB");
}

#[test]
fn test_human_size_mb() {
    let e = FileEntry::file("a".into(), "a".into(), 5 * 1024 * 1024, 0);
    assert_eq!(e.human_size(), "5.0 MB");
}

#[test]
fn test_human_size_gb() {
    let e = FileEntry::file("a".into(), "a".into(), 3 * 1024 * 1024 * 1024, 0);
    assert_eq!(e.human_size(), "3.0 GB");
}

#[test]
fn test_human_size_directory() {
    let e = FileEntry::directory("d".into(), "d".into(), 0);
    assert_eq!(e.human_size(), "--");
}

#[test]
fn test_permissions_from_mode() {
    let p = Permissions::from_mode(0o755);
    assert!(p.readable);
    assert!(p.writable);
    assert!(p.executable);
}

#[test]
fn test_permissions_read_only() {
    let p = Permissions::from_mode(0o444);
    assert!(p.readable);
    assert!(!p.writable);
    assert!(!p.executable);
}

#[test]
fn test_guess_mime_text() {
    assert_eq!(guess_mime("txt"), "text/plain");
    assert_eq!(guess_mime("md"), "text/plain");
}

#[test]
fn test_guess_mime_source() {
    assert_eq!(guess_mime("rs"), "text/x-source");
    assert_eq!(guess_mime("py"), "text/x-source");
}

#[test]
fn test_guess_mime_image() {
    assert_eq!(guess_mime("png"), "image/png");
    assert_eq!(guess_mime("jpg"), "image/jpeg");
}

#[test]
fn test_guess_mime_unknown() {
    assert_eq!(guess_mime("xyz"), "application/x-xyz");
}

#[test]
fn test_guess_mime_empty() {
    assert_eq!(guess_mime(""), "application/octet-stream");
}

#[test]
fn test_entry_kind_display() {
    assert_eq!(EntryKind::File.to_string(), "file");
    assert_eq!(EntryKind::Directory.to_string(), "directory");
    assert_eq!(EntryKind::Symlink.to_string(), "symlink");
}
