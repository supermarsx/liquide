//! Tests for `files` module types.

use liquide_apps_task_manager::files::*;

// ---------------------------------------------------------------------------
// OpenFileType
// ---------------------------------------------------------------------------

#[test]
fn open_file_type_all_variants() {
    let variants = [
        OpenFileType::RegularFile,
        OpenFileType::Directory,
        OpenFileType::NamedPipe,
        OpenFileType::Socket,
        OpenFileType::Device,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn open_file_type_serde_roundtrip() {
    let val = OpenFileType::NamedPipe;
    let json = serde_json::to_string(&val).unwrap();
    let back: OpenFileType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// FileAccessType
// ---------------------------------------------------------------------------

#[test]
fn file_access_type_all_variants() {
    let variants = [
        FileAccessType::Read,
        FileAccessType::Write,
        FileAccessType::ReadWrite,
        FileAccessType::Execute,
        FileAccessType::Delete,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// LockType
// ---------------------------------------------------------------------------

#[test]
fn lock_type_all_variants() {
    let variants = [LockType::None, LockType::Shared, LockType::Exclusive];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// ShareMode
// ---------------------------------------------------------------------------

#[test]
fn share_mode_all_variants() {
    let variants = [
        ShareMode::None,
        ShareMode::Read,
        ShareMode::Write,
        ShareMode::ReadWrite,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn share_mode_serde_roundtrip() {
    let val = ShareMode::Write;
    let json = serde_json::to_string(&val).unwrap();
    let back: ShareMode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// FileGroupBy
// ---------------------------------------------------------------------------

#[test]
fn file_group_by_all_variants() {
    let variants = [
        FileGroupBy::Process,
        FileGroupBy::FileType,
        FileGroupBy::LockType,
        FileGroupBy::Directory,
    ];
    assert_eq!(variants.len(), 4);
}

// ---------------------------------------------------------------------------
// OpenFileInfo construction
// ---------------------------------------------------------------------------

#[test]
fn open_file_info_construction() {
    let info = OpenFileInfo {
        path: "/var/log/syslog".into(),
        file_type: OpenFileType::RegularFile,
        access: FileAccessType::ReadWrite,
        lock_type: LockType::Exclusive,
        share_mode: ShareMode::None,
        pid: 1234,
        process_name: "rsyslog".into(),
        handle_value: 42,
        size_bytes: Some(1024 * 1024),
        offset: None,
        open_since: Some("2026-02-12T10:00:00Z".into()),
        delete_pending: false,
        is_directory: false,
    };
    assert_eq!(info.path, "/var/log/syslog");
    assert_eq!(info.file_type, OpenFileType::RegularFile);
    assert_eq!(info.lock_type, LockType::Exclusive);
}

#[test]
fn open_file_info_serde_roundtrip() {
    let info = OpenFileInfo {
        path: "/tmp/test.txt".into(),
        file_type: OpenFileType::RegularFile,
        access: FileAccessType::Read,
        lock_type: LockType::None,
        share_mode: ShareMode::Read,
        pid: 100,
        process_name: "test".into(),
        handle_value: 1,
        size_bytes: None,
        offset: None,
        open_since: None,
        delete_pending: false,
        is_directory: false,
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: OpenFileInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.path, "/tmp/test.txt");
    assert_eq!(back.pid, 100);
}
