//! File transfer types for USB mass storage redirection.

use serde::{Deserialize, Serialize};

/// A mounted filesystem from a USB device.
#[derive(Debug, Clone)]
pub struct MountPoint {
    pub path: String,
    pub device_name: String,
    pub read_only: bool,
}

/// A file or directory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified: u64,
}

/// File operations supported by the transfer subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperation {
    List,
    Read,
    Write,
    Delete,
    Stat,
}

/// Request for a file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    pub operation: FileOperation,
    pub path: String,
    pub data: Option<Vec<u8>>,
}

/// Response from a file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferResponse {
    pub success: bool,
    pub entries: Option<Vec<FileEntry>>,
    pub data: Option<Vec<u8>>,
    pub error: Option<String>,
}
