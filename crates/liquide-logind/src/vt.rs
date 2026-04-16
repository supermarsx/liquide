//! Virtual terminal allocation and management.

use crate::error::{LogindError, Result};

// Linux ioctl constants for VT/KD operations.
pub const KD_TEXT: i32 = 0x00;
pub const KD_GRAPHICS: i32 = 0x01;
pub const VT_ACTIVATE: u64 = 0x5606;
pub const VT_WAITACTIVE: u64 = 0x5607;
pub const VT_GETSTATE: u64 = 0x5603;
pub const KDSETMODE: u64 = 0x4B3A;
pub const KDGETMODE: u64 = 0x4B3B;

/// VT display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VtMode {
    /// Normal text mode.
    Text,
    /// Graphics mode (used by compositors).
    Graphics,
}

/// A handle to a virtual terminal.
pub struct VirtualTerminal {
    /// File descriptor for the TTY device (-1 if not open / stub).
    fd: i32,
    /// VT number (e.g. 7).
    vt_number: u32,
    /// Mode when the VT was opened, for restoration on drop.
    original_mode: VtMode,
    /// Whether this VT is currently the active foreground console.
    active: bool,
}

impl VirtualTerminal {
    /// Open a specific virtual terminal by number.
    ///
    /// On non-Linux platforms this returns a stub handle.
    pub fn open(vt_number: u32) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            use std::ffi::CString;

            let path = format!("/dev/tty{vt_number}");
            let c_path = CString::new(path.clone()).map_err(|e| LogindError::VtAllocation(e.to_string()))?;

            // SAFETY: opening a TTY device with standard flags.
            let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOCTTY) };
            if fd < 0 {
                return Err(LogindError::VtAllocation(format!(
                    "failed to open {path}: {}",
                    std::io::Error::last_os_error()
                )));
            }

            let mut mode: i32 = 0;
            // SAFETY: KDGETMODE ioctl on a valid tty fd.
            let ret = unsafe { libc::ioctl(fd, KDGETMODE, &mut mode) };
            let original_mode = if ret == 0 && mode == KD_GRAPHICS {
                VtMode::Graphics
            } else {
                VtMode::Text
            };

            Ok(Self {
                fd,
                vt_number,
                original_mode,
                active: false,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            tracing::debug!("VirtualTerminal::open({vt_number}): stub on non-Linux");
            Ok(Self {
                fd: -1,
                vt_number,
                original_mode: VtMode::Text,
                active: false,
            })
        }
    }

    /// Allocate the next free virtual terminal and open it.
    ///
    /// On non-Linux platforms this returns a stub VT 7.
    pub fn allocate_next() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            use std::ffi::CString;

            let tty0 = CString::new("/dev/tty0").unwrap();
            // SAFETY: opening tty0 to query free VTs.
            let fd0 = unsafe { libc::open(tty0.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
            if fd0 < 0 {
                return Err(LogindError::VtAllocation(format!(
                    "failed to open /dev/tty0: {}",
                    std::io::Error::last_os_error()
                )));
            }

            let mut vt_num: i32 = 0;
            // VT_OPENQRY = 0x5600
            // SAFETY: querying the next free VT number via ioctl.
            let ret = unsafe { libc::ioctl(fd0, 0x5600u64, &mut vt_num) };
            unsafe { libc::close(fd0) };

            if ret < 0 || vt_num <= 0 {
                return Err(LogindError::VtAllocation(
                    "VT_OPENQRY failed or no free VT".to_string(),
                ));
            }

            Self::open(vt_num as u32)
        }

        #[cfg(not(target_os = "linux"))]
        {
            tracing::debug!("VirtualTerminal::allocate_next(): stub on non-Linux");
            Ok(Self {
                fd: -1,
                vt_number: 7,
                original_mode: VtMode::Text,
                active: false,
            })
        }
    }

    /// Activate (switch to) this virtual terminal.
    pub fn activate(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: VT_ACTIVATE ioctl to switch to this VT.
            let ret = unsafe { libc::ioctl(self.fd, VT_ACTIVATE, self.vt_number as i32) };
            if ret < 0 {
                return Err(LogindError::VtSwitch {
                    vt: self.vt_number,
                    reason: format!("VT_ACTIVATE: {}", std::io::Error::last_os_error()),
                });
            }
            // SAFETY: VT_WAITACTIVE ioctl to wait until the switch completes.
            let ret = unsafe { libc::ioctl(self.fd, VT_WAITACTIVE, self.vt_number as i32) };
            if ret < 0 {
                return Err(LogindError::VtSwitch {
                    vt: self.vt_number,
                    reason: format!("VT_WAITACTIVE: {}", std::io::Error::last_os_error()),
                });
            }
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            tracing::debug!("VirtualTerminal::activate({}): stub", self.vt_number);
            Ok(())
        }
    }

    /// Set the VT display mode (text or graphics).
    pub fn set_mode(&mut self, mode: VtMode) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let kd = match mode {
                VtMode::Text => KD_TEXT,
                VtMode::Graphics => KD_GRAPHICS,
            };
            // SAFETY: KDSETMODE ioctl on a valid tty fd.
            let ret = unsafe { libc::ioctl(self.fd, KDSETMODE, kd) };
            if ret < 0 {
                return Err(LogindError::VtIoctl {
                    name: "KDSETMODE".to_string(),
                    reason: std::io::Error::last_os_error().to_string(),
                });
            }
            self.active = mode == VtMode::Graphics;
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            tracing::debug!("VirtualTerminal::set_mode({:?}): stub", mode);
            self.active = mode == VtMode::Graphics;
            Ok(())
        }
    }

    /// Get the VT number.
    pub fn vt_number(&self) -> u32 {
        self.vt_number
    }

    /// Whether this VT is in active graphics mode.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for VirtualTerminal {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if self.fd >= 0 {
                // Restore original mode before closing.
                let kd = match self.original_mode {
                    VtMode::Text => KD_TEXT,
                    VtMode::Graphics => KD_GRAPHICS,
                };
                // SAFETY: restoring KDSETMODE before closing the fd.
                unsafe { libc::ioctl(self.fd, KDSETMODE, kd) };
                // SAFETY: closing a valid fd.
                unsafe { libc::close(self.fd) };
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = self.fd;
            let _ = self.original_mode;
        }
    }
}
