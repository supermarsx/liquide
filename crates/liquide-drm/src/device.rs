use crate::error::{DrmError, Result};

/// Primary DRM device path.
pub const DRM_DEVICE_PATH_0: &str = "/dev/dri/card0";
/// Secondary DRM device path.
pub const DRM_DEVICE_PATH_1: &str = "/dev/dri/card1";
/// Render node path.
pub const DRM_RENDER_PATH_0: &str = "/dev/dri/renderD128";

/// DRM ioctl constants (Linux-specific).
/// Used by `set_master` / `drop_master` on Linux; unused on other platforms.
#[allow(dead_code)]
const DRM_IOCTL_SET_MASTER: u64 = 0x0000_641E;
#[allow(dead_code)]
const DRM_IOCTL_DROP_MASTER: u64 = 0x0000_641F;

/// Represents an open DRM device node.
#[derive(Debug)]
pub struct DrmDevice {
    fd: i32,
    path: String,
    is_master: bool,
}

impl DrmDevice {
    /// Opens a DRM device at the given path.
    #[cfg(target_os = "linux")]
    pub fn open(path: &str) -> Result<Self> {
        use std::ffi::CString;
        use std::os::raw::c_int;

        let c_path = CString::new(path).map_err(|_| DrmError::DeviceOpen {
            path: path.to_string(),
            reason: "invalid path".to_string(),
        })?;

        // SAFETY: We pass a valid null-terminated path string and check the return value.
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(DrmError::DeviceOpen {
                path: path.to_string(),
                reason: std::io::Error::last_os_error().to_string(),
            });
        }

        Ok(Self {
            fd,
            path: path.to_string(),
            is_master: false,
        })
    }

    /// On non-Linux platforms, DRM devices are not available.
    #[cfg(not(target_os = "linux"))]
    pub fn open(_path: &str) -> Result<Self> {
        Err(DrmError::NoDevice)
    }

    /// Scans `/dev/dri/card*` for the best primary DRM device.
    #[cfg(target_os = "linux")]
    pub fn find_primary() -> Result<Self> {
        for i in 0..16 {
            let path = format!("/dev/dri/card{i}");
            if let Ok(device) = Self::open(&path) {
                tracing::info!(path = %device.path, "found DRM device");
                return Ok(device);
            }
        }
        Err(DrmError::NoDevice)
    }

    /// On non-Linux platforms, no DRM device can be found.
    #[cfg(not(target_os = "linux"))]
    pub fn find_primary() -> Result<Self> {
        Err(DrmError::NoDevice)
    }

    /// Attempts to become DRM master for this device.
    #[cfg(target_os = "linux")]
    pub fn set_master(&mut self) -> Result<()> {
        // SAFETY: fd is a valid open file descriptor. The ioctl takes no additional args.
        let ret = unsafe { libc::ioctl(self.fd, DRM_IOCTL_SET_MASTER) };
        if ret < 0 {
            return Err(DrmError::Ioctl {
                name: "DRM_IOCTL_SET_MASTER".to_string(),
                reason: std::io::Error::last_os_error().to_string(),
            });
        }
        self.is_master = true;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn set_master(&mut self) -> Result<()> {
        Err(DrmError::NoDevice)
    }

    /// Drops DRM master status for this device.
    #[cfg(target_os = "linux")]
    pub fn drop_master(&mut self) -> Result<()> {
        // SAFETY: fd is a valid open file descriptor. The ioctl takes no additional args.
        let ret = unsafe { libc::ioctl(self.fd, DRM_IOCTL_DROP_MASTER) };
        if ret < 0 {
            return Err(DrmError::Ioctl {
                name: "DRM_IOCTL_DROP_MASTER".to_string(),
                reason: std::io::Error::last_os_error().to_string(),
            });
        }
        self.is_master = false;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn drop_master(&mut self) -> Result<()> {
        Err(DrmError::NoDevice)
    }

    /// Returns whether this device currently holds DRM master.
    pub fn is_master(&self) -> bool {
        self.is_master
    }

    /// Returns the raw file descriptor for the device.
    pub fn fd(&self) -> i32 {
        self.fd
    }

    /// Returns the device path.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Drop for DrmDevice {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if self.fd >= 0 {
                // SAFETY: fd is a valid open file descriptor, closed exactly once on drop.
                unsafe {
                    libc::close(self.fd);
                }
            }
        }
    }
}
