//! CSS theme loading and integration with ShellTheme
//!
//! ## Architecture
//!
//! Themes are organized in [`themes/`](themes) as separate modules per preset.
//! Each theme exports a `pub const CSS: &str` with the full stylesheet.
//!
//! The theme engine loads CSS and converts it to [`ShellTheme`] via property queries.

#![allow(dead_code)]

use liquide_compositor::pixel::Color;
use liquide_theme_css::prelude::PropertyValue;
use liquide_theme_css::value::{ColorStop, Gradient};
use liquide_theme_css::{Result as CssResult, ThemeEngine, ThemeParser};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::theme::ShellTheme;
use crate::themes;

/// Load a CSS theme and convert it to ShellTheme
pub fn load_css_theme<P: AsRef<Path>>(path: P) -> CssResult<ShellTheme> {
    let (theme, _engine) = load_css_theme_with_engine(path)?;
    Ok(theme)
}

/// Load a CSS theme and return both the ShellTheme and the engine (as Arc)
/// so the caller can keep the engine alive for CSS queries.
pub fn load_css_theme_with_engine<P: AsRef<Path>>(
    path: P,
) -> CssResult<(ShellTheme, std::sync::Arc<ThemeEngine>)> {
    let parser = ThemeParser::new();
    let stylesheet = parser.parse_file(path)?;
    let engine = std::sync::Arc::new(ThemeEngine::new(stylesheet));

    info!(
        "Loaded CSS theme with {} rules",
        engine.stylesheet().rule_count()
    );

    let theme = css_to_shell_theme(&engine);
    Ok((theme, engine))
}

/// Convert CSS theme engine to ShellTheme
pub fn css_to_shell_theme(engine: &ThemeEngine) -> ShellTheme {
    ShellTheme {
        // Desktop — spec §2.1: #1C1C2E
        desktop_background: query_color(engine, "desktop-background", &[], &[], "background")
            .unwrap_or_else(|| Color::new(28, 28, 46, 255)),

        // Window decorations
        window_title_bar_focused: query_color(
            engine,
            "window",
            &["focused".into()],
            &[],
            "titlebar-background",
        )
        .or_else(|| query_color(engine, "titlebar", &["focused".into()], &[], "background"))
        .unwrap_or_else(|| Color::new(255, 255, 255, 26)), // rgba(255,255,255,0.10)

        window_title_bar_unfocused: query_color(engine, "window", &[], &[], "titlebar-background")
            .or_else(|| query_color(engine, "titlebar", &[], &[], "background"))
            .unwrap_or_else(|| Color::new(255, 255, 255, 20)), // rgba(255,255,255,0.08)

        window_title_text: query_color(engine, "titlebar", &[], &[], "color")
            .unwrap_or_else(|| Color::new(255, 255, 255, 255)),

        window_border_focused: query_color(
            engine,
            "window",
            &["focused".into()],
            &[],
            "border-color",
        )
        .unwrap_or_else(|| Color::new(255, 255, 255, 51)), // rgba(255,255,255,0.20)

        window_border_unfocused: query_color(engine, "window", &[], &[], "border-color")
            .unwrap_or_else(|| Color::new(255, 255, 255, 31)), // rgba(255,255,255,0.12)

        window_shadow: query_color(engine, "window", &[], &[], "box-shadow-color")
            .unwrap_or_else(|| Color::new(0, 0, 0, 77)), // rgba(0,0,0,0.30)

        window_glass_tint: query_color(engine, "window", &[], &[], "glass-tint")
            .or_else(|| query_color(engine, "window", &[], &[], "background"))
            .unwrap_or_else(|| Color::new(30, 30, 50, 179)), // rgba(30,30,50,0.70)

        // Dock
        dock_glass_tint: query_color(engine, "dock", &[], &[], "background")
            .unwrap_or_else(|| Color::new(30, 30, 50, 179)), // rgba(30,30,50,0.70)

        dock_border: query_color(engine, "dock", &[], &[], "border-top-color")
            .or_else(|| query_color(engine, "dock", &[], &[], "border-color"))
            .unwrap_or_else(|| Color::new(255, 255, 255, 15)), // rgba(255,255,255,0.06)

        dock_item_active: query_color(engine, "dock-item", &["active".into()], &[], "color")
            .unwrap_or_else(|| Color::new(255, 255, 255, 255)),

        dock_item_inactive: query_color(engine, "dock-item", &[], &[], "color")
            .unwrap_or_else(|| Color::new(255, 255, 255, 179)), // rgba(255,255,255,0.70)

        dock_hover_highlight: query_color(
            engine,
            "dock-item",
            &[],
            &["hover".into()],
            "background",
        )
        .unwrap_or_else(|| Color::new(255, 255, 255, 31)), // rgba(255,255,255,0.12)

        // Status bar
        status_bar_glass_tint: query_color(engine, "statusbar", &[], &[], "background")
            .or_else(|| query_color(engine, "status-bar", &[], &[], "background"))
            .unwrap_or_else(|| Color::new(20, 20, 40, 217)), // rgba(20,20,40,0.85)

        status_bar_border: query_color(engine, "statusbar", &[], &[], "border-bottom-color")
            .or_else(|| query_color(engine, "status-bar", &[], &[], "border-color"))
            .unwrap_or_else(|| Color::new(255, 255, 255, 15)), // rgba(255,255,255,0.06)

        status_bar_text: query_color(engine, "statusbar", &[], &[], "color")
            .or_else(|| query_color(engine, "status-bar", &[], &[], "color"))
            .unwrap_or_else(|| Color::new(255, 255, 255, 255)),

        status_bar_connected: query_color(
            engine,
            "status-indicator",
            &["connected".into()],
            &[],
            "color",
        )
        .unwrap_or_else(|| Color::new(48, 209, 88, 255)), // #30D158

        status_bar_degraded: query_color(
            engine,
            "status-indicator",
            &["degraded".into()],
            &[],
            "color",
        )
        .unwrap_or_else(|| Color::new(255, 214, 10, 255)), // #FFD60A

        status_bar_notification_active: query_color(
            engine,
            "notification-indicator",
            &["active".into()],
            &[],
            "color",
        )
        .unwrap_or_else(|| Color::new(255, 69, 58, 255)), // #FF453A

        status_bar_notification_inactive: query_color(
            engine,
            "notification-indicator",
            &[],
            &[],
            "color",
        )
        .unwrap_or_else(|| Color::new(100, 210, 255, 153)), // rgba(100,210,255,0.60)

        status_bar_tray: query_color(engine, "status-tray", &[], &[], "background")
            .unwrap_or_else(|| Color::new(255, 255, 255, 26)), // rgba(255,255,255,0.10)

        // Launcher
        launcher_overlay: query_color(engine, "launcher-overlay", &[], &[], "background")
            .unwrap_or_else(|| Color::new(0, 0, 0, 102)), // rgba(0,0,0,0.40)

        launcher_glass_tint: query_color(engine, "launcher", &[], &[], "background")
            .unwrap_or_else(|| Color::new(20, 20, 40, 217)), // rgba(20,20,40,0.85)

        launcher_search_bar: query_color(engine, "launcher-search", &[], &[], "background")
            .or_else(|| query_color(engine, "launcher", &[], &[], "input-background"))
            .unwrap_or_else(|| Color::new(255, 255, 255, 20)), // rgba(255,255,255,0.08)

        launcher_item_selected: query_color(
            engine,
            "launcher-item",
            &["selected".into()],
            &[],
            "background",
        )
        .unwrap_or_else(|| Color::new(0, 122, 255, 77)), // rgba(0,122,255,0.30)

        launcher_item_normal: query_color(engine, "launcher-item", &[], &[], "background")
            .unwrap_or_else(|| Color::new(0, 0, 0, 0)), // transparent

        // Notifications
        notification_glass_tint: query_color(engine, "notification", &[], &[], "background")
            .unwrap_or_else(|| Color::new(40, 40, 60, 230)), // rgba(40,40,60,0.90)

        // Cursor
        cursor_color: query_color(engine, "cursor", &[], &[], "color")
            .unwrap_or_else(|| Color::new(255, 255, 255, 255)),

        // Cursor size multiplier — themes set `cursor { scale: 1.5 }` (or a
        // `--cursor-scale` custom property on the `cursor` rule) to grow/shrink
        // the cursor. Defaults to 1.0 (historic size) when unset.
        cursor_scale: query_number(engine, "cursor", &[], &[], "scale")
            .or_else(|| query_number(engine, "cursor", &[], &[], "--cursor-scale"))
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(1.0),

        // Context / session menus
        menu_item_hover: query_color(engine, "menu-item", &[], &["hover".into()], "background")
            .unwrap_or_else(|| Color::new(0, 122, 255, 77)), // rgba(0,122,255,0.30)

        menu_separator: query_color(engine, "menu-separator", &[], &[], "background")
            .unwrap_or_else(|| Color::new(255, 255, 255, 31)), // rgba(255,255,255,0.12)

        // Loading overlay
        loading_overlay: query_color(engine, "loading-overlay", &[], &[], "background")
            .unwrap_or_else(|| Color::new(20, 20, 40, 217)), // rgba(20,20,40,0.85)

        loading_glass_tint: query_color(engine, "loading-panel", &[], &[], "background")
            .unwrap_or_else(|| Color::new(40, 40, 60, 230)), // rgba(40,40,60,0.90)

        loading_text: query_color(engine, "loading-panel", &[], &[], "color")
            .unwrap_or_else(|| Color::new(255, 255, 255, 255)),

        // Window content
        window_content_background: query_color(engine, "window-content", &[], &[], "background")
            .unwrap_or_else(|| Color::new(30, 30, 50, 242)), // rgba(30,30,50,0.95)

        // App-specific colors
        app_settings_sidebar_item: query_color(
            engine,
            "app-settings",
            &["sidebar-item".into()],
            &[],
            "background",
        )
        .unwrap_or_else(|| Color::new(255, 255, 255, 20)), // rgba(255,255,255,0.08)

        app_terminal_background: query_color(engine, "app-terminal", &[], &[], "background")
            .unwrap_or_else(|| Color::new(18, 18, 30, 255)), // rgb(18,18,30)

        app_terminal_text: query_color(engine, "app-terminal", &[], &[], "color")
            .unwrap_or_else(|| Color::new(100, 220, 100, 255)),

        app_browser_urlbar: query_color(
            engine,
            "app-browser",
            &["urlbar".into()],
            &[],
            "background",
        )
        .unwrap_or_else(|| Color::new(255, 255, 255, 26)), // rgba(255,255,255,0.10)
    }
}

/// Query a color from the CSS theme engine
fn query_color(
    engine: &ThemeEngine,
    element: &str,
    classes: &[String],
    pseudo_classes: &[String],
    property: &str,
) -> Option<Color> {
    let styles = engine.query(element, classes, pseudo_classes).ok()?;
    let value = styles.get(property)?;

    // Try to extract color from PropertyValue
    match value {
        PropertyValue::Color(css_color) => {
            // Convert CSS color to compositor Color
            Some(Color::new(
                css_color.r,
                css_color.g,
                css_color.b,
                css_color.a,
            ))
        }
        PropertyValue::Gradient(gradient) => representative_gradient_color(gradient),
        _ => {
            debug!("Property {} is not a color", property);
            None
        }
    }
}

/// Query a unitless number (or length resolved to px) from the CSS theme engine.
///
/// Used for scalar theme tokens such as the cursor size multiplier. Accepts a
/// bare `Number` (e.g. `scale: 1.5`) or a `Length`/`calc()` resolved against a
/// 16px base and a 1280×720 reference viewport (cursor scale is viewport-
/// independent, so the reference is only a fallback for length units).
fn query_number(
    engine: &ThemeEngine,
    element: &str,
    classes: &[String],
    pseudo_classes: &[String],
    property: &str,
) -> Option<f32> {
    let styles = engine.query(element, classes, pseudo_classes).ok()?;
    let value = styles.get(property)?;
    value
        .as_number()
        .or_else(|| value.resolve_px(16.0, 1280.0, 720.0))
}

fn representative_gradient_color(gradient: &Gradient) -> Option<Color> {
    let stops = match gradient {
        Gradient::Linear { stops, .. }
        | Gradient::Radial { stops, .. }
        | Gradient::Conic { stops, .. }
        | Gradient::RepeatingLinear { stops, .. }
        | Gradient::RepeatingRadial { stops, .. }
        | Gradient::RepeatingConic { stops, .. } => stops,
    };

    average_stops(stops)
}

fn average_stops(stops: &[ColorStop]) -> Option<Color> {
    if stops.is_empty() {
        return None;
    }

    let len = stops.len() as u32;
    let (r, g, b, a) = stops.iter().fold((0u32, 0u32, 0u32, 0u32), |acc, stop| {
        (
            acc.0 + stop.color.r as u32,
            acc.1 + stop.color.g as u32,
            acc.2 + stop.color.b as u32,
            acc.3 + stop.color.a as u32,
        )
    });

    Some(Color::new(
        (r / len) as u8,
        (g / len) as u8,
        (b / len) as u8,
        (a / len) as u8,
    ))
}

/// Get the default theme CSS (Night theme).
///
/// The default theme is Night — OLED-optimized with true black backgrounds
/// and restrained glass effects for maximum battery efficiency.
pub fn default_theme_css() -> &'static str {
    themes::night::CSS
}

/// Create the default Liquid Glass dark theme CSS (spec §2.1 "Standard")
///
/// This is the default Liquid Glass theme — cool blue tones, full glass effects,
/// translucent surfaces. Based on spec-design.md §2.1 color palette.
pub fn default_liquid_glass_css() -> &'static str {
    themes::liquid_glass::CSS
}

/// Backward-compatible alias — returns the standard Liquid Glass dark theme.
pub fn default_nord_css() -> &'static str {
    default_liquid_glass_css()
}

/// Create the Night OLED-optimized theme CSS (spec-theme-night.md)
///
/// True black backgrounds, restrained glass (10px blur), no noise/specular.
/// Optimized for OLED displays and bandwidth-constrained connections.
pub fn night_css() -> &'static str {
    themes::night::CSS
}

/// Create the Sunset warm dark theme CSS (spec-theme-sunset.md)
///
/// Amber/orange tones, warm glass tint, full effects. The golden-hour theme.
pub fn sunset_css() -> &'static str {
    themes::sunset::CSS
}

/// Create the Midday tarnished-white light theme CSS (spec-theme-midday.md)
///
/// Warm off-white surfaces, dark text, deep teal accent, light-mode glass.
pub fn midday_css() -> &'static str {
    themes::midday::CSS
}

// ── External CSS file loading ─────────────────────────────────────────────

/// Map a theme preset ID to its external `.css` filename (without path).
fn theme_id_to_filename(theme_id: &str) -> Option<&'static str> {
    match theme_id {
        "liquid-glass" | "standard" => Some("liquid_glass.css"),
        "night" | "default" => Some("night.css"),
        "sunset" => Some("sunset.css"),
        "midday" => Some("midday.css"),
        _ => None,
    }
}

/// Get the embedded (fallback) CSS for a theme preset ID.
fn embedded_css_for_id(theme_id: &str) -> Option<&'static str> {
    themes::ThemePreset::from_id(theme_id).map(|p| p.css())
}

/// Load a theme by ID, preferring an external `.css` file from `assets_dir`.
///
/// Resolution order:
/// 1. Try `{assets_dir}/themes/{theme_name}.css` on disk.
/// 2. Fall back to the embedded `const CSS` from the `themes/` module.
///
/// Returns the CSS text as an owned `String`. If neither source is
/// available, returns `None`.
pub fn resolve_theme_css(theme_id: &str, assets_dir: &std::path::Path) -> Option<String> {
    // 1. Try external file
    if let Some(filename) = theme_id_to_filename(theme_id) {
        let css_path = assets_dir.join("themes").join(filename);
        if css_path.is_file() {
            match std::fs::read_to_string(&css_path) {
                Ok(css) => {
                    info!("Loaded external theme CSS from {}", css_path.display());
                    return Some(css);
                }
                Err(e) => {
                    warn!(
                        "Failed to read external theme {}: {}, falling back to embedded",
                        css_path.display(),
                        e
                    );
                }
            }
        }
    }

    // 2. Fall back to embedded CSS
    embedded_css_for_id(theme_id).map(|css| {
        info!("Using embedded CSS for theme '{}'", theme_id);
        css.to_string()
    })
}

/// Load a theme into a [`liquide_style_engine::StyleEngine`], with optional
/// user overrides.
///
/// * Tries external CSS file first, falls back to embedded.
/// * Loads `{config_dir}/custom.css` last (if it exists) so user rules win.
pub fn load_theme_into_engine(
    engine: &mut liquide_style_engine::StyleEngine,
    theme_id: &str,
    assets_dir: &std::path::Path,
    config_dir: Option<&std::path::Path>,
) {
    // Load theme CSS (external → embedded fallback)
    if let Some(css) = resolve_theme_css(theme_id, assets_dir) {
        engine.add_stylesheet(&css);
    } else {
        warn!(
            "Theme '{}' not found externally or embedded; using Night fallback",
            theme_id
        );
        engine.add_stylesheet(default_theme_css());
    }

    // Load user overrides last
    if let Some(cfg) = config_dir {
        engine.load_user_overrides(cfg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_liquid_glass_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(default_liquid_glass_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);

        let theme = css_to_shell_theme(&engine);

        // All Liquid Glass CSS uses literal values (no CSS variables),
        // so every property should parse fully.
        let dock_styles = engine.query("dock", &[], &[]).unwrap();
        let dock_bg = dock_styles.get("background");
        assert!(
            dock_bg.is_some(),
            "dock background should be parsed from literal rgba"
        );

        // Desktop background is derived from the gradient in the single-sourced
        // on-disk asset (assets/themes/liquid_glass.css), embedded via include_str!.
        assert_eq!(theme.desktop_background.r, 28);
        assert_eq!(theme.desktop_background.g, 28);
        assert_eq!(theme.desktop_background.b, 46);
    }

    /// t65-s3 regression: the dock item label must be `display: none` so the
    /// four labels do not stack/overlap along the dock baseline (the label is a
    /// hover tooltip, not always-on text). Without this rule the labels render
    /// crammed under the icons (the captured "Files Terminal Browser Settings"
    /// overlap regression).
    #[test]
    fn liquid_glass_hides_dock_item_label() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(default_liquid_glass_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);

        let label = engine
            .query("dock-item-label", &[], &[])
            .expect("dock-item-label rule must resolve");
        assert_eq!(
            label.get("display").and_then(|v| v.as_string()),
            Some("none"),
            "dock-item-label must be display:none so dock labels do not overlap"
        );
    }

    /// t65-s3 regression: the menu item icon must reserve a fixed-width gutter so
    /// the icon paints in its own column instead of collapsing to 0 width (the
    /// captured empty-icon-gutter regression).
    #[test]
    fn liquid_glass_menu_item_icon_has_fixed_gutter() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(default_liquid_glass_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);

        let icon = engine
            .query("menu-item-icon", &[], &[])
            .expect("menu-item-icon rule must resolve");
        let width = icon
            .get("width")
            .expect("menu-item-icon must set a width")
            .resolve_px(16.0, 1280.0, 720.0)
            .expect("menu-item-icon width must be a length");
        assert!(
            (width - 16.0).abs() < 0.5,
            "menu-item-icon width should reserve a 16px gutter, got {width}"
        );
    }

    /// t65-s3: the dialog now renders through the DOM/CSS pipeline, so the theme
    /// must carry the `dialog*` styling rules (header/title/message/button). A
    /// missing `dialog-title`/`dialog-button` rule was the prior unstyled
    /// (solid-white header, unlabelled button) fallback.
    #[test]
    fn liquid_glass_has_dialog_rules() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(default_liquid_glass_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);

        for selector in ["dialog", "dialog-title", "dialog-message", "dialog-button"] {
            let rule = engine
                .query(selector, &[], &[])
                .unwrap_or_else(|_| panic!("missing `{selector}` rule"));
            assert!(
                rule.iter().next().is_some(),
                "liquid-glass theme must define a non-empty `{selector}` rule for the DOM dialog"
            );
        }
        // The title carries real text color (not an unstyled white fallback).
        let title = engine.query("dialog-title", &[], &[]).unwrap();
        assert!(
            title.get("color").and_then(|v| v.as_color()).is_some(),
            "dialog-title must set a themed text color"
        );
    }

    #[test]
    fn test_gradient_background_contributes_shell_theme_tint() {
        let parser = ThemeParser::new();
        let stylesheet = parser
            .parse_str(
                r#"
                statusbar {
                    background: linear-gradient(180deg, rgba(18, 22, 48, 0.88), rgba(12, 16, 38, 0.82));
                }
            "#,
            )
            .unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        assert_eq!(theme.status_bar_glass_tint.r, 15);
        assert_eq!(theme.status_bar_glass_tint.g, 19);
        assert_eq!(theme.status_bar_glass_tint.b, 43);
        assert!(theme.status_bar_glass_tint.a > 200);
    }

    #[test]
    fn test_night_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(night_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        // Night desktop is a near-black terminal surface; value derived from the
        // gradient in the single-sourced on-disk asset (assets/themes/night.css).
        assert_eq!(theme.desktop_background.r, 11);
        assert_eq!(theme.desktop_background.g, 11);
        assert_eq!(theme.desktop_background.b, 13);
    }

    #[test]
    fn test_sunset_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(sunset_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        // Sunset desktop is warm dark; value derived from the gradient in the
        // single-sourced on-disk asset (assets/themes/sunset.css).
        assert_eq!(theme.desktop_background.r, 47);
        assert_eq!(theme.desktop_background.g, 23);
        assert_eq!(theme.desktop_background.b, 10);
    }

    #[test]
    fn test_midday_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(midday_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        // Midday desktop is tarnished white; value derived from the gradient in
        // the single-sourced on-disk asset (assets/themes/midday.css).
        assert_eq!(theme.desktop_background.r, 242);
        assert_eq!(theme.desktop_background.g, 241);
        assert_eq!(theme.desktop_background.b, 238);
    }

    #[test]
    fn test_theme_preset_from_id() {
        use crate::themes::ThemePreset;

        assert!(ThemePreset::from_id("liquid-glass").is_some());
        assert!(ThemePreset::from_id("night").is_some());
        assert!(ThemePreset::from_id("sunset").is_some());
        assert!(ThemePreset::from_id("midday").is_some());
        assert!(ThemePreset::from_id("default").is_some());
        assert!(ThemePreset::from_id("standard").is_some());
        assert!(ThemePreset::from_id("nonexistent").is_none());
    }

    // ── Cursor appearance is CSS-driven (t95-p2, t86 GAP-4) ──────────────

    /// The on-disk theme asset's `cursor { color; scale }` rule must drive the
    /// resolved `ShellTheme` cursor appearance — proving the cursor APPEARANCE
    /// (not its shape) flows from CSS, not Rust defaults. Drives the REAL
    /// shipped asset through the production resolver.
    #[test]
    fn cursor_appearance_is_css_driven_from_disk_asset() {
        let css = include_str!("../../../assets/themes/liquid_glass.css");
        let parser = ThemeParser::new();
        let stylesheet = parser
            .parse_str(css)
            .expect("liquid_glass.css must parse");
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        // Color comes from `cursor { color: rgba(255,255,255,1.0) }`.
        assert_eq!(
            theme.cursor_color,
            Color::new(255, 255, 255, 255),
            "cursor color must be resolved from the asset's cursor rule"
        );
        // Scale comes from `cursor { scale: 1.0 }` (the shipped default).
        assert!(
            (theme.cursor_scale - 1.0).abs() < f32::EPSILON,
            "cursor scale must be resolved from the asset's cursor rule, got {}",
            theme.cursor_scale
        );
    }

    /// ADVERSARIAL no-fake-green: changing the CSS `cursor { scale }` must
    /// change the resolved `ShellTheme.cursor_scale`. If the resolver ignored
    /// the CSS (e.g. hardcoded 1.0), this differential would fail.
    #[test]
    fn cursor_scale_tracks_the_css_value() {
        let parse = |css: &str| {
            let stylesheet = ThemeParser::new().parse_str(css).unwrap();
            let engine = ThemeEngine::new(stylesheet);
            css_to_shell_theme(&engine).cursor_scale
        };

        let small = parse("cursor { color: rgb(255,255,255); scale: 1.0; }");
        let large = parse("cursor { color: rgb(255,255,255); scale: 2.5; }");

        assert!((small - 1.0).abs() < f32::EPSILON, "scale 1.0 expected, got {small}");
        assert!((large - 2.5).abs() < f32::EPSILON, "scale 2.5 expected, got {large}");
        assert!(
            large > small,
            "a larger CSS cursor scale must yield a larger resolved scale ({large} vs {small})"
        );
    }

    /// A missing or non-positive `cursor { scale }` must fall back to 1.0 so the
    /// cursor never collapses to zero size from a bad theme.
    #[test]
    fn cursor_scale_falls_back_when_unset_or_invalid() {
        let parse = |css: &str| {
            let stylesheet = ThemeParser::new().parse_str(css).unwrap();
            let engine = ThemeEngine::new(stylesheet);
            css_to_shell_theme(&engine).cursor_scale
        };

        // No scale declared → default 1.0.
        let unset = parse("cursor { color: rgb(0,0,0); }");
        assert!((unset - 1.0).abs() < f32::EPSILON, "unset scale must default to 1.0");

        // Zero / negative are rejected → default 1.0.
        let zero = parse("cursor { color: rgb(0,0,0); scale: 0; }");
        assert!((zero - 1.0).abs() < f32::EPSILON, "scale 0 must fall back to 1.0");
    }
}
