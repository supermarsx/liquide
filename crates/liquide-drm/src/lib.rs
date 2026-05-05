//! DRM/KMS display output backend for LiquiDE standalone compositor.
//!
//! Provides Linux Direct Rendering Manager / Kernel Mode Setting support,
//! enabling direct framebuffer-to-monitor scanout when running from a TTY.

pub mod atomic;
pub mod connector;
pub mod crtc;
pub mod device;
pub mod encoder;
pub mod error;
pub mod framebuffer;
#[cfg(any(test, target_os = "linux"))]
pub(crate) mod ioctl;
pub mod mode;
pub mod pageflip;
pub mod plane;
pub mod resources;

pub use atomic::{AtomicFlags, AtomicRequest, ObjectId, PropertyChange, PropertyId};
pub use connector::{
    ConnectorId, ConnectorInfo, ConnectorStatus, ConnectorType, SubpixelOrder, enumerate_connectors,
};
pub use crtc::{CrtcId, CrtcInfo, enumerate_crtcs, select_crtc_for_connector};
pub use encoder::{EncoderId, EncoderInfo, EncoderType, enumerate_encoders};
pub use device::DrmDevice;
pub use error::{DrmError, Result};
pub use framebuffer::{DrmFramebuffer, DumbBuffer, Fourcc, FramebufferId};
pub use mode::{
    DrmMode, ModeFlags, closest_refresh_mode, highest_resolution_mode, match_mode_by_dimensions,
    preferred_mode,
};
pub use plane::{PlaneId, PlaneInfo, PlaneType, enumerate_planes};
pub use resources::DrmResources;
pub use pageflip::{
    DrmEvent, PageFlipEvent, PageFlipFlags, PresentRequest, UnknownDrmEvent, VblankEvent,
    parse_drm_events, request_page_flip, wait_vblank,
};
#[cfg(target_os = "linux")]
pub use pageflip::{drain_pending_events, drain_pending_events_from_fd};

#[cfg(test)]
mod tests;
