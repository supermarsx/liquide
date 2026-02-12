//! Audio routing matrix types (spec section 16.6).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AudioRoutingEntry
// ---------------------------------------------------------------------------

/// A single source-to-target routing connection in the audio routing matrix
/// (spec section 16.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRoutingEntry {
    /// Device identifier of the audio source.
    pub source_device_id: String,
    /// Friendly name of the audio source.
    pub source_name: String,
    /// Device identifier of the routing target.
    pub target_device_id: String,
    /// Friendly name of the routing target.
    pub target_name: String,
    /// Whether audio is actively flowing on this route.
    pub active: bool,
    /// Volume level applied at the routing edge as a percentage (0–100).
    pub volume_percent: f64,
    /// Whether the route is muted.
    pub muted: bool,
}

// ---------------------------------------------------------------------------
// RoutingProfile
// ---------------------------------------------------------------------------

/// A named collection of routing entries that can be saved and recalled
/// (spec section 16.6 – saved routing profiles).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingProfile {
    /// Profile name (e.g., "Production", "Gaming", "Meeting").
    pub name: String,
    /// Routing entries belonging to this profile.
    pub entries: Vec<AudioRoutingEntry>,
    /// Whether this profile is currently active.
    pub active: bool,
    /// Optional description of the routing profile.
    pub description: Option<String>,
}
