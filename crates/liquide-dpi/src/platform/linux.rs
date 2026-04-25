//! Linux DPI detection.
//!
//! Probes multiple sources in order:
//! 1. `GDK_SCALE` environment variable (GNOME/GTK).
//! 2. `GDK_DPI_SCALE` environment variable (fractional GTK scaling).
//! 3. `Xft.dpi` from `xrdb -query` (X11).
//! 4. `WAYLAND_DISPLAY` + `wlr-randr` / `swaymsg` output (Wayland compositors).
//! 5. Falls back to 1.0x if nothing can be determined.

use crate::monitor::MonitorId;
use crate::scale::{DpiScale, STANDARD_DPI};
use std::process::Command;

/// Linux platform DPI detector.
pub struct PlatformDpi;

impl PlatformDpi {
    /// Create a new platform DPI detector.
    pub fn new() -> Self {
        Self
    }

    /// Detect the system DPI scale by probing available sources.
    ///
    /// Tries (in order): `GDK_SCALE`, `GDK_DPI_SCALE`, `QT_SCALE_FACTOR`,
    /// `Xft.dpi`, then Wayland compositor queries.
    pub fn system_dpi(&self) -> DpiScale {
        // 1. GDK_SCALE (integer scaling, GNOME)
        if let Some(scale) = Self::read_env_scale("GDK_SCALE") {
            return scale;
        }

        // 2. GDK_DPI_SCALE (fractional, GTK)
        if let Some(scale) = Self::read_env_scale("GDK_DPI_SCALE") {
            return scale;
        }

        // 3. QT_SCALE_FACTOR
        if let Some(scale) = Self::read_env_scale("QT_SCALE_FACTOR") {
            return scale;
        }

        // 4. Xft.dpi from xrdb
        if let Some(dpi) = Self::read_xft_dpi() {
            return DpiScale::from_dpi(dpi);
        }

        // 5. Wayland compositor output scale
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if let Some(scale) = Self::read_wayland_scale() {
                return scale;
            }
        }

        DpiScale::identity()
    }

    /// Get the DPI for the primary monitor (same as system_dpi on Linux,
    /// unless per-monitor info is available).
    pub fn primary_monitor_dpi(&self) -> DpiScale {
        self.system_dpi()
    }

    /// Enumerate monitors and their DPI scales.
    ///
    /// On Wayland, tries `wlr-randr` or `swaymsg -t get_outputs`.
    /// On X11, tries `xrandr --query`.
    /// Falls back to a single monitor at system DPI.
    pub fn enumerate_monitor_dpis(&self) -> Vec<(MonitorId, DpiScale)> {
        // Try Wayland first.
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if let Some(monitors) = Self::enumerate_wayland_monitors() {
                if !monitors.is_empty() {
                    return monitors;
                }
            }
        }

        // Try X11 xrandr.
        if let Some(monitors) = Self::enumerate_xrandr_monitors() {
            if !monitors.is_empty() {
                return monitors;
            }
        }

        // Fallback.
        vec![(0, self.system_dpi())]
    }

    // ── Internal helpers ──────────────────────────────────────────────

    /// Read a scale factor from an environment variable (expects a numeric value).
    fn read_env_scale(var: &str) -> Option<DpiScale> {
        let val = std::env::var(var).ok()?;
        let factor: f32 = val.trim().parse().ok()?;
        if factor > 0.0 {
            Some(DpiScale::new(factor))
        } else {
            None
        }
    }

    /// Parse `Xft.dpi` from `xrdb -query` output.
    fn read_xft_dpi() -> Option<f32> {
        let output = Command::new("xrdb").arg("-query").output().ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Xft.dpi:") || trimmed.starts_with("Xft.dpi\t") {
                // "Xft.dpi:    144" or "Xft.dpi\t144"
                let value_part = trimmed
                    .trim_start_matches("Xft.dpi:")
                    .trim_start_matches("Xft.dpi")
                    .trim_start_matches('\t')
                    .trim();
                if let Ok(dpi) = value_part.parse::<f32>() {
                    if dpi > 0.0 {
                        return Some(dpi);
                    }
                }
            }
        }
        None
    }

    /// Read scale from Wayland compositor (tries swaymsg, then wlr-randr).
    fn read_wayland_scale() -> Option<DpiScale> {
        // Try swaymsg first (Sway).
        if let Some(scale) = Self::read_swaymsg_scale() {
            return Some(scale);
        }
        // Try wlr-randr (wlroots compositors).
        Self::read_wlr_randr_scale()
    }

    /// Parse scale from `swaymsg -t get_outputs`.
    fn read_swaymsg_scale() -> Option<DpiScale> {
        let output = Command::new("swaymsg")
            .args(["-t", "get_outputs", "--raw"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        // swaymsg outputs JSON. Look for "scale": N.N in the focused output.
        // Simple text parsing to avoid a JSON dependency.
        let text = String::from_utf8_lossy(&output.stdout);
        // Find "focused": true, then the nearest "scale": value.
        let focused_pos = text
            .find("\"focused\":true")
            .or_else(|| text.find("\"focused\": true"))?;

        // Search backwards and forwards for "scale"
        let search_region = &text[..focused_pos.saturating_add(200).min(text.len())];
        Self::extract_json_scale(search_region)
    }

    /// Parse scale from `wlr-randr` output.
    fn read_wlr_randr_scale() -> Option<DpiScale> {
        let output = Command::new("wlr-randr").output().ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        // Look for "Scale: 1.5" or similar.
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Scale:") {
                let val = trimmed.trim_start_matches("Scale:").trim();
                if let Ok(factor) = val.parse::<f32>() {
                    if factor > 0.0 {
                        return Some(DpiScale::new(factor));
                    }
                }
            }
        }
        None
    }

    /// Extract `"scale": <number>` from a JSON-ish text fragment.
    fn extract_json_scale(text: &str) -> Option<DpiScale> {
        let scale_key = "\"scale\":";
        let pos = text.rfind(scale_key)?;
        let after = &text[pos + scale_key.len()..];
        let trimmed = after.trim_start();
        // Read digits/dots until non-numeric.
        let end = trimmed
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(trimmed.len());
        let num_str = &trimmed[..end];
        let factor: f32 = num_str.parse().ok()?;
        if factor > 0.0 {
            Some(DpiScale::new(factor))
        } else {
            None
        }
    }

    /// Enumerate Wayland outputs via swaymsg.
    fn enumerate_wayland_monitors() -> Option<Vec<(MonitorId, DpiScale)>> {
        let output = Command::new("swaymsg")
            .args(["-t", "get_outputs", "--raw"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        let mut id: MonitorId = 0;

        // Simple parser: find all "scale": values in the JSON array.
        let mut search_from = 0;
        let scale_key = "\"scale\":";
        while let Some(pos) = text[search_from..].find(scale_key) {
            let abs_pos = search_from + pos + scale_key.len();
            let after = text[abs_pos..].trim_start();
            let end = after
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(after.len());
            if end > 0 {
                if let Ok(factor) = after[..end].parse::<f32>() {
                    if factor > 0.0 {
                        results.push((id, DpiScale::new(factor)));
                        id += 1;
                    }
                }
            }
            search_from = abs_pos + end;
        }

        Some(results)
    }

    /// Enumerate X11 monitors via xrandr. Parse scale from resolution vs. physical size.
    fn enumerate_xrandr_monitors() -> Option<Vec<(MonitorId, DpiScale)>> {
        let output = Command::new("xrandr").arg("--query").output().ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        let mut id: MonitorId = 0;

        // xrandr output format:
        //   DP-1 connected 2560x1440+0+0 (...) 597mm x 336mm
        // We compute DPI from resolution / physical size.
        for line in text.lines() {
            if !line.contains(" connected") {
                continue;
            }

            let dpi = Self::parse_xrandr_line_dpi(line);
            let scale = match dpi {
                Some(d) if d > 0.0 => DpiScale::from_dpi(d),
                _ => DpiScale::identity(),
            };
            results.push((id, scale));
            id += 1;
        }

        Some(results)
    }

    /// Parse DPI from a single xrandr output line.
    ///
    /// Example: `DP-1 connected 2560x1440+0+0 (...) 597mm x 336mm`
    fn parse_xrandr_line_dpi(line: &str) -> Option<f32> {
        // Find resolution: NNNNxNNNN
        let resolution = line.split_whitespace().find(|w| {
            w.contains('x')
                && w.split('x')
                    .next()
                    .map_or(false, |p| p.chars().all(|c| c.is_ascii_digit()))
        })?;

        // The resolution part might have +offset, e.g. "2560x1440+0+0"
        let res_part = resolution.split('+').next()?;
        let mut dims = res_part.split('x');
        let width_px: f32 = dims.next()?.parse().ok()?;

        // Find physical size: NNNmm x NNNmm
        let mm_pos = line.find("mm x ")?;
        // Scan backwards from mm_pos to find the width in mm.
        let before_mm = &line[..mm_pos];
        let width_mm: f32 = before_mm.split_whitespace().next_back()?.parse().ok()?;

        if width_mm > 0.0 {
            Some(width_px / (width_mm / 25.4))
        } else {
            None
        }
    }
}

impl Default for PlatformDpi {
    fn default() -> Self {
        Self::new()
    }
}
