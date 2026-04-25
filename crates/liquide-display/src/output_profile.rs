//! Display output profiles — save and restore named monitor configurations.
//!
//! Extends the base `profile` module with built-in profile presets,
//! auto-detection heuristics, and profile store management.

use crate::display::{DisplayInfo, Resolution, Rotation};
use crate::profile::{DisplayConfig, DisplayProfile};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// OutputProfile: enriched profile with metadata
// ---------------------------------------------------------------------------

/// An enriched display profile with additional metadata for auto-detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputProfile {
    /// The underlying display profile (connectors, resolutions, positions).
    pub profile: DisplayProfile,
    /// Tags used for heuristic matching (e.g., "laptop", "docked", "presentation").
    pub tags: Vec<String>,
    /// Priority: higher values are preferred when multiple profiles match.
    pub priority: u32,
    /// Whether this profile was auto-generated (vs. user-created).
    pub auto_generated: bool,
}

impl OutputProfile {
    /// Create a new output profile wrapping a `DisplayProfile`.
    pub fn new(profile: DisplayProfile, tags: Vec<String>, priority: u32) -> Self {
        Self {
            profile,
            tags,
            priority,
            auto_generated: false,
        }
    }

    /// Check whether this profile has a particular tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ---------------------------------------------------------------------------
// Built-in profiles
// ---------------------------------------------------------------------------

/// Generate the built-in "laptop-only" profile for a single built-in display.
pub fn builtin_laptop_only(connector: &str, resolution: Resolution) -> OutputProfile {
    let profile = DisplayProfile {
        name: "laptop-only".to_string(),
        displays: vec![DisplayConfig {
            connector: connector.to_string(),
            resolution,
            refresh_rate: 60.0,
            position: (0, 0),
            rotation: Rotation::Normal,
            scale: 1.0,
            primary: true,
            enabled: true,
        }],
    };
    let mut op = OutputProfile::new(profile, vec!["laptop".into()], 10);
    op.auto_generated = true;
    op
}

/// Generate a "docked" profile for a laptop with one external monitor.
///
/// The external monitor is placed to the right of the laptop display.
pub fn builtin_docked(
    laptop_connector: &str,
    laptop_res: Resolution,
    external_connector: &str,
    external_res: Resolution,
) -> OutputProfile {
    let ext_x = (laptop_res.width as f32 / 1.0).round() as i32; // scale=1
    let profile = DisplayProfile {
        name: "docked".to_string(),
        displays: vec![
            DisplayConfig {
                connector: laptop_connector.to_string(),
                resolution: laptop_res,
                refresh_rate: 60.0,
                position: (0, 0),
                rotation: Rotation::Normal,
                scale: 1.0,
                primary: false,
                enabled: true,
            },
            DisplayConfig {
                connector: external_connector.to_string(),
                resolution: external_res,
                refresh_rate: 60.0,
                position: (ext_x, 0),
                rotation: Rotation::Normal,
                scale: 1.0,
                primary: true,
                enabled: true,
            },
        ],
    };
    let mut op = OutputProfile::new(profile, vec!["docked".into(), "external".into()], 20);
    op.auto_generated = true;
    op
}

/// Generate a "presentation" profile (mirror mode) for laptop + projector.
pub fn builtin_presentation(
    laptop_connector: &str,
    laptop_res: Resolution,
    projector_connector: &str,
) -> OutputProfile {
    // Mirror: both at (0,0), use the laptop's native resolution for both.
    let profile = DisplayProfile {
        name: "presentation".to_string(),
        displays: vec![
            DisplayConfig {
                connector: laptop_connector.to_string(),
                resolution: laptop_res,
                refresh_rate: 60.0,
                position: (0, 0),
                rotation: Rotation::Normal,
                scale: 1.0,
                primary: true,
                enabled: true,
            },
            DisplayConfig {
                connector: projector_connector.to_string(),
                resolution: laptop_res,
                refresh_rate: 60.0,
                position: (0, 0),
                rotation: Rotation::Normal,
                scale: 1.0,
                primary: false,
                enabled: true,
            },
        ],
    };
    let mut op = OutputProfile::new(profile, vec!["presentation".into(), "mirror".into()], 15);
    op.auto_generated = true;
    op
}

// ---------------------------------------------------------------------------
// Profile store
// ---------------------------------------------------------------------------

/// In-memory store of output profiles with auto-detection support.
#[derive(Debug, Clone, Default)]
pub struct ProfileStore {
    profiles: Vec<OutputProfile>,
}

impl ProfileStore {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    /// Add a profile to the store.
    pub fn add(&mut self, profile: OutputProfile) {
        self.profiles.push(profile);
    }

    /// Remove a profile by name. Returns `true` if found and removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.profile.name != name);
        self.profiles.len() < before
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&OutputProfile> {
        self.profiles.iter().find(|p| p.profile.name == name)
    }

    /// List all profile names.
    pub fn names(&self) -> Vec<&str> {
        self.profiles
            .iter()
            .map(|p| p.profile.name.as_str())
            .collect()
    }

    /// Number of stored profiles.
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// Auto-detect the best matching profile for the currently connected displays.
    ///
    /// First tries exact connector-set matching (via `detect_matching_profile`).
    /// If multiple exact matches exist, returns the one with the highest priority.
    /// Returns `None` if no profile matches.
    pub fn detect(&self, connected: &[DisplayInfo]) -> Option<&OutputProfile> {
        let mut current_connectors: Vec<&str> = connected
            .iter()
            .filter(|d| d.connected)
            .map(|d| d.connector.as_str())
            .collect();
        current_connectors.sort();

        let mut matches: Vec<&OutputProfile> = self
            .profiles
            .iter()
            .filter(|op| op.profile.connector_set() == current_connectors)
            .collect();

        matches.sort_by(|a, b| b.priority.cmp(&a.priority));
        matches.first().copied()
    }

    /// Save the current display arrangement as a new named profile.
    pub fn save_current(&mut self, name: &str, displays: &[DisplayInfo], tags: Vec<String>) {
        let profile = DisplayProfile::save_current(name, displays);
        self.profiles.push(OutputProfile::new(profile, tags, 50));
    }

    /// Serialize the entire store to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.profiles)
    }

    /// Deserialize a store from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let profiles: Vec<OutputProfile> = serde_json::from_str(json)?;
        Ok(Self { profiles })
    }
}
