//! Automatic enumeration of `/dev/input/event*` devices.

use crate::classify::DeviceInfo;
use crate::error::Result;

/// Scans the system for evdev input devices.
pub struct EvdevEnumerator {
    _private: (),
}

impl EvdevEnumerator {
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Scan `/dev/input/event0` through `event255` and return information
    /// about every device that can be opened and queried.
    ///
    /// On non-Linux platforms this returns an empty list.
    pub fn scan(&self) -> Result<Vec<DeviceInfo>> {
        self.scan_inner()
    }

    /// Convenience: return only keyboard-class devices.
    pub fn scan_keyboards(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self
            .scan()?
            .into_iter()
            .filter(|d| d.device_class == crate::classify::DeviceClass::Keyboard)
            .collect())
    }

    /// Convenience: return only pointer-class devices (mouse, touchpad).
    pub fn scan_pointers(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self
            .scan()?
            .into_iter()
            .filter(|d| {
                matches!(
                    d.device_class,
                    crate::classify::DeviceClass::Mouse | crate::classify::DeviceClass::Touchpad
                )
            })
            .collect())
    }

    // ── Platform-specific scan implementation ───────────────────────────

    #[cfg(target_os = "linux")]
    fn scan_inner(&self) -> Result<Vec<DeviceInfo>> {
        let mut devices = Vec::new();
        for idx in 0..=255u32 {
            let path = format!("/dev/input/event{idx}");
            match query_device_info(&path) {
                Ok(info) => devices.push(info),
                Err(_) => continue,
            }
        }
        Ok(devices)
    }

    #[cfg(not(target_os = "linux"))]
    fn scan_inner(&self) -> Result<Vec<DeviceInfo>> {
        Ok(Vec::new())
    }
}

impl Default for EvdevEnumerator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Linux ioctl helpers ─────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux {
    /// `EVIOCGNAME(len)` — read device name, up to `len` bytes.
    /// Encodes as _IOC(_IOC_READ, 'E', 0x06, len).
    #[allow(non_snake_case)]
    pub const fn EVIOCGNAME(len: u32) -> u64 {
        // _IOC_READ = 2, type = 'E' = 0x45, nr = 0x06
        (2u64 << 30) | ((b'E' as u64) << 8) | 0x06 | ((len as u64) << 16)
    }

    /// `EVIOCGID` — read input_id struct (bus, vendor, product, version).
    pub const EVIOCGID: u64 = (2u64 << 30) | ((b'E' as u64) << 8) | 0x02 | (8u64 << 16);

    /// `EVIOCGBIT(ev, len)` — read event-type capability bits.
    #[allow(non_snake_case)]
    pub const fn EVIOCGBIT(ev: u32, len: u32) -> u64 {
        (2u64 << 30) | ((b'E' as u64) << 8) | (0x20 + ev as u64) | ((len as u64) << 16)
    }

    /// `EV_KEY`, `EV_REL`, `EV_ABS`, etc.
    pub const EV_KEY: u32 = 0x01;
    pub const EV_REL: u32 = 0x02;
    pub const EV_ABS: u32 = 0x03;
    pub const EV_MSC: u32 = 0x04;
    pub const EV_LED: u32 = 0x11;
    pub const EV_REP: u32 = 0x14;
    pub const EV_FF: u32 = 0x15;

    /// `ABS_MT_SLOT` — presence indicates multi-touch capability.
    pub const ABS_MT_SLOT: u32 = 0x2f;
}

#[cfg(target_os = "linux")]
fn query_device_info(path: &str) -> crate::error::Result<crate::classify::DeviceInfo> {
    use std::ffi::CString;
    use std::os::unix::io::AsRawFd;

    use crate::classify::{DeviceCapability, DeviceInfo, classify_device};
    use crate::error::LibinputError;

    let c_path = CString::new(path).map_err(|e| LibinputError::DeviceOpen {
        path: path.to_string(),
        reason: e.to_string(),
    })?;

    // Open read-only, non-blocking.
    // SAFETY: c_path is a valid null-terminated C string. We check the return value
    // before using fd, and wrap it in a File for RAII cleanup.
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(LibinputError::PermissionDenied {
                path: path.to_string(),
            });
        }
        return Err(LibinputError::DeviceOpen {
            path: path.to_string(),
            reason: err.to_string(),
        });
    }

    // SAFETY: fd is valid; we close it via the File wrapper.
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let raw_fd = file.as_raw_fd();

    // ── Device name ─────────────────────────────────────────────────
    let mut name_buf = [0u8; 256];
    // SAFETY: raw_fd is a valid evdev fd; name_buf is large enough for EVIOCGNAME.
    let ret = unsafe {
        libc::ioctl(
            raw_fd,
            linux::EVIOCGNAME(name_buf.len() as u32),
            name_buf.as_mut_ptr(),
        )
    };
    let name = if ret > 0 {
        let len = name_buf
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(ret as usize);
        String::from_utf8_lossy(&name_buf[..len]).to_string()
    } else {
        String::from("unknown")
    };

    // ── Input ID (bus, vendor, product, version) ────────────────────
    #[repr(C)]
    struct InputId {
        bustype: u16,
        vendor: u16,
        product: u16,
        version: u16,
    }
    let mut input_id = InputId {
        bustype: 0,
        vendor: 0,
        product: 0,
        version: 0,
    };
    // SAFETY: EVIOCGID reads an 8-byte input_id struct; input_id has #[repr(C)] layout.
    let _ = unsafe { libc::ioctl(raw_fd, linux::EVIOCGID, &mut input_id) };

    // ── Capability bits (EV_*) ──────────────────────────────────────
    let mut ev_bits = [0u8; 4];
    // SAFETY: EVIOCGBIT(0, len) reads top-level event-type bits into ev_bits.
    let _ = unsafe {
        libc::ioctl(
            raw_fd,
            linux::EVIOCGBIT(0, ev_bits.len() as u32),
            ev_bits.as_mut_ptr(),
        )
    };

    let bit_set = |ev: u32| -> bool {
        let byte = (ev / 8) as usize;
        let bit = ev % 8;
        byte < ev_bits.len() && (ev_bits[byte] & (1 << bit)) != 0
    };

    let mut caps = DeviceCapability::EMPTY;
    if bit_set(linux::EV_KEY) {
        caps |= DeviceCapability::KEY;
    }
    if bit_set(linux::EV_REL) {
        caps |= DeviceCapability::REL;
    }
    if bit_set(linux::EV_ABS) {
        caps |= DeviceCapability::ABS;
    }
    if bit_set(linux::EV_MSC) {
        caps |= DeviceCapability::MSC;
    }
    if bit_set(linux::EV_LED) {
        caps |= DeviceCapability::LED;
    }
    if bit_set(linux::EV_REP) {
        caps |= DeviceCapability::REP;
    }
    if bit_set(linux::EV_FF) {
        caps |= DeviceCapability::FF;
    }

    // ── Multi-touch detection ───────────────────────────────────────
    let has_abs_mt = if caps.contains(DeviceCapability::ABS) {
        let mut abs_bits = [0u8; 8]; // enough for ABS_MT_SLOT (0x2f = 47)
        // SAFETY: EVIOCGBIT(EV_ABS, len) reads ABS capability bits into abs_bits.
        let _ = unsafe {
            libc::ioctl(
                raw_fd,
                linux::EVIOCGBIT(linux::EV_ABS, abs_bits.len() as u32),
                abs_bits.as_mut_ptr(),
            )
        };
        let slot = linux::ABS_MT_SLOT;
        let byte = (slot / 8) as usize;
        let bit = slot % 8;
        byte < abs_bits.len() && (abs_bits[byte] & (1 << bit)) != 0
    } else {
        false
    };

    let device_class = classify_device(caps, has_abs_mt);

    Ok(DeviceInfo {
        path: path.to_string(),
        name,
        device_class,
        capabilities: caps,
        vendor_id: input_id.vendor,
        product_id: input_id.product,
        bus_type: input_id.bustype,
    })
}

// Bring `FromRawFd` into scope for the Linux impl.
#[cfg(target_os = "linux")]
use std::os::unix::io::FromRawFd;
