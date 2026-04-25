//! DRM/KMS display output management for standalone compositor.
//!
//! Wraps liquide-drm and liquide-gbm to provide frame presentation
//! to physical monitors.

use std::time::Duration;

use liquide_drm::{ConnectorInfo, mode::DrmMode};

/// Information about a connected display output.
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PrimaryOutputPriority {
    FallbackUsable,
    PreferredMode,
    CurrentMode,
}

impl PrimaryOutputPriority {
    fn from_mode(mode: &DrmMode) -> Self {
        if mode.is_current() {
            Self::CurrentMode
        } else if mode.is_preferred() {
            Self::PreferredMode
        } else {
            Self::FallbackUsable
        }
    }
}

impl OutputInfo {
    /// Frame interval derived from the display's refresh rate.
    pub fn frame_interval(&self) -> Duration {
        let hz = if self.mode.refresh_hz == 0 {
            60
        } else {
            self.mode.refresh_hz
        };
        Duration::from_nanos(1_000_000_000 / u64::from(hz))
    }
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

    /// Build display outputs from enumerated DRM connector metadata.
    pub fn from_connectors(connectors: &[ConnectorInfo]) -> Self {
        let mut outputs = Vec::new();
        let mut primary_index = None;
        let mut primary_priority = None;

        for connector in connectors {
            let Some((output, priority)) = output_info_from_connector(connector) else {
                continue;
            };

            let output_index = outputs.len();
            outputs.push(output);

            let replace_primary = match primary_priority {
                Some(current_priority) => priority > current_priority,
                None => true,
            };
            if replace_primary {
                primary_index = Some(output_index);
                primary_priority = Some(priority);
            }
        }

        if let Some(primary_index) = primary_index {
            outputs[primary_index].primary = true;
        }

        Self { outputs }
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
        self.outputs
            .iter()
            .find(|o| o.primary)
            .or(self.outputs.first())
    }

    /// Returns the target frame interval for the primary display.
    /// Falls back to 60 Hz (≈16.67 ms) if no outputs are connected.
    pub fn target_frame_interval(&self) -> Duration {
        self.primary()
            .map(|o| o.frame_interval())
            .unwrap_or(Duration::from_nanos(16_666_667))
    }
}

impl Default for DisplayOutput {
    fn default() -> Self {
        Self::new()
    }
}

fn output_info_from_connector(
    connector: &ConnectorInfo,
) -> Option<(OutputInfo, PrimaryOutputPriority)> {
    if !connector.is_connected() {
        return None;
    }

    let mode = connector.launchable_mode()?.clone();
    let priority = PrimaryOutputPriority::from_mode(&mode);

    Some((
        OutputInfo {
            connector_id: connector.id.0,
            name: connector.stable_name().to_string(),
            mode,
            physical_width_mm: connector.physical_width_mm,
            physical_height_mm: connector.physical_height_mm,
            primary: false,
        },
        priority,
    ))
}
