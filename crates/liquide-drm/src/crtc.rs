use crate::connector::{ConnectorId, ConnectorInfo};
use crate::device::DrmDevice;
use crate::encoder::EncoderInfo;
use crate::error::Result;
use crate::mode::DrmMode;

#[cfg(target_os = "linux")]
use crate::ioctl::{drm_ioctl, drm_iowr, slice_ptr_u64};
#[cfg(target_os = "linux")]
use crate::mode::{self, RawDrmModeInfo};

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

#[cfg(target_os = "linux")]
const DRM_IOCTL_MODE_GETRESOURCES: libc::c_ulong =
    drm_iowr(0xA0, std::mem::size_of::<DrmModeCardRes>());
#[cfg(target_os = "linux")]
const DRM_IOCTL_MODE_GETCRTC: libc::c_ulong =
    drm_iowr(0xA1, std::mem::size_of::<DrmModeCrtc>());

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
struct DrmModeCrtc {
    set_connectors_ptr: u64,
    count_connectors: u32,
    crtc_id: u32,
    fb_id: u32,
    x: u32,
    y: u32,
    gamma_size: u32,
    mode_valid: u32,
    mode: RawDrmModeInfo,
}

/// Enumerates all CRTCs on the given DRM device.
///
/// CRTCs are returned in the order reported by `DRM_IOCTL_MODE_GETRESOURCES`.
/// That order is the canonical mapping for `possible_crtcs` bitmask bits in
/// `EncoderInfo`: bit `i` set in `possible_crtcs` refers to the CRTC at index
/// `i` in this returned `Vec`. [`select_crtc_for_connector`] relies on this
/// invariant.
#[cfg(target_os = "linux")]
pub fn enumerate_crtcs(device: &DrmDevice) -> Result<Vec<CrtcInfo>> {
    let crtc_ids = enumerate_crtc_ids(device.fd())?;
    let mut crtcs = Vec::with_capacity(crtc_ids.len());

    for crtc_id in crtc_ids {
        if let Some(crtc) = enumerate_crtc(device.fd(), crtc_id)? {
            crtcs.push(crtc);
        }
    }

    Ok(crtcs)
}

/// On non-Linux platforms, CRTC enumeration returns an empty list.
#[cfg(not(target_os = "linux"))]
pub fn enumerate_crtcs(_device: &DrmDevice) -> Result<Vec<CrtcInfo>> {
    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_crtc_ids(fd: i32) -> Result<Vec<u32>> {
    for _ in 0..ENUMERATION_RETRY_LIMIT {
        let mut resources = DrmModeCardRes::default();
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETRESOURCES,
            "DRM_IOCTL_MODE_GETRESOURCES",
            &mut resources,
        )?;

        let crtc_count = resources.count_crtcs as usize;
        if crtc_count == 0 {
            return Ok(Vec::new());
        }

        let mut crtc_ids = vec![0u32; crtc_count];
        let mut populated = DrmModeCardRes {
            crtc_id_ptr: slice_ptr_u64(&mut crtc_ids),
            count_crtcs: crtc_ids.len() as u32,
            ..Default::default()
        };
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETRESOURCES,
            "DRM_IOCTL_MODE_GETRESOURCES",
            &mut populated,
        )?;

        if (populated.count_crtcs as usize) <= crtc_ids.len() {
            crtc_ids.truncate(populated.count_crtcs as usize);
            crtc_ids.retain(|crtc_id| *crtc_id != 0);
            return Ok(crtc_ids);
        }
    }

    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_crtc(fd: i32, crtc_id: u32) -> Result<Option<CrtcInfo>> {
    let mut request = DrmModeCrtc {
        crtc_id,
        ..Default::default()
    };
    drm_ioctl(
        fd,
        DRM_IOCTL_MODE_GETCRTC,
        "DRM_IOCTL_MODE_GETCRTC",
        &mut request,
    )?;

    let mode = if request.mode_valid != 0 {
        mode::from_raw_mode_info(&request.mode)
    } else {
        None
    };

    Ok(Some(CrtcInfo {
        id: CrtcId(request.crtc_id),
        x: request.x,
        y: request.y,
        width: u32::from(request.mode.hdisplay),
        height: u32::from(request.mode.vdisplay),
        mode,
        connector_id: None,
    }))
}

/// Selects a CRTC capable of driving the given connector.
///
/// Selection prefers, in order:
/// 1. The encoder currently attached to the connector
///    (`connector.encoder_id`) when it is also actively driving a CRTC
///    present in `crtcs`.
/// 2. The same encoder's `possible_crtcs` bitmask, walking `crtcs` in
///    enumeration order and returning the first CRTC whose index is set.
/// 3. Any other encoder in `encoders`, again using `possible_crtcs` against
///    `crtcs` in enumeration order.
///
/// Returns `None` when no compatible CRTC exists. Never panics.
///
/// # Enumeration-order invariant
/// `crtcs` must be the slice returned by [`enumerate_crtcs`] (or an
/// equivalently ordered list). `EncoderInfo::possible_crtcs` is a bitmask
/// indexing that exact ordering: bit `i` refers to `crtcs[i]`.
pub fn select_crtc_for_connector(
    connector: &ConnectorInfo,
    encoders: &[EncoderInfo],
    crtcs: &[CrtcInfo],
) -> Option<CrtcId> {
    if crtcs.is_empty() {
        return None;
    }

    let attached_encoder = connector
        .encoder_id
        .and_then(|id| encoders.iter().find(|enc| enc.id.0 == id));

    if let Some(encoder) = attached_encoder {
        if let Some(crtc_id) = encoder.crtc_id {
            if crtcs.iter().any(|crtc| crtc.id == crtc_id) {
                return Some(crtc_id);
            }
        }
        if let Some(picked) = crtc_from_possible_mask(encoder.possible_crtcs, crtcs) {
            return Some(picked);
        }
    }

    for encoder in encoders {
        if attached_encoder.is_some_and(|attached| attached.id == encoder.id) {
            continue;
        }
        if let Some(picked) = crtc_from_possible_mask(encoder.possible_crtcs, crtcs) {
            return Some(picked);
        }
    }

    None
}

fn crtc_from_possible_mask(possible_crtcs: u32, crtcs: &[CrtcInfo]) -> Option<CrtcId> {
    if possible_crtcs == 0 {
        return None;
    }
    let limit = crtcs.len().min(u32::BITS as usize);
    for index in 0..limit {
        if (possible_crtcs & (1u32 << index)) != 0 {
            return Some(crtcs[index].id);
        }
    }
    None
}
