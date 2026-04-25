//! X11 / toolkit DPI settings bridge.
//!
//! Generates environment variables and Xft/GDK/Qt settings from a UI scale
//! factor, and can detect the current scale from the environment.

/// X11 and toolkit DPI settings derived from a UI scale factor.
///
/// These settings control how GTK, Qt, and X11 applications interpret
/// DPI for font rendering and UI layout.
#[derive(Debug, Clone, PartialEq)]
pub struct XSettings {
    /// Xft.dpi — the X resource DPI value (e.g. 96, 144, 192).
    /// Used by X11 applications and fontconfig for font sizing.
    pub xft_dpi: u32,
    /// GDK_SCALE — integer scale factor for GDK/GTK3+ (e.g. 1, 2).
    /// GTK only supports integer values here.
    pub gdk_scale: u32,
    /// GDK_DPI_SCALE — fractional correction applied on top of GDK_SCALE.
    /// For example, at 1.5x: GDK_SCALE=2, GDK_DPI_SCALE=0.75 (2 * 0.75 = 1.5).
    pub gdk_dpi_scale: f64,
    /// QT_SCALE_FACTOR — scale factor for Qt applications (supports fractional).
    pub qt_scale_factor: f64,
}

impl XSettings {
    /// Compute toolkit settings from a UI scale factor.
    ///
    /// The mapping follows common Linux desktop conventions:
    ///
    /// | Field          | Formula                           |
    /// |----------------|-----------------------------------|
    /// | xft_dpi        | `round(scale * 96)`               |
    /// | gdk_scale      | `ceil(scale)` (integer only)      |
    /// | gdk_dpi_scale  | `scale / gdk_scale`               |
    /// | qt_scale_factor| `scale`                           |
    pub fn from_ui_scale(scale: f64) -> Self {
        let scale = scale.max(0.5);
        let xft_dpi = (scale * 96.0).round() as u32;
        let gdk_scale = scale.ceil() as u32;
        let gdk_dpi_scale = if gdk_scale > 0 {
            scale / gdk_scale as f64
        } else {
            1.0
        };
        Self {
            xft_dpi,
            gdk_scale,
            gdk_dpi_scale,
            qt_scale_factor: scale,
        }
    }

    /// Generate environment variable key-value pairs for applying these settings.
    ///
    /// Returns variables suitable for passing to `std::env::set_var` or
    /// prepending to a child process's environment.
    pub fn to_env_vars(&self) -> Vec<(String, String)> {
        vec![
            (
                "QT_SCALE_FACTOR".to_string(),
                format!("{}", self.qt_scale_factor),
            ),
            ("GDK_SCALE".to_string(), format!("{}", self.gdk_scale)),
            (
                "GDK_DPI_SCALE".to_string(),
                format!("{:.4}", self.gdk_dpi_scale),
            ),
            ("QT_FONT_DPI".to_string(), format!("{}", self.xft_dpi)),
        ]
    }

    /// Generate the Xft.dpi X resource string (e.g. `"Xft.dpi: 144"`).
    pub fn xft_resource_string(&self) -> String {
        format!("Xft.dpi: {}", self.xft_dpi)
    }

    /// The effective scale factor these settings represent.
    #[inline]
    pub fn effective_scale(&self) -> f64 {
        self.qt_scale_factor
    }
}

impl Default for XSettings {
    fn default() -> Self {
        Self::from_ui_scale(1.0)
    }
}

impl std::fmt::Display for XSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Xft.dpi={}, GDK_SCALE={}, GDK_DPI_SCALE={:.4}, QT_SCALE_FACTOR={}",
            self.xft_dpi, self.gdk_scale, self.gdk_dpi_scale, self.qt_scale_factor
        )
    }
}

/// Attempt to detect the current UI scale factor from environment variables.
///
/// Checks (in order of priority):
/// 1. `GDK_SCALE` * `GDK_DPI_SCALE` (if both set)
/// 2. `QT_SCALE_FACTOR`
/// 3. `GDK_SCALE` alone (integer)
///
/// Returns `None` if no recognized environment variable is set.
pub fn detect_from_env() -> Option<f64> {
    // Try GDK_SCALE * GDK_DPI_SCALE first (most precise for GTK apps).
    if let Ok(gdk_scale_str) = std::env::var("GDK_SCALE") {
        if let Ok(gdk_scale) = gdk_scale_str.parse::<f64>() {
            if let Ok(gdk_dpi_str) = std::env::var("GDK_DPI_SCALE") {
                if let Ok(gdk_dpi) = gdk_dpi_str.parse::<f64>() {
                    let combined = gdk_scale * gdk_dpi;
                    if combined > 0.0 {
                        return Some(combined);
                    }
                }
            }
            // GDK_SCALE alone (integer scale).
            if gdk_scale > 0.0 {
                return Some(gdk_scale);
            }
        }
    }

    // Try QT_SCALE_FACTOR.
    if let Ok(qt_str) = std::env::var("QT_SCALE_FACTOR") {
        if let Ok(qt_scale) = qt_str.parse::<f64>() {
            if qt_scale > 0.0 {
                return Some(qt_scale);
            }
        }
    }

    None
}
