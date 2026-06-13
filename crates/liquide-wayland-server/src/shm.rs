use crate::error::{Result, WaylandServerError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmFormat {
    Argb8888,
    Xrgb8888,
    Rgb888,
}

#[derive(Debug)]
pub struct ShmPool {
    // The client-provided fd is owned by the server once received via SCM_RIGHTS.
    // Storing it as an `OwnedFd` guarantees it is closed exactly once when the pool
    // is dropped (no manual `libc::close`, no double-close), avoiding the fd leak
    // that previously caused compositor fd exhaustion over many pool lifecycles.
    #[cfg(target_os = "linux")]
    fd: std::os::fd::OwnedFd,
    // On non-Linux targets the pool is never constructed (`new` returns
    // `NotSupported`); keep a trivial field so the struct shape is stable.
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    fd: i32,
    size: usize,
    #[allow(dead_code)] // used in cfg(target_os = "linux") mmap/mremap/read paths
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
            use std::os::fd::{FromRawFd, OwnedFd};
            use std::ptr;
            // SAFETY: fd is a valid file descriptor provided by the client via
            // the Wayland protocol. We map it read-only with MAP_SHARED and check
            // for MAP_FAILED before storing the pointer.
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
                // Take ownership of the fd so it is closed on the error path too,
                // then drop it — the failed mapping never referenced it afterwards.
                // SAFETY: `fd` is the client-provided descriptor; we are the sole
                // owner now and have not stored it anywhere else.
                drop(unsafe { OwnedFd::from_raw_fd(fd) });
                return Err(WaylandServerError::ShmPool(format!(
                    "mmap failed for fd={fd}, size={size}"
                )));
            }
            // SAFETY: mmap with MAP_SHARED keeps the mapping valid independently of
            // the fd, and we now take sole ownership of the descriptor so `Drop`
            // closes it exactly once.
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
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
            // SAFETY: self.data was obtained from a successful mmap call and self.size
            // is the current mapping length. MREMAP_MAYMOVE allows the kernel to
            // relocate the mapping. We check for MAP_FAILED before storing.
            let new_data = unsafe {
                libc::mremap(
                    self.data as *mut _,
                    self.size,
                    new_size,
                    libc::MREMAP_MAYMOVE,
                )
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
            // SAFETY: offset + len has been bounds-checked above against self.size.
            // self.data is a valid pointer from mmap, and the pool memory is immutable
            // from our side (PROT_READ).
            Ok(unsafe { std::slice::from_raw_parts(self.data.add(offset), len) })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (offset, len);
            Err(WaylandServerError::NotSupported)
        }
    }

    pub fn fd(&self) -> i32 {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            self.fd.as_raw_fd()
        }

        #[cfg(not(target_os = "linux"))]
        {
            self.fd
        }
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
                // SAFETY: self.data and self.size were set by a successful mmap/mremap.
                // This is the single munmap call for this mapping. We unmap the memory
                // here, BEFORE `self.fd` (an `OwnedFd`) is dropped by the compiler after
                // this method returns, which then closes the descriptor exactly once.
                unsafe {
                    libc::munmap(self.data as *mut _, self.size);
                }
            }
            // `self.fd: OwnedFd` is closed automatically when the struct's fields are
            // dropped after this method body, i.e. after the munmap above. No explicit
            // `libc::close` is needed, and the single ownership prevents a double-close.
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// Probe whether `fd` is still an open descriptor in this process.
    ///
    /// SAFETY: `fcntl(F_GETFD)` only inspects the descriptor table; it has no
    /// side effects on the descriptor and is safe to call with any integer.
    fn fd_is_open(fd: i32) -> bool {
        unsafe { libc::fcntl(fd, libc::F_GETFD) != -1 }
    }

    /// Create a sealed, sized anonymous memory fd usable as an shm pool backing.
    fn make_memfd(size: usize) -> i32 {
        let name = CString::new("liquide-shm-test").unwrap();
        // SAFETY: `name` is a valid NUL-terminated C string; memfd_create returns
        // an owned fd (>= 0) or -1 on error.
        let fd = unsafe { libc::memfd_create(name.as_ptr(), 0) };
        assert!(
            fd >= 0,
            "memfd_create failed: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: `fd` is a freshly created, owned descriptor.
        let rc = unsafe { libc::ftruncate(fd, size as libc::off_t) };
        assert_eq!(
            rc,
            0,
            "ftruncate failed: {}",
            std::io::Error::last_os_error()
        );
        fd
    }

    /// Regression test for t49-e9-02: `ShmPool` must close the client-provided fd
    /// when dropped (previously it munmapped but leaked the descriptor, leading to
    /// fd exhaustion). We build a real pool from a memfd, record its raw fd, drop
    /// the pool, and assert the descriptor is no longer open.
    #[test]
    fn drop_closes_the_backing_fd() {
        let size = 4096;
        let raw_fd = make_memfd(size);
        assert!(fd_is_open(raw_fd), "memfd should start open");

        let pool = ShmPool::new(raw_fd, size).expect("pool creation should succeed on linux");
        assert_eq!(pool.fd(), raw_fd, "pool should expose the backing fd");
        assert!(
            fd_is_open(raw_fd),
            "fd must stay open while the pool is alive"
        );

        drop(pool);

        assert!(
            !fd_is_open(raw_fd),
            "ShmPool::drop must close the client fd (fd {raw_fd} still open => leak)"
        );
    }

    /// Even when `mmap` fails, `new` must not leak the fd: it takes ownership and
    /// closes it on the error path. We force failure with a zero-length mapping.
    #[test]
    fn failed_new_closes_the_backing_fd() {
        let raw_fd = make_memfd(4096);
        assert!(fd_is_open(raw_fd), "memfd should start open");

        // size == 0 makes mmap fail with EINVAL, exercising the error path.
        let err = ShmPool::new(raw_fd, 0);
        assert!(err.is_err(), "zero-length mmap should fail");

        assert!(
            !fd_is_open(raw_fd),
            "failed ShmPool::new must still close the fd it took ownership of"
        );
    }
}
