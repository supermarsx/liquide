use serde::{Deserialize, Serialize};

/// Night light / blue-light filter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NightLight {
    /// Whether the filter is active.
    pub enabled: bool,
    /// Color temperature in Kelvin (typically 1800K–6500K).
    /// Lower values are warmer (more red/orange).
    pub temperature_kelvin: u32,
    /// Schedule for automatic activation.
    pub schedule: NightLightSchedule,
}

impl Default for NightLight {
    fn default() -> Self {
        Self {
            enabled: false,
            temperature_kelvin: 3400,
            schedule: NightLightSchedule::Manual,
        }
    }
}

impl NightLight {
    /// Create a new NightLight with custom temperature.
    pub fn new(temperature_kelvin: u32) -> Self {
        Self {
            enabled: true,
            temperature_kelvin: temperature_kelvin.clamp(1000, 10000),
            schedule: NightLightSchedule::Manual,
        }
    }

    /// Get the 3x3 color transformation matrix for the current temperature.
    pub fn color_matrix(&self) -> [f32; 9] {
        if !self.enabled {
            return IDENTITY_MATRIX;
        }
        color_temperature_matrix(self.temperature_kelvin)
    }

    /// Check if the night light should be active at the given time
    /// (hours 0-23, minutes 0-59).
    pub fn is_active_at(&self, hour: u8, minute: u8) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.schedule {
            NightLightSchedule::Manual => true,
            NightLightSchedule::SunsetSunrise { latitude, longitude } => {
                // Simple sunrise/sunset approximation.
                let (sunrise_h, sunrise_m, sunset_h, sunset_m) =
                    approximate_sun_times(*latitude, *longitude);
                let now = hour as u16 * 60 + minute as u16;
                let sunrise = sunrise_h as u16 * 60 + sunrise_m as u16;
                let sunset = sunset_h as u16 * 60 + sunset_m as u16;
                // Active from sunset to sunrise (overnight).
                if sunset < sunrise {
                    // Shouldn't happen (sunset before sunrise) but handle gracefully.
                    now >= sunset || now < sunrise
                } else {
                    now >= sunset || now < sunrise
                }
            }
            NightLightSchedule::Custom {
                start_hour,
                start_min,
                end_hour,
                end_min,
            } => {
                let now = hour as u16 * 60 + minute as u16;
                let start = *start_hour as u16 * 60 + *start_min as u16;
                let end = *end_hour as u16 * 60 + *end_min as u16;
                if start <= end {
                    // Same-day range: e.g. 20:00–23:00.
                    now >= start && now < end
                } else {
                    // Overnight range: e.g. 22:00–06:00.
                    now >= start || now < end
                }
            }
        }
    }
}

/// Schedule for automatic night light activation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NightLightSchedule {
    /// Always on when enabled (no auto-schedule).
    Manual,
    /// Activate at sunset, deactivate at sunrise based on geographic location.
    SunsetSunrise { latitude: f64, longitude: f64 },
    /// Custom time range.
    Custom {
        start_hour: u8,
        start_min: u8,
        end_hour: u8,
        end_min: u8,
    },
}

/// Identity 3x3 color matrix (no transformation).
const IDENTITY_MATRIX: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

/// Compute a 3x3 color temperature transformation matrix.
///
/// This approximates the color of a black-body radiator at the given Kelvin
/// temperature, then constructs a diagonal matrix that scales R, G, B channels
/// to produce that tint. Based on Tanner Helland's algorithm.
///
/// The matrix is row-major: `[r_scale, 0, 0, 0, g_scale, 0, 0, 0, b_scale]`.
///
/// Temperature range: 1000K (very warm/red) to 10000K+ (very cool/blue).
/// 6500K is roughly "daylight" (identity).
pub fn color_temperature_matrix(kelvin: u32) -> [f32; 9] {
    let (r, g, b) = kelvin_to_rgb(kelvin);
    [r, 0.0, 0.0, 0.0, g, 0.0, 0.0, 0.0, b]
}

/// Convert color temperature in Kelvin to RGB scaling factors (0.0–1.0 each).
///
/// Based on Tanner Helland's approximation of Planckian locus:
/// <http://www.tannerhelland.com/4435/convert-temperature-rgb-algorithm-code/>
fn kelvin_to_rgb(kelvin: u32) -> (f32, f32, f32) {
    let temp = (kelvin as f64 / 100.0).clamp(10.0, 100.0);

    // Red
    let r = if temp <= 66.0 {
        1.0
    } else {
        let r_raw = 329.698727446 * (temp - 60.0).powf(-0.1332047592);
        (r_raw / 255.0).clamp(0.0, 1.0)
    };

    // Green
    let g = if temp <= 66.0 {
        let g_raw = 99.4708025861 * temp.ln() - 161.1195681661;
        (g_raw / 255.0).clamp(0.0, 1.0)
    } else {
        let g_raw = 288.1221695283 * (temp - 60.0).powf(-0.0755148492);
        (g_raw / 255.0).clamp(0.0, 1.0)
    };

    // Blue
    let b = if temp >= 66.0 {
        1.0
    } else if temp <= 19.0 {
        0.0
    } else {
        let b_raw = 138.5177312231 * (temp - 10.0).ln() - 305.0447927307;
        (b_raw / 255.0).clamp(0.0, 1.0)
    };

    (r as f32, g as f32, b as f32)
}

/// Approximate sunrise and sunset times for a given latitude/longitude.
///
/// Returns (sunrise_hour, sunrise_min, sunset_hour, sunset_min) in local solar
/// time. This is a rough approximation using day-of-year = 80 (spring equinox
/// region) for simplicity. A real implementation would use the current date.
fn approximate_sun_times(latitude: f64, _longitude: f64) -> (u8, u8, u8, u8) {
    // Use a simplified model: at equator, sunrise ~6:00, sunset ~18:00.
    // Each degree of latitude shifts sunrise/sunset by ~2.5 minutes near equinoxes.
    let lat_abs = latitude.abs().min(66.0);

    // Day length variation: longer days in summer hemisphere, shorter in winter.
    // At equinox, day length is ~12h everywhere. We approximate for "average" day.
    let half_day_hours = 6.0 + (lat_abs / 90.0) * 0.5; // slight latitude effect

    let solar_noon_h = 12.0; // local solar noon
    let sunrise_h = solar_noon_h - half_day_hours;
    let sunset_h = solar_noon_h + half_day_hours;

    let sr_hour = sunrise_h.floor() as u8;
    let sr_min = ((sunrise_h - sunrise_h.floor()) * 60.0) as u8;
    let ss_hour = sunset_h.floor().min(23.0) as u8;
    let ss_min = ((sunset_h - sunset_h.floor()) * 60.0) as u8;

    (sr_hour, sr_min, ss_hour, ss_min)
}
