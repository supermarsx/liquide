//! CSS theme loading and integration with ShellTheme
//!
//! ## Architecture
//!
//! Themes are organized in [`themes/`](themes) as separate modules per preset.
//! Each theme exports a `pub const CSS: &str` with the full stylesheet.
//!
//! The theme engine loads CSS and converts it to [`ShellTheme`] via property queries.

use liquide_compositor::pixel::Color;
use liquide_theme_css::prelude::PropertyValue;
use liquide_theme_css::{Result as CssResult, ThemeEngine, ThemeParser};
use std::path::Path;
use tracing::{info, warn};

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
        _ => {
            warn!("Property {} is not a color", property);
            None
        }
    }
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

        // Verify desktop background matches spec: #1C1C2E = rgb(28,28,46)
        assert_eq!(theme.desktop_background.r, 28);
        assert_eq!(theme.desktop_background.g, 28);
        assert_eq!(theme.desktop_background.b, 46);
    }

    #[test]
    fn test_night_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(night_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        // Night desktop is true black: #000000
        assert_eq!(theme.desktop_background.r, 0);
        assert_eq!(theme.desktop_background.g, 0);
        assert_eq!(theme.desktop_background.b, 0);
    }

    #[test]
    fn test_sunset_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(sunset_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        // Sunset desktop is warm dark: #1A1008 = rgb(26,16,8)
        assert_eq!(theme.desktop_background.r, 26);
        assert_eq!(theme.desktop_background.g, 16);
        assert_eq!(theme.desktop_background.b, 8);
    }

    #[test]
    fn test_midday_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(midday_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let theme = css_to_shell_theme(&engine);

        // Midday desktop is tarnished white: #F5F0E8 = rgb(245,240,232)
        assert_eq!(theme.desktop_background.r, 245);
        assert_eq!(theme.desktop_background.g, 240);
        assert_eq!(theme.desktop_background.b, 232);
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
}
