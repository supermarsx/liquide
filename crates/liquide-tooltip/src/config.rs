//! Tooltip configuration.

use serde::{Deserialize, Serialize};

/// Tooltip display configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TooltipConfig {
    /// Delay before showing the tooltip (milliseconds).
    pub show_delay_ms: u32,
    /// Duration to keep the tooltip visible (milliseconds, 0 = indefinite).
    pub display_duration_ms: u32,
    /// Fade-in duration (milliseconds).
    pub fade_in_ms: u32,
    /// Fade-out duration (milliseconds).
    pub fade_out_ms: u32,
    /// Offset from the cursor/widget in pixels.
    pub offset_x: f32,
    pub offset_y: f32,
    /// Maximum tooltip width before word-wrapping.
    pub max_width: f32,
    /// Padding inside the tooltip box.
    pub padding: f32,
    /// Corner radius of the tooltip box.
    pub corner_radius: f32,
    /// Whether tooltips are globally enabled.
    pub enabled: bool,
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self {
            show_delay_ms: 500,
            display_duration_ms: 5000,
            fade_in_ms: 150,
            fade_out_ms: 100,
            offset_x: 0.0,
            offset_y: 8.0,
            max_width: 300.0,
            padding: 8.0,
            corner_radius: 6.0,
            enabled: true,
        }
    }
}
