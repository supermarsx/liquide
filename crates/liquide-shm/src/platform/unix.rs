use crate::{ShmAccess, ShmHandle, SharedMemoryError, SharedMemoryOps};
use std::ffi::CString;

pub struct SharedMemory {
    name: String,
    ptr: *mut u8,
    size: usize,
    fd: i32,
    is_owner: bool,
}

// Safety: SharedMemory is conceptually an owned memory region.
// The pointer is valid for the lifetime of the mapping and access
// is controlled by the caller (single-writer or external sync).
unsafe impl Send for SharedMemory {}

impl SharedMemoryOps for SharedMemory {
    fn create(name: &str, size: usize) -> Result<Self, SharedMemoryError> {
        let c_name =
            CString::new(name).map_err(|e| SharedMemoryError::CreationFailed(e.to_string()))?;

        unsafe {
            // Create shared memory object (O_EXCL = fail if already exists)
            let fd = libc::shm_open(
                c_name.as_ptr(),
                libc::O_CREAT | libc::O_RDWR | libc::O_EXCL,
                0o600,
            );
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    return Err(SharedMemoryError::AlreadyExists(name.into()));
                }
                return Err(SharedMemoryError::CreationFailed(err.to_string()));
            }

            // Set size
            if libc::ftruncate(fd, size as libc::off_t) < 0 {
                let err = std::io::Error::last_os_error();
                libc::close(fd);
                libc::shm_unlink(c_name.as_ptr());
                return Err(SharedMemoryError::CreationFailed(err.to_string()));
            }

            // Map into address space
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            );
            if ptr == libc::MAP_FAILED {
                let err = std::io::Error::last_os_error();
                libc::close(fd);
                libc::shm_unlink(c_name.as_ptr());
                return Err(SharedMemoryError::MapFailed(err.to_string()));
            }

            Ok(SharedMemory {
                name: name.into(),
                ptr: ptr as *mut u8,
                size,
                fd,
                is_owner: true,
            })
        }
    }

    fn open(name: &str, access: ShmAccess) -> Result<Self, SharedMemoryError> {
        let c_name =
            CString::new(name).map_err(|e| SharedMemoryError::OpenFailed(e.to_string()))?;

        let flags = match access {
            ShmAccess::ReadOnly => libc::O_RDONLY,
            ShmAccess::ReadWrite => libc::O_RDWR,
        };

        unsafe {
            let fd = libc::shm_open(c_name.as_ptr(), flags, 0);
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                return Err(SharedMemoryError::OpenFailed(err.to_string()));
            }

            // Get size from fstat
            let mut stat: libc::stat = std::mem::zeroed();
            if libc::fstat(fd, &mut stat) < 0 {
                let err = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(SharedMemoryError::OpenFailed(err.to_string()));
            }
            let size = stat.st_size as usize;

            let prot = match access {
                ShmAccess::ReadOnly => libc::PROT_READ,
                ShmAccess::ReadWrite => libc::PROT_READ | libc::PROT_WRITE,
            };

            let ptr = libc::mmap(std::ptr::null_mut(), size, prot, libc::MAP_SHARED, fd, 0);
            if ptr == libc::MAP_FAILED {
                let err = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(SharedMemoryError::MapFailed(err.to_string()));
            }

            Ok(SharedMemory {
                name: name.into(),
                ptr: ptr as *mut u8,
                size,
                fd,
                is_owner: false,
            })
        }
    }

    fn as_ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    fn size(&self) -> usize {
        self.size
    }

    fn handle(&self) -> ShmHandle {
        ShmHandle {
            name: self.name.clone(),
            size: self.size,
        }
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                libc::munmap(self.ptr as *mut libc::c_void, self.size);
            }
            if self.fd >= 0 {
                libc::close(self.fd);
            }
            if self.is_owner {
                let c_name = CString::new(self.name.as_str()).unwrap();
                libc::shm_unlink(c_name.as_ptr());
            }
        }
    }
}
