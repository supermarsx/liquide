use crate::device::DrmDevice;
use crate::error::Result;
use crate::mode::DrmMode;

/// Unique identifier for a DRM connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectorId(pub u32);

/// Physical connector type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorType {
    HDMI,
    DisplayPort,
    VGA,
    DVI,
    LVDS,
    EDP,
    DSI,
    Virtual,
    Unknown(u32),
}

/// Whether a display is currently attached to the connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorStatus {
    Connected,
    Disconnected,
    Unknown,
}

/// Subpixel layout order of the attached display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubpixelOrder {
    Unknown,
    HorizontalRgb,
    HorizontalBgr,
    VerticalRgb,
    VerticalBgr,
    None,
}

/// Information about a single display connector.
#[derive(Debug, Clone)]
pub struct ConnectorInfo {
    pub id: ConnectorId,
    pub connector_type: ConnectorType,
    pub status: ConnectorStatus,
    pub modes: Vec<DrmMode>,
    pub physical_width_mm: u32,
    pub physical_height_mm: u32,
    pub subpixel_order: SubpixelOrder,
    pub encoder_id: Option<u32>,
}

/// Enumerates all connectors on the given DRM device.
#[cfg(target_os = "linux")]
pub fn enumerate_connectors(_device: &DrmDevice) -> Result<Vec<ConnectorInfo>> {
    // TODO: implement via DRM ioctl DRM_IOCTL_MODE_GETRESOURCES + DRM_IOCTL_MODE_GETCONNECTOR
    Ok(Vec::new())
}

/// On non-Linux platforms, connector enumeration returns an empty list.
#[cfg(not(target_os = "linux"))]
pub fn enumerate_connectors(_device: &DrmDevice) -> Result<Vec<ConnectorInfo>> {
    Ok(Vec::new())
}
