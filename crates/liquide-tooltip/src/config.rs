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
            show_delay_ms: 100,
            display_duration_ms: 5000,
            fade_in_ms: 50,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the hover-jank reduction (t77-A1): the default show delay and
    /// fade-in duration must stay at the responsive 100ms / 50ms values.
    ///
    /// This test FAILS if either constant regresses to the old laggy
    /// 500ms / 150ms (or any other value), so the responsiveness win cannot
    /// be silently reverted.
    #[test]
    fn default_show_delay_and_fade_in_stay_responsive() {
        let cfg = TooltipConfig::default();
        assert_eq!(
            cfg.show_delay_ms, 100,
            "tooltip show_delay_ms regressed (jank source); expected 100ms, was {}ms",
            cfg.show_delay_ms
        );
        assert_eq!(
            cfg.fade_in_ms, 50,
            "tooltip fade_in_ms regressed (jank source); expected 50ms, was {}ms",
            cfg.fade_in_ms
        );
        // Guard against accidentally re-introducing the old 650ms total budget.
        assert!(
            cfg.show_delay_ms + cfg.fade_in_ms <= 150,
            "hover->fully-visible budget too high ({}ms); should be <=150ms",
            cfg.show_delay_ms + cfg.fade_in_ms
        );
    }
}
