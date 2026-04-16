use crate::connector::ConnectorId;
use crate::device::DrmDevice;
use crate::error::Result;
use crate::mode::DrmMode;

/// Unique identifier for a CRTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CrtcId(pub u32);

/// Information about a single CRTC (display pipeline).
#[derive(Debug, Clone)]
pub struct CrtcInfo {
    pub id: CrtcId,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub mode: Option<DrmMode>,
    pub connector_id: Option<ConnectorId>,
}

/// Enumerates all CRTCs on the given DRM device.
///
/// # Stub
/// Not yet implemented — requires DRM ioctls
/// (`DRM_IOCTL_MODE_GETRESOURCES` + `DRM_IOCTL_MODE_GETCRTC`).
/// Currently returns an empty list.
#[cfg(target_os = "linux")]
pub fn enumerate_crtcs(_device: &DrmDevice) -> Result<Vec<CrtcInfo>> {
    // TODO: implement via DRM ioctl DRM_IOCTL_MODE_GETRESOURCES + DRM_IOCTL_MODE_GETCRTC
    Ok(Vec::new())
}

/// On non-Linux platforms, CRTC enumeration returns an empty list.
#[cfg(not(target_os = "linux"))]
pub fn enumerate_crtcs(_device: &DrmDevice) -> Result<Vec<CrtcInfo>> {
    Ok(Vec::new())
}
