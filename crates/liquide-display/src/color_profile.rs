//! ICC color profile management.
//!
//! Provides loading, caching, and application of ICC color profiles,
//! plus gamma ramp computation for common color spaces.

use crate::display::DisplayId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Color profile
// ---------------------------------------------------------------------------

/// A named color profile with transfer-function parameters.
///
/// Rather than embedding raw ICC binary data, this stores the essential
/// parameters needed to compute gamma ramps and color transforms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorProfile {
    /// Human-readable name (e.g., "sRGB IEC61966-2.1").
    pub name: String,
    /// Color space identifier.
    pub color_space: ColorSpace,
    /// Gamma exponent (2.2 for sRGB-like, 1.8 for some print profiles).
    /// The sRGB transfer function is not a pure gamma, but for display
    /// purposes this is a useful approximation.
    pub gamma: f32,
    /// White point chromaticity (x, y) in CIE 1931. D65 = (0.3127, 0.3290).
    pub white_point: (f32, f32),
    /// Red primary chromaticity (x, y).
    pub red_primary: (f32, f32),
    /// Green primary chromaticity (x, y).
    pub green_primary: (f32, f32),
    /// Blue primary chromaticity (x, y).
    pub blue_primary: (f32, f32),
}

/// Well-known color space identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorSpace {
    /// Standard RGB (IEC 61966-2-1). Gamma ~2.2, D65 white point.
    Srgb,
    /// Display P3 (wide gamut, used by modern displays). Gamma ~2.2, D65.
    DisplayP3,
    /// Adobe RGB (1998). Gamma ~2.2, D65.
    AdobeRgb,
    /// Generic / unknown profile.
    Custom,
}

impl Default for ColorSpace {
    fn default() -> Self {
        ColorSpace::Srgb
    }
}

impl ColorProfile {
    /// Built-in sRGB profile.
    pub fn srgb() -> Self {
        Self {
            name: "sRGB IEC61966-2.1".to_string(),
            color_space: ColorSpace::Srgb,
            gamma: 2.2,
            white_point: (0.3127, 0.3290),
            red_primary: (0.64, 0.33),
            green_primary: (0.30, 0.60),
            blue_primary: (0.15, 0.06),
        }
    }

    /// Built-in Display P3 profile.
    pub fn display_p3() -> Self {
        Self {
            name: "Display P3".to_string(),
            color_space: ColorSpace::DisplayP3,
            gamma: 2.2,
            white_point: (0.3127, 0.3290),
            red_primary: (0.68, 0.32),
            green_primary: (0.265, 0.69),
            blue_primary: (0.15, 0.06),
        }
    }

    /// Built-in Adobe RGB (1998) profile.
    pub fn adobe_rgb() -> Self {
        Self {
            name: "Adobe RGB (1998)".to_string(),
            color_space: ColorSpace::AdobeRgb,
            gamma: 2.2,
            white_point: (0.3127, 0.3290),
            red_primary: (0.64, 0.33),
            green_primary: (0.21, 0.71),
            blue_primary: (0.15, 0.06),
        }
    }

    /// Create a custom profile.
    pub fn custom(
        name: impl Into<String>,
        gamma: f32,
        white_point: (f32, f32),
        red: (f32, f32),
        green: (f32, f32),
        blue: (f32, f32),
    ) -> Self {
        Self {
            name: name.into(),
            color_space: ColorSpace::Custom,
            gamma,
            white_point,
            red_primary: red,
            green_primary: green,
            blue_primary: blue,
        }
    }

    /// Compute a 256-entry gamma ramp for this profile.
    ///
    /// Each entry maps an input value (0-255, linear index) to an output
    /// value (0.0-1.0) using the profile's gamma curve.
    ///
    /// For sRGB, this uses the piecewise transfer function:
    ///   - Linear region: C/12.92 for C <= 0.04045
    ///   - Gamma region:  ((C + 0.055) / 1.055)^2.4
    ///
    /// For other profiles, a simple power-law gamma is used.
    pub fn gamma_ramp(&self) -> [f32; 256] {
        let mut ramp = [0.0f32; 256];
        for i in 0..256 {
            let c = i as f32 / 255.0;
            ramp[i] = match self.color_space {
                ColorSpace::Srgb => srgb_to_linear(c),
                _ => c.powf(self.gamma),
            };
        }
        ramp
    }

    /// Compute the inverse gamma ramp (linear -> display).
    pub fn inverse_gamma_ramp(&self) -> [f32; 256] {
        let mut ramp = [0.0f32; 256];
        for i in 0..256 {
            let c = i as f32 / 255.0;
            ramp[i] = match self.color_space {
                ColorSpace::Srgb => linear_to_srgb(c),
                _ => c.powf(1.0 / self.gamma),
            };
        }
        ramp
    }

    /// Compute the CIE XYZ to RGB 3x3 matrix for this profile.
    ///
    /// This is computed from the primaries and white point using the
    /// standard colorimetric equations. Row-major order.
    pub fn xyz_to_rgb_matrix(&self) -> [f64; 9] {
        let (xr, yr) = (self.red_primary.0 as f64, self.red_primary.1 as f64);
        let (xg, yg) = (self.green_primary.0 as f64, self.green_primary.1 as f64);
        let (xb, yb) = (self.blue_primary.0 as f64, self.blue_primary.1 as f64);
        let (xw, yw) = (self.white_point.0 as f64, self.white_point.1 as f64);

        if yr.abs() < 1e-10 || yg.abs() < 1e-10 || yb.abs() < 1e-10 || yw.abs() < 1e-10 {
            return [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        }

        // Chromaticity to XYZ (Y=1 for each primary).
        let xr_xyz = xr / yr;
        let zr_xyz = (1.0 - xr - yr) / yr;
        let xg_xyz = xg / yg;
        let zg_xyz = (1.0 - xg - yg) / yg;
        let xb_xyz = xb / yb;
        let zb_xyz = (1.0 - xb - yb) / yb;

        // White point XYZ (Y=1).
        let xw_xyz = xw / yw;
        let zw_xyz = (1.0 - xw - yw) / yw;

        // Solve for S (scaling factors): [Xr Xg Xb; 1 1 1; Zr Zg Zb] * S = [Xw; 1; Zw]
        // Using Cramer's rule for 3x3 system.
        let m = [
            [xr_xyz, xg_xyz, xb_xyz],
            [1.0, 1.0, 1.0],
            [zr_xyz, zg_xyz, zb_xyz],
        ];
        let rhs = [xw_xyz, 1.0, zw_xyz];
        let det = det3(&m);
        if det.abs() < 1e-12 {
            return [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        }

        let sr = det3(&[
            [rhs[0], m[0][1], m[0][2]],
            [rhs[1], m[1][1], m[1][2]],
            [rhs[2], m[2][1], m[2][2]],
        ]) / det;
        let sg = det3(&[
            [m[0][0], rhs[0], m[0][2]],
            [m[1][0], rhs[1], m[1][2]],
            [m[2][0], rhs[2], m[2][2]],
        ]) / det;
        let sb = det3(&[
            [m[0][0], m[0][1], rhs[0]],
            [m[1][0], m[1][1], rhs[1]],
            [m[2][0], m[2][1], rhs[2]],
        ]) / det;

        // RGB to XYZ matrix (M).
        let rgb_to_xyz = [
            sr * xr_xyz, sg * xg_xyz, sb * xb_xyz,
            sr,           sg,           sb,
            sr * zr_xyz, sg * zg_xyz, sb * zb_xyz,
        ];

        // Invert to get XYZ to RGB.
        invert3(&rgb_to_xyz)
    }
}

/// sRGB transfer function: sRGB -> linear.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Inverse sRGB transfer function: linear -> sRGB.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// 3x3 determinant.
fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Invert a row-major 3x3 matrix. Returns identity if singular.
fn invert3(m: &[f64; 9]) -> [f64; 9] {
    let a = [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]];
    let det = det3(&a);
    if det.abs() < 1e-12 {
        return [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    }
    let inv_det = 1.0 / det;
    [
        (a[1][1] * a[2][2] - a[1][2] * a[2][1]) * inv_det,
        (a[0][2] * a[2][1] - a[0][1] * a[2][2]) * inv_det,
        (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * inv_det,
        (a[1][2] * a[2][0] - a[1][0] * a[2][2]) * inv_det,
        (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * inv_det,
        (a[0][2] * a[1][0] - a[0][0] * a[1][2]) * inv_det,
        (a[1][0] * a[2][1] - a[1][1] * a[2][0]) * inv_det,
        (a[0][1] * a[2][0] - a[0][0] * a[2][1]) * inv_det,
        (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * inv_det,
    ]
}

// ---------------------------------------------------------------------------
// ICC profile store
// ---------------------------------------------------------------------------

/// Per-monitor color profile assignment.
#[derive(Debug, Clone)]
struct MonitorAssignment {
    monitor_id: DisplayId,
    profile: ColorProfile,
}

/// Store for loading and caching ICC color profiles, and assigning them
/// to monitors.
#[derive(Debug, Clone, Default)]
pub struct IccProfileStore {
    /// Named profiles available for assignment.
    profiles: HashMap<String, ColorProfile>,
    /// Per-monitor assignments.
    assignments: Vec<MonitorAssignment>,
}

impl IccProfileStore {
    /// Create a new store pre-loaded with the built-in profiles (sRGB, Display P3, Adobe RGB).
    pub fn new() -> Self {
        let mut profiles = HashMap::new();
        let srgb = ColorProfile::srgb();
        let p3 = ColorProfile::display_p3();
        let adobe = ColorProfile::adobe_rgb();
        profiles.insert(srgb.name.clone(), srgb);
        profiles.insert(p3.name.clone(), p3);
        profiles.insert(adobe.name.clone(), adobe);
        Self {
            profiles,
            assignments: Vec::new(),
        }
    }

    /// Add (or replace) a named profile.
    pub fn add_profile(&mut self, profile: ColorProfile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Get a profile by name.
    pub fn get_profile(&self, name: &str) -> Option<&ColorProfile> {
        self.profiles.get(name)
    }

    /// List all available profile names.
    pub fn profile_names(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    /// Number of profiles in the store.
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Assign a profile to a monitor. If the profile name is not found in the
    /// store, returns `false`.
    pub fn apply_profile(&mut self, monitor_id: DisplayId, profile_name: &str) -> bool {
        if let Some(profile) = self.profiles.get(profile_name) {
            // Remove existing assignment for this monitor.
            self.assignments.retain(|a| a.monitor_id != monitor_id);
            self.assignments.push(MonitorAssignment {
                monitor_id,
                profile: profile.clone(),
            });
            true
        } else {
            false
        }
    }

    /// Get the profile currently assigned to a monitor.
    pub fn get_monitor_profile(&self, monitor_id: DisplayId) -> Option<&ColorProfile> {
        self.assignments
            .iter()
            .find(|a| a.monitor_id == monitor_id)
            .map(|a| &a.profile)
    }

    /// Remove the profile assignment for a monitor. Returns `true` if an
    /// assignment existed.
    pub fn remove_assignment(&mut self, monitor_id: DisplayId) -> bool {
        let before = self.assignments.len();
        self.assignments.retain(|a| a.monitor_id != monitor_id);
        self.assignments.len() < before
    }

    /// Compute the gamma ramp for a specific monitor. If no profile is
    /// assigned, returns the sRGB ramp as default.
    pub fn gamma_ramp_for(&self, monitor_id: DisplayId) -> [f32; 256] {
        if let Some(assignment) = self.assignments.iter().find(|a| a.monitor_id == monitor_id) {
            assignment.profile.gamma_ramp()
        } else {
            ColorProfile::srgb().gamma_ramp()
        }
    }
}
