//! DRM/KMS display output backend for LiquiDE standalone compositor.
//!
//! Provides Linux Direct Rendering Manager / Kernel Mode Setting support,
//! enabling direct framebuffer-to-monitor scanout when running from a TTY.

pub mod atomic;
pub mod connector;
pub mod crtc;
pub mod device;
pub mod error;
pub mod framebuffer;
pub mod mode;
pub mod pageflip;

pub use atomic::{AtomicFlags, AtomicRequest, PropertyChange};
pub use connector::{
    ConnectorId, ConnectorInfo, ConnectorStatus, ConnectorType, SubpixelOrder,
    enumerate_connectors,
};
pub use crtc::{CrtcId, CrtcInfo, enumerate_crtcs};
pub use device::DrmDevice;
pub use error::{DrmError, Result};
pub use framebuffer::DrmFramebuffer;
pub use mode::{DrmMode, ModeFlags, preferred_mode};
pub use pageflip::{PageFlipEvent, PageFlipFlags, request_page_flip, wait_vblank};

#[cfg(test)]
mod tests;
