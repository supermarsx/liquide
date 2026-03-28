//! Error types for storage operations.

/// Errors from storage subsystem operations.
#[derive(Debug, Clone)]
pub enum StorageError {
    /// The specified device was not found.
    DeviceNotFound(String),
    /// The specified partition was not found.
    PartitionNotFound(String),
    /// The partition is already mounted.
    AlreadyMounted(String),
    /// The partition is not mounted.
    NotMounted(String),
    /// The device cannot be ejected (e.g., system disk).
    CannotEject(String),
    /// Insufficient permissions for the operation.
    PermissionDenied,
    /// The mount point path does not exist or is not a directory.
    InvalidMountPoint(String),
    /// A platform command failed.
    CommandFailed(String),
    /// Could not parse platform command output.
    ParseError(String),
    /// Generic I/O error.
    IoError(String),
    /// The operation is not supported on this platform.
    NotSupported,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceNotFound(id) => write!(f, "device not found: {id}"),
            Self::PartitionNotFound(id) => write!(f, "partition not found: {id}"),
            Self::AlreadyMounted(id) => write!(f, "partition already mounted: {id}"),
            Self::NotMounted(id) => write!(f, "partition not mounted: {id}"),
            Self::CannotEject(reason) => write!(f, "cannot eject device: {reason}"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::InvalidMountPoint(path) => write!(f, "invalid mount point: {path}"),
            Self::CommandFailed(msg) => write!(f, "command failed: {msg}"),
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::NotSupported => write!(f, "operation not supported on this platform"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
    }
}
