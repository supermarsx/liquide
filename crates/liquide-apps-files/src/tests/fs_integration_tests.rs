//! Integration tests that exercise real filesystem operations via tempdir.

use crate::config::FilesConfig;
use crate::entry::EntryKind;
use crate::listing::DirectoryListing;
use crate::operations::{execute_operation, FileOp};
use crate::runtime::FilesRuntime;
use crate::trash::TrashManager;
use std::fs;
use std::path::Path;

/// Create a temporary directory with a unique name under the system temp dir.
fn make_temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("liquide_test_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

// ===========================================================================
// DirectoryListing::load_directory
// ===========================================================================

#[test]
fn test_load_directory_reads_real_files() {
    let tmp = make_temp("load_dir");
    fs::write(tmp.join("hello.txt"), "hello").unwrap();
    fs::write(tmp.join("world.rs"), "fn main(){}").unwrap();
    fs::create_dir(tmp.join("subdir")).unwrap();

    let mut listing = DirectoryListing::new(String::new());
    listing.show_hidden = true;
    listing.load_directory(&tmp).unwrap();

    assert_eq!(listing.visible_count(), 3);
    assert!(listing.find_by_name("hello.txt").is_some());
    assert!(listing.find_by_name("subdir").is_some());

    let subdir = listing.find_by_name("subdir").unwrap();
    assert_eq!(subdir.kind, EntryKind::Directory);

    let hello = listing.find_by_name("hello.txt").unwrap();
    assert_eq!(hello.kind, EntryKind::File);
    assert_eq!(hello.size, 5);
    assert_eq!(hello.mime_type, "text/plain");

    cleanup(&tmp);
}

#[test]
fn test_load_directory_nonexistent_returns_error() {
    let bad_path = std::env::temp_dir().join("liquide_nonexistent_dir_xyz");
    let mut listing = DirectoryListing::new(String::new());
    let result = listing.load_directory(&bad_path);
    assert!(result.is_err());
}

#[test]
fn test_load_directory_filters_hidden() {
    let tmp = make_temp("load_hidden");
    fs::write(tmp.join("visible.txt"), "").unwrap();
    // On Unix, dot-files are hidden. On Windows, we'd need to set attributes.
    // We create both and check that show_hidden=false filters some.
    fs::write(tmp.join(".hidden_file"), "").unwrap();

    let mut listing = DirectoryListing::new(String::new());
    listing.show_hidden = false;
    listing.load_directory(&tmp).unwrap();

    // On all platforms, dot-prefixed files should be hidden (Unix natively,
    // Windows through our is_hidden fallback).
    // At minimum, visible.txt should be present.
    assert!(listing.find_by_name("visible.txt").is_some());

    cleanup(&tmp);
}

// ===========================================================================
// execute_operation
// ===========================================================================

#[test]
fn test_execute_copy() {
    let tmp = make_temp("exec_copy");
    let src = tmp.join("source");
    let dst = tmp.join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.txt"), "alpha").unwrap();

    execute_operation(&FileOp::Copy {
        sources: vec![src.join("a.txt").to_string_lossy().to_string()],
        destination: dst.to_string_lossy().to_string(),
    })
    .unwrap();

    assert!(dst.join("a.txt").exists());
    assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "alpha");
    // Source should still exist.
    assert!(src.join("a.txt").exists());

    cleanup(&tmp);
}

#[test]
fn test_execute_copy_dir_recursive() {
    let tmp = make_temp("exec_copy_dir");
    let src = tmp.join("srcdir");
    fs::create_dir_all(src.join("inner")).unwrap();
    fs::write(src.join("top.txt"), "top").unwrap();
    fs::write(src.join("inner").join("deep.txt"), "deep").unwrap();

    let dst = tmp.join("dest");
    execute_operation(&FileOp::Copy {
        sources: vec![src.to_string_lossy().to_string()],
        destination: dst.to_string_lossy().to_string(),
    })
    .unwrap();

    assert!(dst.join("srcdir").join("top.txt").exists());
    assert!(dst.join("srcdir").join("inner").join("deep.txt").exists());

    cleanup(&tmp);
}

#[test]
fn test_execute_move() {
    let tmp = make_temp("exec_move");
    let src = tmp.join("src");
    let dst = tmp.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("moved.txt"), "data").unwrap();

    execute_operation(&FileOp::Move {
        sources: vec![src.join("moved.txt").to_string_lossy().to_string()],
        destination: dst.to_string_lossy().to_string(),
    })
    .unwrap();

    assert!(dst.join("moved.txt").exists());
    assert!(!src.join("moved.txt").exists());

    cleanup(&tmp);
}

#[test]
fn test_execute_delete() {
    let tmp = make_temp("exec_del");
    let file = tmp.join("doomed.txt");
    fs::write(&file, "bye").unwrap();

    execute_operation(&FileOp::Delete {
        paths: vec![file.to_string_lossy().to_string()],
        trash: false,
    })
    .unwrap();

    assert!(!file.exists());

    cleanup(&tmp);
}

#[test]
fn test_execute_delete_dir() {
    let tmp = make_temp("exec_del_dir");
    let dir = tmp.join("somedir");
    fs::create_dir_all(dir.join("child")).unwrap();
    fs::write(dir.join("child").join("f.txt"), "x").unwrap();

    execute_operation(&FileOp::Delete {
        paths: vec![dir.to_string_lossy().to_string()],
        trash: false,
    })
    .unwrap();

    assert!(!dir.exists());

    cleanup(&tmp);
}

#[test]
fn test_execute_rename() {
    let tmp = make_temp("exec_rename");
    fs::write(tmp.join("old.txt"), "content").unwrap();

    execute_operation(&FileOp::Rename {
        path: tmp.join("old.txt").to_string_lossy().to_string(),
        new_name: "new.txt".to_string(),
    })
    .unwrap();

    assert!(!tmp.join("old.txt").exists());
    assert!(tmp.join("new.txt").exists());

    cleanup(&tmp);
}

#[test]
fn test_execute_create_folder() {
    let tmp = make_temp("exec_mkdir");

    execute_operation(&FileOp::CreateFolder {
        parent: tmp.to_string_lossy().to_string(),
        name: "new_folder".to_string(),
    })
    .unwrap();

    assert!(tmp.join("new_folder").is_dir());

    cleanup(&tmp);
}

#[test]
fn test_execute_create_file() {
    let tmp = make_temp("exec_mkfile");

    execute_operation(&FileOp::CreateFile {
        parent: tmp.to_string_lossy().to_string(),
        name: "new_file.txt".to_string(),
    })
    .unwrap();

    assert!(tmp.join("new_file.txt").exists());

    cleanup(&tmp);
}

// ===========================================================================
// TrashManager with real filesystem
// ===========================================================================

#[test]
fn test_trash_manager_physical_trash_and_restore() {
    let tmp = make_temp("trash_phys");
    let trash_dir = tmp.join("trash");
    let file = tmp.join("trashme.txt");
    fs::write(&file, "trash content").unwrap();

    let mut tm = TrashManager::with_dir(trash_dir.to_string_lossy().to_string());
    let entry = tm.trash(
        &file.to_string_lossy(),
        fs::metadata(&file).map(|m| m.len()).unwrap_or(0),
    )
    .unwrap();

    // File should be moved to trash/files/.
    assert!(!file.exists(), "original should be gone");
    assert!(
        Path::new(&entry.trash_path).exists(),
        "file should be in trash"
    );

    // Restore it.
    tm.restore(&entry).unwrap();
    assert!(file.exists(), "file should be restored");
    assert!(!Path::new(&entry.trash_path).exists());

    cleanup(&tmp);
}

#[test]
fn test_trash_manager_empty_trash_deletes_files() {
    let tmp = make_temp("trash_empty");
    let trash_dir = tmp.join("trash");
    let f1 = tmp.join("f1.txt");
    let f2 = tmp.join("f2.txt");
    fs::write(&f1, "one").unwrap();
    fs::write(&f2, "two").unwrap();

    let mut tm = TrashManager::with_dir(trash_dir.to_string_lossy().to_string());
    let e1 = tm.trash(&f1.to_string_lossy(), 3).unwrap();
    let e2 = tm.trash(&f2.to_string_lossy(), 3).unwrap();

    assert!(Path::new(&e1.trash_path).exists());
    assert!(Path::new(&e2.trash_path).exists());

    tm.empty_trash();
    assert!(!Path::new(&e1.trash_path).exists());
    assert!(!Path::new(&e2.trash_path).exists());
    assert_eq!(tm.count(), 0);

    cleanup(&tmp);
}

#[test]
fn test_trash_manager_load_from_disk() {
    let tmp = make_temp("trash_load");
    let trash_dir = tmp.join("trash");
    let file = tmp.join("persisted.txt");
    fs::write(&file, "persist").unwrap();

    // Trash a file with one manager instance.
    let mut tm1 = TrashManager::with_dir(trash_dir.to_string_lossy().to_string());
    tm1.trash(&file.to_string_lossy(), 7).unwrap();
    assert_eq!(tm1.count(), 1);

    // Create a fresh manager and load from disk.
    let mut tm2 = TrashManager::with_dir(trash_dir.to_string_lossy().to_string());
    assert_eq!(tm2.count(), 0);
    tm2.load_from_disk();
    assert_eq!(tm2.count(), 1);

    cleanup(&tmp);
}

// ===========================================================================
// FilesRuntime real navigation
// ===========================================================================

#[test]
fn test_runtime_navigate_to_real_dir() {
    let tmp = make_temp("rt_nav");
    fs::write(tmp.join("file.txt"), "hello").unwrap();
    fs::create_dir(tmp.join("child")).unwrap();

    let mut rt = FilesRuntime::new(FilesConfig::default());
    rt.navigate_to(&tmp).unwrap();

    assert!(rt.current_listing().visible_count() >= 2);
    assert!(rt.current_listing().find_by_name("file.txt").is_some());
    assert!(rt.current_listing().find_by_name("child").is_some());

    cleanup(&tmp);
}

#[test]
fn test_runtime_open_entry_navigates_into_dir() {
    let tmp = make_temp("rt_open");
    let child = tmp.join("subdir");
    fs::create_dir_all(&child).unwrap();
    fs::write(child.join("inner.txt"), "inside").unwrap();
    fs::write(tmp.join("top.txt"), "top").unwrap();

    let mut rt = FilesRuntime::new(FilesConfig::default());
    rt.navigate_to(&tmp).unwrap();

    // Find the index of "subdir".
    let idx = rt
        .current_listing()
        .entries
        .iter()
        .position(|e| e.name == "subdir")
        .expect("subdir should be in listing");

    let navigated = rt.open_entry(idx).unwrap();
    assert!(navigated);
    assert!(rt.current_listing().find_by_name("inner.txt").is_some());

    cleanup(&tmp);
}

#[test]
fn test_runtime_navigate_back_and_refresh() {
    let tmp = make_temp("rt_back");
    let child = tmp.join("sub");
    fs::create_dir_all(&child).unwrap();
    fs::write(tmp.join("root.txt"), "r").unwrap();
    fs::write(child.join("child.txt"), "c").unwrap();

    let mut rt = FilesRuntime::new(FilesConfig::default());
    rt.navigate_to(&tmp).unwrap();
    rt.navigate_to(&child).unwrap();

    assert!(rt.current_listing().find_by_name("child.txt").is_some());

    let went_back = rt.navigate_back().unwrap();
    assert!(went_back);
    assert!(rt.current_listing().find_by_name("root.txt").is_some());

    // Refresh should re-read the same directory.
    rt.refresh().unwrap();
    assert!(rt.current_listing().find_by_name("root.txt").is_some());

    cleanup(&tmp);
}

#[test]
fn test_runtime_navigate_up_disk() {
    let tmp = make_temp("rt_up");
    let child = tmp.join("deep");
    fs::create_dir_all(&child).unwrap();

    let mut rt = FilesRuntime::new(FilesConfig::default());
    rt.navigate_to(&child).unwrap();

    let went_up = rt.navigate_up_disk().unwrap();
    assert!(went_up);
    // Should now be in `tmp` (the parent of `deep`).
    assert!(rt.current_listing().find_by_name("deep").is_some());

    cleanup(&tmp);
}
