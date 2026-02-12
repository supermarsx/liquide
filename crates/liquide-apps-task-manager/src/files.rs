//! Open file handle types for the Files & Folders In Use tab (spec section 11).
//!
//! Shows all currently open file handles system-wide, which processes hold them,
//! lock status, and provides grouping and filtering capabilities.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of open file resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenFileType {
    RegularFile,
    Directory,
    NamedPipe,
    Socket,
    Device,
}

impl OpenFileType {
    /// Returns the string representation of this file type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RegularFile => "Regular File",
            Self::Directory => "Directory",
            Self::NamedPipe => "Named Pipe",
            Self::Socket => "Socket",
            Self::Device => "Device",
        }
    }
}

impl fmt::Display for OpenFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Type of access a process has on an open file handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileAccessType {
    Read,
    Write,
    ReadWrite,
    Execute,
    Delete,
}

impl FileAccessType {
    /// Returns the string representation of this access type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::ReadWrite => "Read/Write",
            Self::Execute => "Execute",
            Self::Delete => "Delete",
        }
    }
}

impl fmt::Display for FileAccessType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Type of file lock held on a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockType {
    None,
    Shared,
    Exclusive,
}

impl LockType {
    /// Returns the string representation of this lock type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Shared => "Shared",
            Self::Exclusive => "Exclusive",
        }
    }
}

impl fmt::Display for LockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a file handle allows other processes to share access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareMode {
    None,
    Read,
    Write,
    ReadWrite,
}

impl ShareMode {
    /// Returns the string representation of this share mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Read => "Read",
            Self::Write => "Write",
            Self::ReadWrite => "Read/Write",
        }
    }
}

impl fmt::Display for ShareMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How to group open file entries in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileGroupBy {
    Process,
    FileType,
    LockType,
    Directory,
}

impl FileGroupBy {
    /// Returns the string representation of this grouping mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Process => "Process",
            Self::FileType => "File Type",
            Self::LockType => "Lock Type",
            Self::Directory => "Directory",
        }
    }
}

impl fmt::Display for FileGroupBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Information about a single open file handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFileInfo {
    /// Full path of the open file or folder.
    pub path: String,
    /// Type of the open resource.
    pub file_type: OpenFileType,
    /// Access mode of the handle.
    pub access: FileAccessType,
    /// Lock type held on the file.
    pub lock_type: LockType,
    /// Share mode of the handle.
    pub share_mode: ShareMode,
    /// Process ID of the handle holder.
    pub pid: u32,
    /// Name of the process holding the handle.
    pub process_name: String,
    /// Handle value (system handle identifier).
    pub handle_value: u64,
    /// Size of the file in bytes, if available.
    pub size_bytes: Option<u64>,
    /// Current read/write offset in the file, if applicable.
    pub offset: Option<u64>,
    /// Timestamp when the handle was opened.
    pub open_since: Option<String>,
    /// Whether a delete operation is pending on this file.
    pub delete_pending: bool,
    /// Whether this handle refers to a directory.
    pub is_directory: bool,
}

impl Default for OpenFileInfo {
    fn default() -> Self {
        Self {
            path: String::new(),
            file_type: OpenFileType::RegularFile,
            access: FileAccessType::Read,
            lock_type: LockType::None,
            share_mode: ShareMode::None,
            pid: 0,
            process_name: String::new(),
            handle_value: 0,
            size_bytes: None,
            offset: None,
            open_since: None,
            delete_pending: false,
            is_directory: false,
        }
    }
}
