//! Theme presets for the LiquiDE desktop environment.

pub mod liquid_glass;
pub mod macos_dark;
pub mod midday;
pub mod night;
pub mod sunset;

/// Available theme preset IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ThemePreset {
    /// macOS-style full-dark theme with a graphite accent (default).
    MacosDark,
    /// Standard Liquid Glass dark theme.
    LiquidGlass,
    /// OLED-optimized dark theme with true black backgrounds.
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
            ThemePreset::MacosDark => macos_dark::CSS,
            ThemePreset::LiquidGlass => liquid_glass::CSS,
            ThemePreset::Night => night::CSS,
            ThemePreset::Sunset => sunset::CSS,
            ThemePreset::Midday => midday::CSS,
        }
    }

    /// Parse a theme ID string to a preset.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "macos-dark" | "default" => Some(ThemePreset::MacosDark),
            "liquid-glass" | "standard" => Some(ThemePreset::LiquidGlass),
            "night" => Some(ThemePreset::Night),
            "sunset" => Some(ThemePreset::Sunset),
            "midday" => Some(ThemePreset::Midday),
            _ => None,
        }
    }

    /// The theme preset ID string.
    pub fn id(&self) -> &'static str {
        match self {
            ThemePreset::MacosDark => "macos-dark",
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
            ThemePreset::MacosDark => "macos_dark.css",
            ThemePreset::LiquidGlass => "liquid_glass.css",
            ThemePreset::Night => "night.css",
            ThemePreset::Sunset => "sunset.css",
            ThemePreset::Midday => "midday.css",
        }
    }

    /// All presets, for exhaustive iteration in the drift-guard tests.
    const ALL_PRESETS: &[ThemePreset] = &[
        ThemePreset::MacosDark,
        ThemePreset::LiquidGlass,
        ThemePreset::Night,
        ThemePreset::Sunset,
        ThemePreset::Midday,
    ];

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
        for &preset in ALL_PRESETS {
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
        for &preset in ALL_PRESETS {
            assert!(
                preset.css().len() > 1000,
                "embedded CSS for '{}' looks truncated ({} bytes)",
                preset.id(),
                preset.css().len(),
            );
        }
    }

    // ── macOS Dark foundation (t172-e1) ──────────────────────────────────

    /// `macos-dark` is registered: its ID resolves to a preset, round-trips, and
    /// is the theme the `default` alias points at (it is the shipped default).
    #[test]
    fn macos_dark_is_registered_and_default() {
        let preset = ThemePreset::from_id("macos-dark")
            .expect("`macos-dark` must resolve to a registered preset");
        assert_eq!(preset, ThemePreset::MacosDark);
        assert_eq!(preset.id(), "macos-dark");

        // The `default` alias now points at macos-dark (it is the shipped default).
        assert_eq!(
            ThemePreset::from_id("default"),
            Some(ThemePreset::MacosDark),
            "the `default` theme alias must resolve to macos-dark"
        );

        // The embedded CSS is non-empty and matches the on-disk asset name.
        assert!(preset.css().len() > 1000, "macos-dark CSS looks truncated");
        assert_eq!(asset_file(ThemePreset::MacosDark), "macos_dark.css");
    }
}

#[cfg(test)]
mod macos_dark_token_tests {
    use super::*;
    use liquide_style_engine::engine::ViewportSize;
    use liquide_style_engine::StyleEngine;
    use liquide_theme_css::{ThemeEngine, ThemeParser};

    /// On-disk asset paths (read at runtime, like the drift-guard, so the test
    /// drives the SHIPPED cascade — base variables then the theme override).
    fn asset(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/themes")
            .join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
    }

    /// Build the production cascade the DE uses: `variables.css` (the `:root`
    /// token defaults) THEN `macos_dark.css` (the theme override layer). Tokens
    /// are read back through the SAME `resolve_variable` path the shell uses for
    /// `--menu-item-height`, focus rings, etc. (see `accessors.rs`).
    fn cascade() -> StyleEngine {
        let mut engine = StyleEngine::new(ViewportSize { width: 1280.0, height: 720.0 }, 16.0);
        engine.add_stylesheet(&asset("variables.css"));
        engine.add_stylesheet(macos_dark::CSS);
        engine
    }

    /// The KEY tokens resolve to GRAPHITE through the real cascade — and the
    /// theme layer OVERRIDES the blue variables.css default. No-fake-green: if
    /// the theme failed to load/override, the resolved accent would be the blue
    /// `#3b82f6` from variables.css and these assertions would fail.
    #[test]
    fn macos_dark_accent_resolves_graphite_not_blue() {
        let engine = cascade();

        let accent = engine
            .resolve_variable("--accent-color")
            .and_then(|v| v.as_color().copied())
            .expect("--accent-color must resolve to a color in the cascade");
        assert_eq!(
            (accent.r, accent.g, accent.b),
            (142, 142, 147),
            "accent must be graphite #8e8e93, got {accent:?}"
        );
        // Graphite is near-neutral: the channels are tightly clustered (a true
        // gray), unlike a saturated accent where one channel dominates.
        let max = accent.r.max(accent.g).max(accent.b) as i32;
        let min = accent.r.min(accent.g).min(accent.b) as i32;
        assert!(
            max - min <= 8,
            "graphite accent must be near-neutral (low channel spread), got {accent:?}"
        );
        assert!(
            !(accent.r == 59 && accent.g == 130 && accent.b == 246),
            "accent must NOT be the blue variables.css default #3b82f6 — the \
             macos_dark theme layer must override it"
        );

        // Focus ring follows graphite (one token recolors every widget ring).
        let ring = engine
            .resolve_variable("--widget-focus-ring")
            .and_then(|v| v.as_color().copied())
            .expect("--widget-focus-ring must resolve");
        assert_eq!(
            (ring.r, ring.g, ring.b),
            (142, 142, 147),
            "focus ring must follow the graphite accent"
        );
    }

    /// The surface tokens resolve DARK through the cascade (layered grays, not a
    /// light/blue surface). Drives the same path the chrome consumes.
    #[test]
    fn macos_dark_surface_tokens_resolve_dark() {
        let engine = cascade();
        let bg = engine
            .resolve_variable("--bg-primary")
            .and_then(|v| v.as_color().copied())
            .expect("--bg-primary must resolve");
        assert!(
            bg.r < 64 && bg.g < 64 && bg.b < 64,
            "primary background must be dark, got {bg:?}"
        );
    }

    /// The UI font stack is the system-UI / Inter family (the SF substitute),
    /// not the serif/mono fallback.
    #[test]
    fn macos_dark_ui_font_is_system_ui_inter_stack() {
        let engine = cascade();
        let font = engine
            .resolve_variable("--font-family-ui")
            .expect("--font-family-ui must resolve");
        let text = format!("{font:?}").to_ascii_lowercase();
        assert!(
            text.contains("inter") || text.contains("system-ui"),
            "UI font stack must be the system-UI/Inter family, got {font:?}"
        );
    }

    /// The desktop wallpaper (element rule, what the renderer-painted
    /// `ShellTheme` resolver reads — and what the standalone `DesktopPipeline`
    /// boots with) is DARK. This parses the theme file ALONE, exactly as the
    /// resolver does, proving the literal element rules (not just the tokens)
    /// render dark.
    #[test]
    fn macos_dark_shell_theme_desktop_background_is_dark() {
        let stylesheet = ThemeParser::new()
            .parse_str(macos_dark::CSS)
            .expect("macos_dark.css must parse standalone");
        let theme =
            crate::theme_loader::css_to_shell_theme(&ThemeEngine::new(stylesheet));
        let desk = theme.desktop_background;
        assert!(
            desk.r < 80 && desk.g < 80 && desk.b < 80,
            "macos-dark desktop background must be dark, got {desk:?}"
        );
    }
}
