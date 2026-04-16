use crate::error::{Result, WaylandServerError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmFormat {
    Argb8888,
    Xrgb8888,
    Rgb888,
}

#[derive(Debug)]
pub struct ShmPool {
    fd: i32,
    size: usize,
    data: *mut u8,
}

// SAFETY: ShmPool manages a memory-mapped region that is not shared across threads
// without external synchronization. The raw pointer is only dereferenced through
// safe methods that enforce bounds checking.
unsafe impl Send for ShmPool {}

impl ShmPool {
    pub fn new(fd: i32, size: usize) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            use std::ptr;
            let data = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    size,
                    libc::PROT_READ,
                    libc::MAP_SHARED,
                    fd,
                    0,
                )
            };
            if data == libc::MAP_FAILED {
                return Err(WaylandServerError::ShmPool(format!(
                    "mmap failed for fd={fd}, size={size}"
                )));
            }
            Ok(Self {
                fd,
                size,
                data: data as *mut u8,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (fd, size);
            Err(WaylandServerError::NotSupported)
        }
    }

    pub fn resize(&mut self, new_size: usize) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let new_data = unsafe {
                libc::mremap(self.data as *mut _, self.size, new_size, libc::MREMAP_MAYMOVE)
            };
            if new_data == libc::MAP_FAILED {
                return Err(WaylandServerError::ShmPool(format!(
                    "mremap failed: old_size={}, new_size={new_size}",
                    self.size
                )));
            }
            self.data = new_data as *mut u8;
            self.size = new_size;
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = new_size;
            Err(WaylandServerError::NotSupported)
        }
    }

    pub fn read(&self, offset: usize, len: usize) -> Result<&[u8]> {
        #[cfg(target_os = "linux")]
        {
            if offset + len > self.size {
                return Err(WaylandServerError::ShmPool(format!(
                    "read out of bounds: offset={offset}, len={len}, pool_size={}",
                    self.size
                )));
            }
            Ok(unsafe { std::slice::from_raw_parts(self.data.add(offset), len) })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (offset, len);
            Err(WaylandServerError::NotSupported)
        }
    }

    pub fn fd(&self) -> i32 {
        self.fd
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for ShmPool {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if !self.data.is_null() {
                unsafe {
                    libc::munmap(self.data as *mut _, self.size);
                }
            }
        }
    }
}
