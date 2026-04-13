mod platform;
#[cfg(test)]
mod tests;

pub use platform::SharedMemory;

/// Shared memory buffer handle -- can be passed between processes
#[derive(Debug, Clone)]
pub struct ShmHandle {
    /// Platform-specific name/ID for the shared memory region
    pub name: String,
    /// Size in bytes
    pub size: usize,
}

/// Access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmAccess {
    ReadOnly,
    ReadWrite,
}

/// Shared memory region mapped into this process
pub trait SharedMemoryOps {
    /// Create a new shared memory region
    fn create(name: &str, size: usize) -> Result<Self, SharedMemoryError>
    where
        Self: Sized;

    /// Open an existing shared memory region
    fn open(name: &str, access: ShmAccess) -> Result<Self, SharedMemoryError>
    where
        Self: Sized;

    /// Get a raw pointer to the mapped memory
    fn as_ptr(&self) -> *const u8;

    /// Get a mutable raw pointer (only valid for ReadWrite mappings)
    fn as_mut_ptr(&mut self) -> *mut u8;

    /// Size of the mapping in bytes
    fn size(&self) -> usize;

    /// Get the access mode of this mapping.
    fn access(&self) -> ShmAccess;

    /// Get the handle (can be shared with other processes)
    fn handle(&self) -> ShmHandle;

    /// Read bytes from the shared memory
    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<(), SharedMemoryError> {
        if offset + buf.len() > self.size() {
            return Err(SharedMemoryError::OutOfBounds {
                offset,
                len: buf.len(),
                size: self.size(),
            });
        }
        unsafe {
            std::ptr::copy_nonoverlapping(self.as_ptr().add(offset), buf.as_mut_ptr(), buf.len());
        }
        Ok(())
    }

    /// Write bytes to the shared memory
    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), SharedMemoryError> {
        if self.access() == ShmAccess::ReadOnly {
            return Err(SharedMemoryError::PermissionDenied);
        }
        if offset + data.len() > self.size() {
            return Err(SharedMemoryError::OutOfBounds {
                offset,
                len: data.len(),
                size: self.size(),
            });
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.as_mut_ptr().add(offset), data.len());
        }
        Ok(())
    }

    /// Get a slice view of the shared memory
    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.as_ptr(), self.size()) }
    }

    /// Get a mutable slice view
    fn as_mut_slice(&mut self) -> &mut [u8] {
        assert!(
            self.access() == ShmAccess::ReadWrite,
            "as_mut_slice() called on a ReadOnly shared memory mapping"
        );
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr(), self.size()) }
    }
}

/// Errors
#[derive(Debug, Clone)]
pub enum SharedMemoryError {
    CreationFailed(String),
    OpenFailed(String),
    MapFailed(String),
    AlreadyExists(String),
    NotFound(String),
    PermissionDenied,
    OutOfBounds {
        offset: usize,
        len: usize,
        size: usize,
    },
    PlatformError(String),
}

impl std::fmt::Display for SharedMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreationFailed(msg) => write!(f, "creation failed: {}", msg),
            Self::OpenFailed(msg) => write!(f, "open failed: {}", msg),
            Self::MapFailed(msg) => write!(f, "map failed: {}", msg),
            Self::AlreadyExists(name) => write!(f, "already exists: {}", name),
            Self::NotFound(name) => write!(f, "not found: {}", name),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::OutOfBounds { offset, len, size } => {
                write!(
                    f,
                    "out of bounds: offset={}, len={}, size={}",
                    offset, len, size
                )
            }
            Self::PlatformError(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for SharedMemoryError {}

/// Helper: Create a shared memory region for a framebuffer
pub fn create_framebuffer_shm(
    name: &str,
    width: u32,
    height: u32,
    bpp: u32,
) -> Result<SharedMemory, SharedMemoryError> {
    let size = (width * height * bpp) as usize;
    SharedMemory::create(name, size)
}

/// Helper: Generate a unique SHM name for a window surface
pub fn surface_shm_name(session_id: u64, window_id: u64) -> String {
    format!("/liquide-surface-{}-{}", session_id, window_id)
}
