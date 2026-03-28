use crate::display::{DisplayInfo, Resolution, Rotation};
use serde::{Deserialize, Serialize};

/// Saved configuration for a single display output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Connector name used to match physical outputs (e.g., "DP-1").
    pub connector: String,
    /// Desired resolution.
    pub resolution: Resolution,
    /// Desired refresh rate in Hz.
    pub refresh_rate: f32,
    /// Position in virtual desktop coordinates.
    pub position: (i32, i32),
    /// Rotation.
    pub rotation: Rotation,
    /// DPI scale factor.
    pub scale: f32,
    /// Whether this output is the primary display.
    pub primary: bool,
    /// Whether this output is enabled.
    pub enabled: bool,
}

impl DisplayConfig {
    /// Create a `DisplayConfig` from a live `DisplayInfo`.
    pub fn from_display(info: &DisplayInfo) -> Self {
        Self {
            connector: info.connector.clone(),
            resolution: info.resolution,
            refresh_rate: info.refresh_rate,
            position: info.position,
            rotation: info.rotation,
            scale: info.scale,
            primary: info.primary,
            enabled: info.enabled,
        }
    }
}

/// A named display profile that stores the full arrangement for a set of
/// monitors. Profiles are matched by the set of connected connectors so
/// that plugging in or docking a laptop can auto-apply the right layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayProfile {
    /// Human-readable name (e.g., "Office Dual-Monitor", "Laptop Only").
    pub name: String,
    /// Per-display configuration.
    pub displays: Vec<DisplayConfig>,
}

impl DisplayProfile {
    /// Snapshot the current arrangement into a named profile.
    pub fn save_current(name: &str, current_displays: &[DisplayInfo]) -> Self {
        Self {
            name: name.to_string(),
            displays: current_displays
                .iter()
                .filter(|d| d.connected)
                .map(DisplayConfig::from_display)
                .collect(),
        }
    }

    /// The set of connector names in this profile.
    pub fn connector_set(&self) -> Vec<&str> {
        let mut connectors: Vec<&str> = self.displays.iter().map(|d| d.connector.as_str()).collect();
        connectors.sort();
        connectors
    }

    /// Apply this profile to a live display list. For each `DisplayConfig`,
    /// finds the matching `DisplayInfo` by connector and updates its settings.
    /// Returns the number of displays successfully matched.
    pub fn apply(&self, displays: &mut [DisplayInfo]) -> usize {
        let mut matched = 0;
        for config in &self.displays {
            if let Some(d) = displays
                .iter_mut()
                .find(|d| d.connector == config.connector && d.connected)
            {
                d.resolution = config.resolution;
                d.refresh_rate = config.refresh_rate;
                d.position = config.position;
                d.rotation = config.rotation;
                d.scale = config.scale;
                d.primary = config.primary;
                d.enabled = config.enabled;
                matched += 1;
            }
        }
        matched
    }

    /// Serialize the profile to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a profile from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Given the currently connected displays and a list of saved profiles,
/// find the first profile whose connector set exactly matches the connected
/// outputs.
pub fn detect_matching_profile<'a>(
    connected_displays: &[DisplayInfo],
    saved_profiles: &'a [DisplayProfile],
) -> Option<&'a DisplayProfile> {
    let mut current_connectors: Vec<&str> = connected_displays
        .iter()
        .filter(|d| d.connected)
        .map(|d| d.connector.as_str())
        .collect();
    current_connectors.sort();

    saved_profiles
        .iter()
        .find(|profile| profile.connector_set() == current_connectors)
}
