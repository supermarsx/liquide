use crate::device::DrmDevice;
use crate::error::Result;
use crate::mode::{self, DrmMode};

#[cfg(target_os = "linux")]
use crate::error::DrmError;
#[cfg(target_os = "linux")]
use crate::mode::RawDrmModeInfo;

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
    pub connector_type_id: u32,
    pub name: String,
    pub status: ConnectorStatus,
    pub modes: Vec<DrmMode>,
    pub physical_width_mm: u32,
    pub physical_height_mm: u32,
    pub subpixel_order: SubpixelOrder,
    pub encoder_id: Option<u32>,
}

impl ConnectorInfo {
    pub fn is_connected(&self) -> bool {
        matches!(self.status, ConnectorStatus::Connected)
    }

    pub fn stable_name(&self) -> &str {
        &self.name
    }

    pub fn launchable_mode(&self) -> Option<&DrmMode> {
        mode::launchable_mode(&self.modes)
    }
}

#[cfg(target_os = "linux")]
const DRM_MODE_CONNECTED: u32 = 1;
#[cfg(target_os = "linux")]
const DRM_MODE_DISCONNECTED: u32 = 2;
#[cfg(target_os = "linux")]
const DRM_MODE_UNKNOWNCONNECTION: u32 = 3;

#[cfg(target_os = "linux")]
const DRM_MODE_SUBPIXEL_UNKNOWN: u32 = 0;
#[cfg(target_os = "linux")]
const DRM_MODE_SUBPIXEL_HORIZONTAL_RGB: u32 = 1;
#[cfg(target_os = "linux")]
const DRM_MODE_SUBPIXEL_HORIZONTAL_BGR: u32 = 2;
#[cfg(target_os = "linux")]
const DRM_MODE_SUBPIXEL_VERTICAL_RGB: u32 = 3;
#[cfg(target_os = "linux")]
const DRM_MODE_SUBPIXEL_VERTICAL_BGR: u32 = 4;
#[cfg(target_os = "linux")]
const DRM_MODE_SUBPIXEL_NONE: u32 = 5;

#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_VGA: u32 = 1;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_DVII: u32 = 2;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_DVID: u32 = 3;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_DVIA: u32 = 4;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_LVDS: u32 = 7;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_DISPLAYPORT: u32 = 10;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_HDMIA: u32 = 11;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_HDMIB: u32 = 12;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_EDP: u32 = 14;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_VIRTUAL: u32 = 15;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_CONNECTOR_DSI: u32 = 16;

#[cfg(target_os = "linux")]
const IOC_NRBITS: u32 = 8;
#[cfg(target_os = "linux")]
const IOC_TYPEBITS: u32 = 8;
#[cfg(target_os = "linux")]
const IOC_SIZEBITS: u32 = 14;
#[cfg(target_os = "linux")]
const IOC_NRSHIFT: u32 = 0;
#[cfg(target_os = "linux")]
const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
#[cfg(target_os = "linux")]
const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
#[cfg(target_os = "linux")]
const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;
#[cfg(target_os = "linux")]
const IOC_WRITE: u32 = 1;
#[cfg(target_os = "linux")]
const IOC_READ: u32 = 2;
#[cfg(target_os = "linux")]
const DRM_IOCTL_BASE: u32 = b'd' as u32;

#[cfg(target_os = "linux")]
const DRM_IOCTL_MODE_GETRESOURCES: libc::c_ulong =
    drm_iowr(0xA0, std::mem::size_of::<DrmModeCardRes>());
#[cfg(target_os = "linux")]
const DRM_IOCTL_MODE_GETCONNECTOR: libc::c_ulong =
    drm_iowr(0xA7, std::mem::size_of::<DrmModeGetConnector>());

#[cfg(target_os = "linux")]
const ENUMERATION_RETRY_LIMIT: usize = 3;

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmModeCardRes {
    fb_id_ptr: u64,
    crtc_id_ptr: u64,
    connector_id_ptr: u64,
    encoder_id_ptr: u64,
    count_fbs: u32,
    count_crtcs: u32,
    count_connectors: u32,
    count_encoders: u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
}

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmModeGetConnector {
    encoders_ptr: u64,
    modes_ptr: u64,
    props_ptr: u64,
    prop_values_ptr: u64,
    count_modes: u32,
    count_props: u32,
    count_encoders: u32,
    encoder_id: u32,
    connector_id: u32,
    connector_type: u32,
    connector_type_id: u32,
    connection: u32,
    mm_width: u32,
    mm_height: u32,
    subpixel: u32,
    pad: u32,
}

#[cfg(target_os = "linux")]
const fn drm_iowr(nr: u32, size: usize) -> libc::c_ulong {
    (((IOC_READ | IOC_WRITE) as u64) << IOC_DIRSHIFT
        | (DRM_IOCTL_BASE as u64) << IOC_TYPESHIFT
        | (nr as u64) << IOC_NRSHIFT
        | ((size as u64) << IOC_SIZESHIFT)) as libc::c_ulong
}

#[cfg(any(test, target_os = "linux"))]
pub(crate) fn stable_connector_name(
    raw_connector_type: u32,
    connector_type_id: u32,
    connector_id: u32,
) -> String {
    let suffix = if connector_type_id > 0 {
        connector_type_id
    } else {
        connector_id
    };
    format!("{}-{suffix}", raw_connector_type_label(raw_connector_type))
}

/// Enumerates all connectors on the given DRM device.
#[cfg(target_os = "linux")]
pub fn enumerate_connectors(device: &DrmDevice) -> Result<Vec<ConnectorInfo>> {
    let connector_ids = enumerate_connector_ids(device.fd())?;
    let mut connectors = Vec::with_capacity(connector_ids.len());

    for connector_id in connector_ids {
        let Some(connector) = enumerate_connector(device.fd(), connector_id)? else {
            continue;
        };
        connectors.push(connector);
    }

    Ok(connectors)
}

/// On non-Linux platforms, connector enumeration returns an empty list.
#[cfg(not(target_os = "linux"))]
pub fn enumerate_connectors(_device: &DrmDevice) -> Result<Vec<ConnectorInfo>> {
    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_connector_ids(fd: i32) -> Result<Vec<u32>> {
    for _ in 0..ENUMERATION_RETRY_LIMIT {
        let mut resources = DrmModeCardRes::default();
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETRESOURCES,
            "DRM_IOCTL_MODE_GETRESOURCES",
            &mut resources,
        )?;

        let connector_count = resources.count_connectors as usize;
        if connector_count == 0 {
            return Ok(Vec::new());
        }

        let mut connector_ids = vec![0u32; connector_count];
        let mut populated = DrmModeCardRes {
            connector_id_ptr: slice_ptr_u64(&mut connector_ids),
            count_connectors: connector_ids.len() as u32,
            ..Default::default()
        };
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETRESOURCES,
            "DRM_IOCTL_MODE_GETRESOURCES",
            &mut populated,
        )?;

        if (populated.count_connectors as usize) <= connector_ids.len() {
            connector_ids.truncate(populated.count_connectors as usize);
            connector_ids.retain(|connector_id| *connector_id != 0);
            return Ok(connector_ids);
        }
    }

    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_connector(fd: i32, connector_id: u32) -> Result<Option<ConnectorInfo>> {
    for _ in 0..ENUMERATION_RETRY_LIMIT {
        let mut request = DrmModeGetConnector {
            connector_id,
            ..Default::default()
        };
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETCONNECTOR,
            "DRM_IOCTL_MODE_GETCONNECTOR",
            &mut request,
        )?;

        let mut modes = vec![RawDrmModeInfo::default(); request.count_modes as usize];
        let mut encoders = vec![0u32; request.count_encoders as usize];
        request.modes_ptr = slice_ptr_u64(&mut modes);
        request.count_modes = modes.len() as u32;
        request.encoders_ptr = slice_ptr_u64(&mut encoders);
        request.count_encoders = encoders.len() as u32;

        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETCONNECTOR,
            "DRM_IOCTL_MODE_GETCONNECTOR",
            &mut request,
        )?;

        if (request.count_modes as usize) > modes.len()
            || (request.count_encoders as usize) > encoders.len()
        {
            continue;
        }

        return Ok(Some(build_connector_info(&request, &modes)));
    }

    Ok(None)
}

#[cfg(target_os = "linux")]
fn build_connector_info(
    request: &DrmModeGetConnector,
    raw_modes: &[RawDrmModeInfo],
) -> ConnectorInfo {
    let modes = raw_modes
        .iter()
        .take(request.count_modes as usize)
        .filter_map(mode::from_raw_mode_info)
        .collect();

    ConnectorInfo {
        id: ConnectorId(request.connector_id),
        connector_type: connector_type_from_raw(request.connector_type),
        connector_type_id: request.connector_type_id,
        name: stable_connector_name(
            request.connector_type,
            request.connector_type_id,
            request.connector_id,
        ),
        status: connector_status_from_raw(request.connection),
        modes,
        physical_width_mm: request.mm_width,
        physical_height_mm: request.mm_height,
        subpixel_order: subpixel_order_from_raw(request.subpixel),
        encoder_id: (request.encoder_id != 0).then_some(request.encoder_id),
    }
}

#[cfg(target_os = "linux")]
fn connector_type_from_raw(raw: u32) -> ConnectorType {
    match raw {
        DRM_MODE_CONNECTOR_VGA => ConnectorType::VGA,
        DRM_MODE_CONNECTOR_DVII | DRM_MODE_CONNECTOR_DVID | DRM_MODE_CONNECTOR_DVIA => {
            ConnectorType::DVI
        }
        DRM_MODE_CONNECTOR_LVDS => ConnectorType::LVDS,
        DRM_MODE_CONNECTOR_DISPLAYPORT => ConnectorType::DisplayPort,
        DRM_MODE_CONNECTOR_HDMIA | DRM_MODE_CONNECTOR_HDMIB => ConnectorType::HDMI,
        DRM_MODE_CONNECTOR_EDP => ConnectorType::EDP,
        DRM_MODE_CONNECTOR_VIRTUAL => ConnectorType::Virtual,
        DRM_MODE_CONNECTOR_DSI => ConnectorType::DSI,
        _ => ConnectorType::Unknown(raw),
    }
}

#[cfg(any(test, target_os = "linux"))]
fn raw_connector_type_label(raw: u32) -> &'static str {
    match raw {
        DRM_MODE_CONNECTOR_VGA => "VGA",
        DRM_MODE_CONNECTOR_DVII => "DVI-I",
        DRM_MODE_CONNECTOR_DVID => "DVI-D",
        DRM_MODE_CONNECTOR_DVIA => "DVI-A",
        DRM_MODE_CONNECTOR_LVDS => "LVDS",
        DRM_MODE_CONNECTOR_DISPLAYPORT => "DP",
        DRM_MODE_CONNECTOR_HDMIA => "HDMI-A",
        DRM_MODE_CONNECTOR_HDMIB => "HDMI-B",
        DRM_MODE_CONNECTOR_EDP => "eDP",
        DRM_MODE_CONNECTOR_VIRTUAL => "Virtual",
        DRM_MODE_CONNECTOR_DSI => "DSI",
        _ => "Unknown",
    }
}

#[cfg(target_os = "linux")]
fn connector_status_from_raw(raw: u32) -> ConnectorStatus {
    match raw {
        DRM_MODE_CONNECTED => ConnectorStatus::Connected,
        DRM_MODE_DISCONNECTED => ConnectorStatus::Disconnected,
        DRM_MODE_UNKNOWNCONNECTION => ConnectorStatus::Unknown,
        _ => ConnectorStatus::Unknown,
    }
}

#[cfg(target_os = "linux")]
fn subpixel_order_from_raw(raw: u32) -> SubpixelOrder {
    match raw {
        DRM_MODE_SUBPIXEL_HORIZONTAL_RGB => SubpixelOrder::HorizontalRgb,
        DRM_MODE_SUBPIXEL_HORIZONTAL_BGR => SubpixelOrder::HorizontalBgr,
        DRM_MODE_SUBPIXEL_VERTICAL_RGB => SubpixelOrder::VerticalRgb,
        DRM_MODE_SUBPIXEL_VERTICAL_BGR => SubpixelOrder::VerticalBgr,
        DRM_MODE_SUBPIXEL_NONE => SubpixelOrder::None,
        DRM_MODE_SUBPIXEL_UNKNOWN => SubpixelOrder::Unknown,
        _ => SubpixelOrder::Unknown,
    }
}

#[cfg(target_os = "linux")]
fn slice_ptr_u64<T>(slice: &mut [T]) -> u64 {
    if slice.is_empty() {
        0
    } else {
        slice.as_mut_ptr() as usize as u64
    }
}

#[cfg(target_os = "linux")]
fn drm_ioctl<T>(fd: i32, request: libc::c_ulong, name: &str, arg: &mut T) -> Result<()> {
    // SAFETY: `arg` points to initialized storage for the duration of the ioctl call.
    let result = unsafe { libc::ioctl(fd, request, arg as *mut T) };
    if result < 0 {
        return Err(DrmError::Ioctl {
            name: name.to_string(),
            reason: std::io::Error::last_os_error().to_string(),
        });
    }
    Ok(())
}
