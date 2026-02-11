use crate::file_transfer::{
    FileEntry, FileOperation, FileTransferRequest, FileTransferResponse, MountPoint,
};

#[test]
fn test_mount_point() {
    let mp = MountPoint {
        path: "/mnt/usb0".to_string(),
        device_name: "USB Drive".to_string(),
        read_only: true,
    };
    assert_eq!(mp.path, "/mnt/usb0");
    assert!(mp.read_only);
}

#[test]
fn test_file_entry_serialize() {
    let entry = FileEntry {
        name: "readme.txt".to_string(),
        size: 1024,
        is_dir: false,
        modified: 1700000000,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: FileEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.name, "readme.txt");
    assert_eq!(decoded.size, 1024);
    assert!(!decoded.is_dir);
}

#[test]
fn test_file_transfer_request_serialize() {
    let req = FileTransferRequest {
        operation: FileOperation::List,
        path: "/some/dir".to_string(),
        data: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    let decoded: FileTransferRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.operation, FileOperation::List);
    assert_eq!(decoded.path, "/some/dir");
}

#[test]
fn test_file_transfer_response_serialize() {
    let resp = FileTransferResponse {
        success: true,
        entries: Some(vec![FileEntry {
            name: "file.bin".to_string(),
            size: 256,
            is_dir: false,
            modified: 1700000000,
        }]),
        data: None,
        error: None,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let decoded: FileTransferResponse = serde_json::from_str(&json).unwrap();
    assert!(decoded.success);
    assert_eq!(decoded.entries.unwrap().len(), 1);
}

#[test]
fn test_file_operation_variants() {
    let ops = [
        FileOperation::List,
        FileOperation::Read,
        FileOperation::Write,
        FileOperation::Delete,
        FileOperation::Stat,
    ];
    assert_eq!(ops.len(), 5);
}
