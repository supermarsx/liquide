//! GBM (Generic Buffer Manager) integration for the LiquiDE compositor.
//!
//! Provides GPU buffer allocation and DMA-BUF export for zero-copy
//! frame presentation via DRM/KMS scanout. When running as a standalone
//! compositor from TTY, this crate bridges the GPU renderer's output
//! directly to the display hardware.

pub mod buffer;
pub mod device;
pub mod error;
pub mod format;
pub mod surface;

pub use buffer::{GbmBuffer, GbmBufferFlags};
pub use device::GbmDevice;
pub use error::{GbmError, Result};
pub use format::{DrmFourcc, DrmModifier};
pub use surface::GbmSurface;

#[cfg(test)]
mod tests;
