//! Theme presets for the LiquiDE desktop environment.

pub mod liquid_glass;
pub mod midday;
pub mod night;
pub mod sunset;

/// Available theme preset IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ThemePreset {
    /// Standard Liquid Glass dark theme.
    LiquidGlass,
    /// OLED-optimized dark theme with true black backgrounds (default).
    Night,
    /// Warm dark theme with amber/orange tones.
    Sunset,
    /// Tarnished white light theme.
    Midday,
}

#[allow(dead_code)]
impl ThemePreset {
    /// Get the CSS string for this theme preset.
    pub fn css(&self) -> &'static str {
        match self {
            ThemePreset::LiquidGlass => liquid_glass::CSS,
            ThemePreset::Night => night::CSS,
            ThemePreset::Sunset => sunset::CSS,
            ThemePreset::Midday => midday::CSS,
        }
    }

    /// Parse a theme ID string to a preset.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "liquid-glass" | "standard" => Some(ThemePreset::LiquidGlass),
            "night" | "default" => Some(ThemePreset::Night),
            "sunset" => Some(ThemePreset::Sunset),
            "midday" => Some(ThemePreset::Midday),
            _ => None,
        }
    }

    /// The theme preset ID string.
    pub fn id(&self) -> &'static str {
        match self {
            ThemePreset::LiquidGlass => "liquid-glass",
            ThemePreset::Night => "night",
            ThemePreset::Sunset => "sunset",
            ThemePreset::Midday => "midday",
        }
    }
}

#[cfg(test)]
mod drift_guard_tests {
    use super::*;

    /// The on-disk asset filename for a preset.
    fn asset_file(preset: ThemePreset) -> &'static str {
        match preset {
            ThemePreset::LiquidGlass => "liquid_glass.css",
            ThemePreset::Night => "night.css",
            ThemePreset::Sunset => "sunset.css",
            ThemePreset::Midday => "midday.css",
        }
    }

    /// Read the on-disk theme asset at runtime (NOT via `include_str!`, so this
    /// is a genuine, independent read — if the embedded copy is hand-maintained
    /// and drifts, the comparison must fail).
    fn read_disk_asset(preset: ThemePreset) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/themes")
            .join(asset_file(preset));
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
    }

    /// Each embedded theme's `css()` MUST be byte-for-byte identical to the
    /// corresponding on-disk asset. This is the single-source guarantee: the
    /// embedded fallback (used when disk assets fail to load) renders the same
    /// DE as the on-disk theme, never a stale copy.
    #[test]
    fn embedded_theme_css_equals_on_disk_asset() {
        for preset in [
            ThemePreset::LiquidGlass,
            ThemePreset::Night,
            ThemePreset::Sunset,
            ThemePreset::Midday,
        ] {
            let embedded = preset.css();
            let disk = read_disk_asset(preset);
            assert_eq!(
                embedded,
                disk.as_str(),
                "embedded CSS for theme '{}' has drifted from on-disk asset {} \
                 (embedded {} bytes vs disk {} bytes); the embedded fallback must \
                 be single-sourced from the asset via include_str!",
                preset.id(),
                asset_file(preset),
                embedded.len(),
                disk.len(),
            );
        }
    }

    /// Spot-check that the embedded copy is actually substantial (guards against
    /// an empty/truncated include resolving to an empty string).
    #[test]
    fn embedded_theme_css_is_nonempty() {
        for preset in [
            ThemePreset::LiquidGlass,
            ThemePreset::Night,
            ThemePreset::Sunset,
            ThemePreset::Midday,
        ] {
            assert!(
                preset.css().len() > 1000,
                "embedded CSS for '{}' looks truncated ({} bytes)",
                preset.id(),
                preset.css().len(),
            );
        }
    }
}
