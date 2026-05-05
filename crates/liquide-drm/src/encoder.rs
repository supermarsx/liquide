//! Typed DRM encoder enumeration.
//!
//! Mirrors the connector enumeration shape: `DRM_IOCTL_MODE_GETRESOURCES`
//! supplies the encoder id list, then `DRM_IOCTL_MODE_GETENCODER` populates
//! per-encoder type / current attachment / possible_crtcs / possible_clones.
//! Non-Linux targets return an empty list.

use crate::crtc::CrtcId;
use crate::device::DrmDevice;
use crate::error::Result;

#[cfg(target_os = "linux")]
use crate::ioctl::{drm_ioctl, drm_iowr, slice_ptr_u64};

/// Unique identifier for a DRM encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EncoderId(pub u32);

/// Physical encoder kind reported by the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderType {
    None,
    DAC,
    TMDS,
    LVDS,
    TVDAC,
    Virtual,
    DSI,
    DPMST,
    DPI,
    Unknown(u32),
}

/// Information about a single DRM encoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderInfo {
    pub id: EncoderId,
    pub encoder_type: EncoderType,
    /// Id of the CRTC currently driven by this encoder, if any.
    pub crtc_id: Option<CrtcId>,
    /// Bitmask of CRTC indices this encoder can drive, in
    /// `DRM_IOCTL_MODE_GETRESOURCES` enumeration order.
    pub possible_crtcs: u32,
    /// Bitmask of encoders this one can be cloned with, in
    /// `DRM_IOCTL_MODE_GETRESOURCES` enumeration order.
    pub possible_clones: u32,
}

#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_NONE: u32 = 0;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_DAC: u32 = 1;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_TMDS: u32 = 2;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_LVDS: u32 = 3;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_TVDAC: u32 = 4;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_VIRTUAL: u32 = 5;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_DSI: u32 = 6;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_DPMST: u32 = 7;
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_MODE_ENCODER_DPI: u32 = 8;

#[cfg(any(test, target_os = "linux"))]
pub(crate) fn encoder_type_from_raw(raw: u32) -> EncoderType {
    match raw {
        DRM_MODE_ENCODER_NONE => EncoderType::None,
        DRM_MODE_ENCODER_DAC => EncoderType::DAC,
        DRM_MODE_ENCODER_TMDS => EncoderType::TMDS,
        DRM_MODE_ENCODER_LVDS => EncoderType::LVDS,
        DRM_MODE_ENCODER_TVDAC => EncoderType::TVDAC,
        DRM_MODE_ENCODER_VIRTUAL => EncoderType::Virtual,
        DRM_MODE_ENCODER_DSI => EncoderType::DSI,
        DRM_MODE_ENCODER_DPMST => EncoderType::DPMST,
        DRM_MODE_ENCODER_DPI => EncoderType::DPI,
        other => EncoderType::Unknown(other),
    }
}

#[cfg(target_os = "linux")]
const DRM_IOCTL_MODE_GETRESOURCES: libc::c_ulong =
    drm_iowr(0xA0, std::mem::size_of::<DrmModeCardRes>());
#[cfg(target_os = "linux")]
const DRM_IOCTL_MODE_GETENCODER: libc::c_ulong =
    drm_iowr(0xA6, std::mem::size_of::<DrmModeGetEncoder>());

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
struct DrmModeGetEncoder {
    encoder_id: u32,
    encoder_type: u32,
    crtc_id: u32,
    possible_crtcs: u32,
    possible_clones: u32,
}

/// Enumerates all encoders on the given DRM device.
#[cfg(target_os = "linux")]
pub fn enumerate_encoders(device: &DrmDevice) -> Result<Vec<EncoderInfo>> {
    let encoder_ids = enumerate_encoder_ids(device.fd())?;
    let mut encoders = Vec::with_capacity(encoder_ids.len());

    for encoder_id in encoder_ids {
        if let Some(encoder) = enumerate_encoder(device.fd(), encoder_id)? {
            encoders.push(encoder);
        }
    }

    Ok(encoders)
}

/// On non-Linux platforms, encoder enumeration returns an empty list.
#[cfg(not(target_os = "linux"))]
pub fn enumerate_encoders(_device: &DrmDevice) -> Result<Vec<EncoderInfo>> {
    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_encoder_ids(fd: i32) -> Result<Vec<u32>> {
    for _ in 0..ENUMERATION_RETRY_LIMIT {
        let mut resources = DrmModeCardRes::default();
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETRESOURCES,
            "DRM_IOCTL_MODE_GETRESOURCES",
            &mut resources,
        )?;

        let encoder_count = resources.count_encoders as usize;
        if encoder_count == 0 {
            return Ok(Vec::new());
        }

        let mut encoder_ids = vec![0u32; encoder_count];
        let mut populated = DrmModeCardRes {
            encoder_id_ptr: slice_ptr_u64(&mut encoder_ids),
            count_encoders: encoder_ids.len() as u32,
            ..Default::default()
        };
        drm_ioctl(
            fd,
            DRM_IOCTL_MODE_GETRESOURCES,
            "DRM_IOCTL_MODE_GETRESOURCES",
            &mut populated,
        )?;

        if (populated.count_encoders as usize) <= encoder_ids.len() {
            encoder_ids.truncate(populated.count_encoders as usize);
            encoder_ids.retain(|encoder_id| *encoder_id != 0);
            return Ok(encoder_ids);
        }
    }

    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn enumerate_encoder(fd: i32, encoder_id: u32) -> Result<Option<EncoderInfo>> {
    let mut request = DrmModeGetEncoder {
        encoder_id,
        ..Default::default()
    };
    drm_ioctl(
        fd,
        DRM_IOCTL_MODE_GETENCODER,
        "DRM_IOCTL_MODE_GETENCODER",
        &mut request,
    )?;

    Ok(Some(EncoderInfo {
        id: EncoderId(request.encoder_id),
        encoder_type: encoder_type_from_raw(request.encoder_type),
        crtc_id: (request.crtc_id != 0).then(|| CrtcId(request.crtc_id)),
        possible_crtcs: request.possible_crtcs,
        possible_clones: request.possible_clones,
    }))
}
