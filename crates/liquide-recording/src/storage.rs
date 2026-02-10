//! Storage backend abstraction for recording data.

use crate::Result;

/// Abstract storage backend for writing recording data.
pub trait StorageBackend: Send {
    /// Write data to storage.
    fn write(&mut self, data: &[u8]) -> Result<()>;
    /// Flush buffered data.
    fn flush(&mut self) -> Result<()>;
    /// Total bytes written.
    fn bytes_written(&self) -> u64;
    /// Close the storage.
    fn close(&mut self) -> Result<()>;
}

/// In-memory storage backend.
pub struct MemoryStorage {
    buffer: Vec<u8>,
    closed: bool,
}

impl MemoryStorage {
    /// Create a new in-memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            closed: false,
        }
    }

    /// Access the accumulated buffer.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageBackend for MemoryStorage {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        if self.closed {
            return Err(crate::RecordingError::StorageError("storage closed".into()));
        }
        self.buffer.extend_from_slice(data);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn close(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
}

impl std::fmt::Display for MemoryStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemoryStorage({} bytes)", self.buffer.len())
    }
}

/// Simulated file-path storage backend (no actual I/O).
pub struct FilePathStorage {
    path: String,
    bytes_written: u64,
    closed: bool,
}

impl FilePathStorage {
    /// Create a new file path storage (simulated).
    #[must_use]
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            bytes_written: 0,
            closed: false,
        }
    }

    /// The target file path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl StorageBackend for FilePathStorage {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        if self.closed {
            return Err(crate::RecordingError::StorageError("storage closed".into()));
        }
        self.bytes_written += data.len() as u64;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    fn close(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
}

impl std::fmt::Display for FilePathStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FilePathStorage({}, {} bytes)",
            self.path, self.bytes_written
        )
    }
}
