//! Typed DRM plane enumeration.
//!
//! Mirrors the encoder enumeration shape: `DRM_IOCTL_MODE_GETPLANERESOURCES`
//! supplies the plane id list, then `DRM_IOCTL_MODE_GETPLANE` populates per-
//! plane `possible_crtcs` and the supported framebuffer format list. Plane
//! "type" (Primary / Cursor / Overlay) is not exposed in `GETPLANE`; it lives
//! on the `type` property and is left as `Unknown` here for property-driven
//! callers to refine later.
//! Non-Linux targets return an empty list.

use crate::device::DrmDevice;
use crate::error::Result;

#[cfg(target_os = "linux")]
use crate::ioctl::{drm_ioctl, drm_iowr, slice_ptr_u64};

/// Unique identifier for a DRM plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlaneId(pub u32);

/// DRM plane role. Resolved from the plane `type` property by callers; left
/// as `Unknown` from raw `GETPLANE` enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneType {
    Primary,
    Cursor,
    Overlay,
    Unknown,
}

impl PlaneType {
    /// Map the raw kernel `DRM_PLANE_TYPE_*` value to a typed variant.
    pub fn from_caps(caps: u32) -> PlaneType {
        match caps {
            0 => PlaneType::Overlay,
            1 => PlaneType::Primary,
            2 => PlaneType::Cursor,
            _ => PlaneType::Unknown,
        }
    }
}

/// Information about a single DRM plane.
#[derive(Debug, Clone)]
pub struct PlaneInfo {
    pub id: PlaneId,
    /// Bitmask of CRTC indices this plane can be bound to, in
    /// `DRM_IOCTL_MODE_GETRESOURCES` enumeration order.
    pub possible_crtcs: u32,
    /// Supported framebuffer formats as DRM fourcc codes.
    pub formats: Vec<u32>,
}

#[cfg(target_os = "linux")]
pub(crate) const DRM_IOCTL_MODE_GETPLANERESOURCES: libc::c_ulong =
    drm_iowr(0xB5, std::mem::size_of::<DrmModeGetPlaneRes>());
#[cfg(target_os = "linux")]
pub(crate) const DRM_IOCTL_MODE_GETPLANE: libc::c_ulong =
    drm_iowr(0xB6, std::mem::size_of::<DrmModeGetPlane>());

#[cfg(target_os = "linux")]
const ENUMERATION_RETRY_LIMIT: usize = 3;

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DrmModeGetPlaneRes {
    pub plane_id_ptr: u64,
    pub count_planes: u32,
}

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DrmModeGetPlane {
    pub plane_id: u32,
    pub crtc_id: u32,
    pub fb_id: u32,
    pub possible_crtcs: u32,
    pub gamma_size: u32,
    pub count_format_types: u32,
    pub format_type_ptr: u64,
}

/// Enumerates all planes on the given DRM device.
#[cfg(target_os = "linux")]
pub fn enumerate_planes(device: &DrmDevice) -> Result<Vec<PlaneInfo>> {
    let plane_ids = enumerate_plane_ids(device.fd())?;
    let mut planes = Vec::with_capacity(plane_ids.len());

    for plane_id in plane_ids {
        if let Some(plane) = enumerate_plane(device.fd(), plane_id)? {
            planes.push(plane);
        }
    }

    Ok(planes)
}

/// On non-Linux platforms, plane enumeration returns an empty list.
#[cfg(not(target_os = "linux"))]
pub fn enumerate_planes(_device: &DrmDevice) -> Result<Vec<PlaneInfo>> {
    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_plane_ids(fd: i32) -> Result<Vec<u32>> {
    for _ in 0..ENUMERATION_RETRY_LIMIT {
        let mut resources = DrmModeGetPlaneRes::default();
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETPLANERESOURCES,
            "DRM_IOCTL_MODE_GETPLANERESOURCES",
            &mut resources,
        )?;

        let plane_count = resources.count_planes as usize;
        if plane_count == 0 {
            return Ok(Vec::new());
        }

        let mut plane_ids = vec![0u32; plane_count];
        let mut populated = DrmModeGetPlaneRes {
            plane_id_ptr: slice_ptr_u64(&mut plane_ids),
            count_planes: plane_ids.len() as u32,
        };
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETPLANERESOURCES,
            "DRM_IOCTL_MODE_GETPLANERESOURCES",
            &mut populated,
        )?;

        if (populated.count_planes as usize) <= plane_ids.len() {
            plane_ids.truncate(populated.count_planes as usize);
            plane_ids.retain(|plane_id| *plane_id != 0);
            return Ok(plane_ids);
        }
    }

    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_plane(fd: i32, plane_id: u32) -> Result<Option<PlaneInfo>> {
    // First call: discover format count.
    let mut probe = DrmModeGetPlane {
        plane_id,
        ..Default::default()
    };
    drm_ioctl(
        fd,
        DRM_IOCTL_MODE_GETPLANE,
        "DRM_IOCTL_MODE_GETPLANE",
        &mut probe,
    )?;

    let format_count = probe.count_format_types as usize;
    let mut formats = vec![0u32; format_count];
    let mut filled = DrmModeGetPlane {
        plane_id,
        count_format_types: formats.len() as u32,
        format_type_ptr: slice_ptr_u64(&mut formats),
        ..Default::default()
    };
    drm_ioctl(
        fd,
        DRM_IOCTL_MODE_GETPLANE,
        "DRM_IOCTL_MODE_GETPLANE",
        &mut filled,
    )?;

    if (filled.count_format_types as usize) < formats.len() {
        formats.truncate(filled.count_format_types as usize);
    }

    Ok(Some(PlaneInfo {
        id: PlaneId(filled.plane_id),
        possible_crtcs: filled.possible_crtcs,
        formats,
    }))
}
