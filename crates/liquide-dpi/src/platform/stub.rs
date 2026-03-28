//! Stub platform DPI detector for unsupported platforms.
//!
//! Always returns 1.0x (96 DPI).

use crate::monitor::MonitorId;
use crate::scale::DpiScale;

/// Stub platform DPI detector.
pub struct PlatformDpi;

impl PlatformDpi {
    pub fn new() -> Self {
        Self
    }

    pub fn system_dpi(&self) -> DpiScale {
        DpiScale::identity()
    }

    pub fn primary_monitor_dpi(&self) -> DpiScale {
        DpiScale::identity()
    }

    pub fn enumerate_monitor_dpis(&self) -> Vec<(MonitorId, DpiScale)> {
        vec![(0, DpiScale::identity())]
    }
}

impl Default for PlatformDpi {
    fn default() -> Self {
        Self::new()
    }
}
