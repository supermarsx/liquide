//! Workload profiles for benchmark simulation.
//!
//! Each workload profile represents a typical usage pattern with expected
//! performance characteristics.

use serde::{Deserialize, Serialize};

/// A workload profile representing a typical usage pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkloadProfile {
    /// Desktop idle, minimal updates.
    Idle,
    /// Text editing with cursor blink and typing.
    TextEditing,
    /// Web browsing with scrolling and page loads.
    WebBrowsing,
    /// Document viewing with occasional scrolling.
    Document,
    /// Full-screen video playback.
    VideoPlayback,
    /// Multi-window desktop workflow (IDE + terminal + browser).
    DesktopWorkflow,
    /// Dashboard with live-updating charts and graphs.
    Dashboard,
    /// Presentation mode (slides with transitions).
    Presentation,
}

/// All available workload profiles.
pub const ALL: &[WorkloadProfile] = &[
    WorkloadProfile::Idle,
    WorkloadProfile::TextEditing,
    WorkloadProfile::WebBrowsing,
    WorkloadProfile::Document,
    WorkloadProfile::VideoPlayback,
    WorkloadProfile::DesktopWorkflow,
    WorkloadProfile::Dashboard,
    WorkloadProfile::Presentation,
];

impl WorkloadProfile {
    /// Human-readable label for this workload.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::TextEditing => "text-editing",
            Self::WebBrowsing => "web-browsing",
            Self::Document => "document",
            Self::VideoPlayback => "video-playback",
            Self::DesktopWorkflow => "desktop-workflow",
            Self::Dashboard => "dashboard",
            Self::Presentation => "presentation",
        }
    }

    /// Description of what this workload simulates.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Idle => "Desktop idle with minimal screen updates",
            Self::TextEditing => "Text editing with cursor blink and typing",
            Self::WebBrowsing => "Web browsing with scrolling and page loads",
            Self::Document => "Document viewing with occasional scrolling",
            Self::VideoPlayback => "Full-screen video playback at target FPS",
            Self::DesktopWorkflow => "Multi-window workflow (IDE + terminal + browser)",
            Self::Dashboard => "Dashboard with live-updating charts",
            Self::Presentation => "Presentation slides with transitions",
        }
    }

    /// Expected FPS range (min, max) for this workload under good conditions.
    #[must_use]
    pub fn expected_fps_range(&self) -> (u32, u32) {
        match self {
            Self::Idle => (1, 5),
            Self::TextEditing => (15, 30),
            Self::WebBrowsing => (30, 60),
            Self::Document => (10, 30),
            Self::VideoPlayback => (30, 60),
            Self::DesktopWorkflow => (30, 60),
            Self::Dashboard => (15, 30),
            Self::Presentation => (30, 60),
        }
    }

    /// Expected bandwidth range in bytes/sec (min, max) for this workload.
    #[must_use]
    pub fn expected_bandwidth_range(&self) -> (u64, u64) {
        match self {
            Self::Idle => (1_000, 50_000),
            Self::TextEditing => (10_000, 200_000),
            Self::WebBrowsing => (500_000, 5_000_000),
            Self::Document => (50_000, 500_000),
            Self::VideoPlayback => (2_000_000, 20_000_000),
            Self::DesktopWorkflow => (500_000, 8_000_000),
            Self::Dashboard => (200_000, 2_000_000),
            Self::Presentation => (500_000, 5_000_000),
        }
    }

    /// Simulated damage fraction: what portion of the screen changes per frame.
    #[must_use]
    pub fn damage_fraction(&self) -> f64 {
        match self {
            Self::Idle => 0.001,
            Self::TextEditing => 0.02,
            Self::WebBrowsing => 0.3,
            Self::Document => 0.05,
            Self::VideoPlayback => 1.0,
            Self::DesktopWorkflow => 0.15,
            Self::Dashboard => 0.1,
            Self::Presentation => 0.5,
        }
    }
}

impl std::fmt::Display for WorkloadProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Parameters controlling a workload simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadParams {
    /// The workload profile to simulate.
    pub profile: WorkloadProfile,
    /// Duration in seconds.
    pub duration_secs: u64,
    /// Horizontal resolution.
    pub resolution_width: u32,
    /// Vertical resolution.
    pub resolution_height: u32,
    /// Tile size in pixels.
    pub tile_size: u32,
}

impl Default for WorkloadParams {
    fn default() -> Self {
        Self {
            profile: WorkloadProfile::DesktopWorkflow,
            duration_secs: 30,
            resolution_width: 1920,
            resolution_height: 1080,
            tile_size: 64,
        }
    }
}

impl WorkloadParams {
    /// Number of tiles in the horizontal direction.
    #[must_use]
    pub fn tiles_x(&self) -> u32 {
        (self.resolution_width + self.tile_size - 1) / self.tile_size
    }

    /// Number of tiles in the vertical direction.
    #[must_use]
    pub fn tiles_y(&self) -> u32 {
        (self.resolution_height + self.tile_size - 1) / self.tile_size
    }

    /// Total number of tiles.
    #[must_use]
    pub fn total_tiles(&self) -> u32 {
        self.tiles_x() * self.tiles_y()
    }

    /// Number of damaged tiles per frame based on the workload profile.
    #[must_use]
    pub fn damaged_tiles_per_frame(&self) -> u32 {
        let total = self.total_tiles() as f64;
        let damaged = (total * self.profile.damage_fraction()).ceil() as u32;
        damaged.max(1)
    }
}
