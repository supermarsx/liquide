use crate::crtc::CrtcId;
use crate::device::DrmDevice;
use crate::error::{DrmError, Result};

/// Bitflags for page flip requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlipFlags(u32);

impl PageFlipFlags {
    pub const EVENT: Self = Self(1 << 0);
    pub const ASYNC: Self = Self(1 << 1);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for PageFlipFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for PageFlipFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// An event delivered after a page flip completes at vblank.
#[derive(Debug, Clone)]
pub struct PageFlipEvent {
    pub sequence: u32,
    pub timestamp_ns: u64,
    pub crtc_id: CrtcId,
}

/// Requests a page flip on a CRTC to the given framebuffer.
#[cfg(target_os = "linux")]
pub fn request_page_flip(
    _device: &DrmDevice,
    _crtc: CrtcId,
    _fb_id: u32,
    _flags: PageFlipFlags,
) -> Result<()> {
    // TODO: implement via DRM_IOCTL_MODE_PAGE_FLIP
    Err(DrmError::PageFlip("not yet implemented".to_string()))
}

#[cfg(not(target_os = "linux"))]
pub fn request_page_flip(
    _device: &DrmDevice,
    _crtc: CrtcId,
    _fb_id: u32,
    _flags: PageFlipFlags,
) -> Result<()> {
    Err(DrmError::NoDevice)
}

/// Waits for the next vblank on the given CRTC.
#[cfg(target_os = "linux")]
pub fn wait_vblank(_device: &DrmDevice, _crtc: CrtcId) -> Result<PageFlipEvent> {
    // TODO: implement via DRM_IOCTL_WAIT_VBLANK
    Err(DrmError::VblankWait("not yet implemented".to_string()))
}

#[cfg(not(target_os = "linux"))]
pub fn wait_vblank(_device: &DrmDevice, _crtc: CrtcId) -> Result<PageFlipEvent> {
    Err(DrmError::NoDevice)
}
