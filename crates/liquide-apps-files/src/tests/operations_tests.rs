//! Tests for file operations, FileOp enum, OperationProgress, and ArchiveFormat.

use crate::operations::{
    ArchiveFormat, FileOp, FileOperation, OperationKind, OperationProgress,
};

// ===========================================================================
// ArchiveFormat
// ===========================================================================

#[test]
fn test_archive_format_display() {
    assert_eq!(ArchiveFormat::Zip.to_string(), "zip");
    assert_eq!(ArchiveFormat::TarGz.to_string(), "tar.gz");
    assert_eq!(ArchiveFormat::TarBz2.to_string(), "tar.bz2");
    assert_eq!(ArchiveFormat::TarXz.to_string(), "tar.xz");
    assert_eq!(ArchiveFormat::SevenZip.to_string(), "7z");
}

#[test]
fn test_archive_format_from_extension() {
    assert_eq!(ArchiveFormat::from_extension("data.zip"), Some(ArchiveFormat::Zip));
    assert_eq!(ArchiveFormat::from_extension("data.tar.gz"), Some(ArchiveFormat::TarGz));
    assert_eq!(ArchiveFormat::from_extension("data.tgz"), Some(ArchiveFormat::TarGz));
    assert_eq!(ArchiveFormat::from_extension("data.tar.bz2"), Some(ArchiveFormat::TarBz2));
    assert_eq!(ArchiveFormat::from_extension("data.tar.xz"), Some(ArchiveFormat::TarXz));
    assert_eq!(ArchiveFormat::from_extension("data.7z"), Some(ArchiveFormat::SevenZip));
    assert_eq!(ArchiveFormat::from_extension("data.txt"), None);
}

// ===========================================================================
// FileOp enum
// ===========================================================================

#[test]
fn test_file_op_copy() {
    let op = FileOp::Copy {
        sources: vec!["/a".into(), "/b".into()],
        destination: "/dest".into(),
    };
    let display = op.to_string();
    assert!(display.contains("copy"));
    assert!(display.contains("2 item(s)"));
}

#[test]
fn test_file_op_delete_trash() {
    let op = FileOp::Delete {
        paths: vec!["/a".into()],
        trash: true,
    };
    let display = op.to_string();
    assert!(display.contains("trash"));
}

#[test]
fn test_file_op_delete_permanent() {
    let op = FileOp::Delete {
        paths: vec!["/a".into()],
        trash: false,
    };
    let display = op.to_string();
    assert!(display.contains("delete"));
}

#[test]
fn test_file_op_rename() {
    let op = FileOp::Rename {
        path: "/old.txt".into(),
        new_name: "new.txt".into(),
    };
    let display = op.to_string();
    assert!(display.contains("rename"));
}

#[test]
fn test_file_op_create_folder() {
    let op = FileOp::CreateFolder {
        parent: "/home/user".into(),
        name: "projects".into(),
    };
    let display = op.to_string();
    assert!(display.contains("create folder"));
}

#[test]
fn test_file_op_compress() {
    let op = FileOp::Compress {
        sources: vec!["/a".into()],
        archive_path: "/out.zip".into(),
        format: ArchiveFormat::Zip,
    };
    let display = op.to_string();
    assert!(display.contains("compress"));
    assert!(display.contains("zip"));
}

#[test]
fn test_file_op_extract() {
    let op = FileOp::Extract {
        archive_path: "/data.tar.gz".into(),
        destination: "/out".into(),
    };
    let display = op.to_string();
    assert!(display.contains("extract"));
}

// ===========================================================================
// OperationProgress
// ===========================================================================

#[test]
fn test_progress_percent_by_bytes() {
    let mut p = OperationProgress::new(1000, 10);
    p.completed_bytes = 500;
    p.completed_items = 5;
    let pct = p.progress_percent();
    assert!((pct - 50.0).abs() < 0.1);
}

#[test]
fn test_progress_percent_by_items_when_no_bytes() {
    let mut p = OperationProgress::new(0, 4);
    p.completed_items = 2;
    let pct = p.progress_percent();
    assert!((pct - 50.0).abs() < 0.1);
}

#[test]
fn test_progress_zero_total() {
    let p = OperationProgress::new(0, 0);
    assert_eq!(p.progress_percent(), 0.0);
}

#[test]
fn test_progress_update() {
    let mut p = OperationProgress::new(2000, 4);
    p.update(1000, 2, "file2.txt".into(), 500);
    assert_eq!(p.completed_bytes, 1000);
    assert_eq!(p.completed_items, 2);
    assert_eq!(p.current_file, "file2.txt");
    assert_eq!(p.speed_bytes_per_sec, 500);
    // ETA: 1000 remaining / 500 bps = 2 seconds
    assert_eq!(p.eta_seconds, 2);
}

#[test]
fn test_progress_is_complete() {
    let mut p = OperationProgress::new(100, 2);
    assert!(!p.is_complete());
    p.completed_bytes = 100;
    p.completed_items = 2;
    assert!(p.is_complete());
}

// ===========================================================================
// FileOperation new constructors
// ===========================================================================

#[test]
fn test_file_operation_create_file() {
    let op = FileOperation::create_file("/home".into(), "notes.txt".into());
    assert_eq!(op.kind, OperationKind::CreateFile);
    assert_eq!(op.destination, "/home");
    assert_eq!(op.sources[0], "notes.txt");
}

#[test]
fn test_file_operation_compress() {
    let op = FileOperation::compress(vec!["/a".into(), "/b".into()], "/out.zip".into());
    assert_eq!(op.kind, OperationKind::Compress);
    assert_eq!(op.files_total, 2);
}

#[test]
fn test_file_operation_extract() {
    let op = FileOperation::extract("/archive.tar.gz".into(), "/dest".into());
    assert_eq!(op.kind, OperationKind::Extract);
}

#[test]
fn test_file_operation_to_file_op() {
    let op = FileOperation::copy(vec!["/a".into()], "/b".into());
    let fop = op.to_file_op().unwrap();
    assert!(matches!(fop, FileOp::Copy { .. }));
}

#[test]
fn test_operation_kind_new_variants_display() {
    assert_eq!(OperationKind::CreateFile.to_string(), "create file");
    assert_eq!(OperationKind::Compress.to_string(), "compress");
    assert_eq!(OperationKind::Extract.to_string(), "extract");
}
