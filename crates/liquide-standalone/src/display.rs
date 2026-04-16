//! DRM/KMS display output management for standalone compositor.
//!
//! Wraps liquide-drm and liquide-gbm to provide frame presentation
//! to physical monitors.

use liquide_drm::connector::ConnectorInfo;
use liquide_drm::mode::DrmMode;

/// Information about a connected display output.
#[derive(Debug, Clone)]
pub struct OutputInfo {
    /// Connector ID from DRM.
    pub connector_id: u32,
    /// Human-readable name (e.g. "HDMI-A-1").
    pub name: String,
    /// Selected display mode.
    pub mode: DrmMode,
    /// Physical width in mm.
    pub physical_width_mm: u32,
    /// Physical height in mm.
    pub physical_height_mm: u32,
    /// Whether this is the primary output.
    pub primary: bool,
}

/// Display output manager for presenting frames via DRM/KMS.
pub struct DisplayOutput {
    outputs: Vec<OutputInfo>,
}

impl DisplayOutput {
    /// Create a new display output manager with no outputs.
    pub fn new() -> Self {
        Self {
            outputs: Vec::new(),
        }
    }

    /// Add a connected output.
    pub fn add_output(&mut self, output: OutputInfo) {
        self.outputs.push(output);
    }

    /// Get all connected outputs.
    pub fn outputs(&self) -> &[OutputInfo] {
        &self.outputs
    }

    /// Get the primary output (first output if none marked primary).
    pub fn primary(&self) -> Option<&OutputInfo> {
        self.outputs.iter().find(|o| o.primary).or(self.outputs.first())
    }
}

impl Default for DisplayOutput {
    fn default() -> Self {
        Self::new()
    }
}
