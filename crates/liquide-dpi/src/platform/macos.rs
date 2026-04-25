//! macOS DPI detection.
//!
//! Uses `NSScreen.backingScaleFactor` via `system_profiler` and `defaults` commands,
//! since we avoid linking Cocoa directly (matching the project's Command-based pattern).

use crate::monitor::MonitorId;
use crate::scale::DpiScale;
use std::process::Command;

/// macOS platform DPI detector.
pub struct PlatformDpi;

impl PlatformDpi {
    /// Create a new platform DPI detector.
    pub fn new() -> Self {
        Self
    }

    /// Get the system DPI scale.
    ///
    /// On macOS this is typically 1.0 (non-Retina) or 2.0 (Retina).
    /// Uses `system_profiler SPDisplaysDataType` to detect Retina displays.
    pub fn system_dpi(&self) -> DpiScale {
        if let Some(scale) = Self::detect_retina_scale() {
            return scale;
        }
        DpiScale::identity()
    }

    /// Get the DPI for the primary monitor.
    pub fn primary_monitor_dpi(&self) -> DpiScale {
        self.system_dpi()
    }

    /// Enumerate monitors and their DPI scales.
    ///
    /// On macOS, parses `system_profiler SPDisplaysDataType` output to find
    /// all connected displays and their Retina status.
    pub fn enumerate_monitor_dpis(&self) -> Vec<(MonitorId, DpiScale)> {
        if let Some(monitors) = Self::enumerate_displays() {
            if !monitors.is_empty() {
                return monitors;
            }
        }
        vec![(0, self.system_dpi())]
    }

    // ── Internal helpers ──────────────────────────────────────────────

    /// Detect Retina scaling from system_profiler.
    fn detect_retina_scale() -> Option<DpiScale> {
        let output = Command::new("system_profiler")
            .arg("SPDisplaysDataType")
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);

        // Look for "Retina" in the display info, or "Resolution" lines.
        // Example:
        //   Resolution: 2560 x 1600 Retina
        //   Resolution: 3840 x 2160
        //   UI Looks like: 1280 x 800

        // If "Retina" appears in the main display section, it's 2x.
        // Some displays have fractional scaling via "UI Looks like" lines.
        let mut found_retina = false;
        let mut actual_width: Option<f32> = None;
        let mut ui_width: Option<f32> = None;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Resolution:") {
                if trimmed.contains("Retina") {
                    found_retina = true;
                }
                // Extract the first number as the horizontal resolution.
                let parts: Vec<&str> = trimmed
                    .trim_start_matches("Resolution:")
                    .trim()
                    .split_whitespace()
                    .collect();
                if let Some(w) = parts.first().and_then(|s| s.parse::<f32>().ok()) {
                    actual_width = Some(w);
                }
            }
            if trimmed.starts_with("UI Looks like:") {
                let parts: Vec<&str> = trimmed
                    .trim_start_matches("UI Looks like:")
                    .trim()
                    .split_whitespace()
                    .collect();
                if let Some(w) = parts.first().and_then(|s| s.parse::<f32>().ok()) {
                    ui_width = Some(w);
                }
            }
        }

        // If we have both actual and UI resolution, compute the scale.
        if let (Some(actual), Some(ui)) = (actual_width, ui_width) {
            if ui > 0.0 {
                let scale = actual / ui;
                if scale > 0.5 {
                    return Some(DpiScale::new(scale));
                }
            }
        }

        // Fallback: Retina keyword means 2x.
        if found_retina {
            return Some(DpiScale::new(2.0));
        }

        None
    }

    /// Enumerate all connected displays from system_profiler.
    fn enumerate_displays() -> Option<Vec<(MonitorId, DpiScale)>> {
        let output = Command::new("system_profiler")
            .arg("SPDisplaysDataType")
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        let mut id: MonitorId = 0;

        // Track state per display block.
        let mut current_retina = false;
        let mut current_actual: Option<f32> = None;
        let mut current_ui: Option<f32> = None;
        let mut in_display = false;

        for line in text.lines() {
            let trimmed = line.trim();

            // A new display block starts with a non-empty, non-indented line
            // that contains "Display" or follows "Displays:".
            if !trimmed.is_empty()
                && !trimmed.starts_with("Resolution")
                && !trimmed.starts_with("UI Looks")
                && !trimmed.starts_with("Display Type")
                && !trimmed.starts_with("Pixel")
                && trimmed.ends_with(':')
                && line.starts_with("        ")
                && !line.starts_with("          ")
            {
                // Flush previous display.
                if in_display {
                    let scale =
                        Self::compute_display_scale(current_retina, current_actual, current_ui);
                    results.push((id, scale));
                    id += 1;
                }
                in_display = true;
                current_retina = false;
                current_actual = None;
                current_ui = None;
            }

            if trimmed.starts_with("Resolution:") {
                in_display = true;
                if trimmed.contains("Retina") {
                    current_retina = true;
                }
                let parts: Vec<&str> = trimmed
                    .trim_start_matches("Resolution:")
                    .trim()
                    .split_whitespace()
                    .collect();
                if let Some(w) = parts.first().and_then(|s| s.parse::<f32>().ok()) {
                    current_actual = Some(w);
                }
            }

            if trimmed.starts_with("UI Looks like:") {
                let parts: Vec<&str> = trimmed
                    .trim_start_matches("UI Looks like:")
                    .trim()
                    .split_whitespace()
                    .collect();
                if let Some(w) = parts.first().and_then(|s| s.parse::<f32>().ok()) {
                    current_ui = Some(w);
                }
            }
        }

        // Flush last display.
        if in_display {
            let scale = Self::compute_display_scale(current_retina, current_actual, current_ui);
            results.push((id, scale));
        }

        Some(results)
    }

    /// Compute scale from parsed display info.
    fn compute_display_scale(
        retina: bool,
        actual_width: Option<f32>,
        ui_width: Option<f32>,
    ) -> DpiScale {
        if let (Some(actual), Some(ui)) = (actual_width, ui_width) {
            if ui > 0.0 {
                let scale = actual / ui;
                if scale > 0.5 {
                    return DpiScale::new(scale);
                }
            }
        }
        if retina {
            DpiScale::new(2.0)
        } else {
            DpiScale::identity()
        }
    }
}

impl Default for PlatformDpi {
    fn default() -> Self {
        Self::new()
    }
}
