//! CSS theme loading and integration with ShellTheme

use liquide_compositor::pixel::Color;
use liquide_theme_css::prelude::PropertyValue;
use liquide_theme_css::{Result as CssResult, ThemeEngine, ThemeParser};
use std::path::Path;
use tracing::{info, warn};

use crate::theme::ShellTheme;

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

/// Create the default Liquid Glass dark theme CSS (spec §2.1 "Standard")
///
/// This is the default Liquid Glass theme — cool blue tones, full glass effects,
/// translucent surfaces. Based on spec-design.md §2.1 color palette.
pub fn default_liquid_glass_css() -> &'static str {
    r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Liquid Glass — Standard Dark
   Preset: liquid-glass (default)
   Spec: spec-design.md §2.1
   ═══════════════════════════════════════════════════════ */

/* Desktop */
desktop-background {
    background: rgb(28, 28, 46);
}

/* Windows */
window {
    background: rgba(30, 30, 50, 0.75);
    border-color: rgba(255, 255, 255, 0.12);
    border-width: 1;
    border-radius: 16;
    box-shadow-color: rgba(0, 0, 0, 0.30);
    glass-tint: rgba(30, 30, 50, 0.70);
}

window.focused {
    border-color: rgba(255, 255, 255, 0.20);
    titlebar-background: rgba(255, 255, 255, 0.10);
}

titlebar {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 255);
    height: 36;
}

titlebar.focused {
    background: rgba(255, 255, 255, 0.10);
}

titlebar-button {
    width: 28;
    height: 28;
    margin-right: 4;
    border-radius: 14;
}

/* Window decoration buttons */
close-button {
    background: rgba(255, 69, 58, 0.80);
    color: rgba(255, 255, 255, 0.94);
}

close-button:hover {
    background: rgba(255, 69, 58, 1.0);
}

maximize-button {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.70);
}

maximize-button:hover {
    background: rgba(255, 255, 255, 0.12);
}

minimize-button {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.70);
}

minimize-button:hover {
    background: rgba(255, 255, 255, 0.12);
}

pin-button {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.70);
}

pin-button:hover {
    background: rgba(255, 255, 255, 0.12);
}

pin-button.active {
    background: rgba(0, 122, 255, 0.70);
    color: rgba(255, 255, 255, 1.0);
}

pin-button.active:hover {
    background: rgba(0, 122, 255, 0.90);
}

/* Dock */
dock {
    background: rgba(30, 30, 50, 0.70);
    border-top-color: rgba(255, 255, 255, 0.06);
    padding: 12;
    border-width: 1;
    blur-radius: 20;
}

dock-item {
    color: rgba(255, 255, 255, 0.70);
}

dock-item.active {
    color: rgba(255, 255, 255, 1.0);
}

dock-item:hover {
    background: rgba(255, 255, 255, 0.12);
}

/* Status Bar */
statusbar {
    background: rgba(20, 20, 40, 0.85);
    border-bottom-color: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 1.0);
    height: 28;
    padding: 8;
    border-width: 1;
    blur-radius: 10;
}

status-indicator.connected {
    color: rgb(48, 209, 88);
}

status-indicator.degraded {
    color: rgb(255, 214, 10);
}

notification-indicator.active {
    color: rgb(255, 69, 58);
}

notification-indicator {
    color: rgba(100, 210, 255, 0.60);
}

status-tray {
    background: rgba(255, 255, 255, 0.10);
}

/* Launcher */
launcher-overlay {
    background: rgba(0, 0, 0, 0.40);
}

launcher {
    background: rgba(20, 20, 40, 0.85);
    width: 60;
    height: 70;
    padding: 16;
    blur-radius: 40;
}

launcher-search {
    background: rgba(255, 255, 255, 0.08);
    height: 36;
}

launcher-item {
    background: transparent;
    height: 40;
    margin-bottom: 4;
}

launcher-item.selected {
    background: rgba(0, 122, 255, 0.30);
}

/* Notifications */
notification {
    background: rgba(40, 40, 60, 0.90);
    width: 320;
    height: 80;
    margin-bottom: 8;
    margin-right: 12;
    margin-top: 32;
    blur-radius: 20;
}

/* Menus */
menu {
    blur-radius: 20;
    border-radius: 10;
    padding: 4;
    height: 28;
}

/* Cursor */
cursor {
    color: rgba(255, 255, 255, 255);
}

menu-item:hover {
    background: rgba(0, 122, 255, 0.30);
}

menu-separator {
    background: rgba(255, 255, 255, 0.12);
}

/* Loading */
loading-overlay {
    background: rgba(20, 20, 40, 0.85);
}

loading-panel {
    background: rgba(40, 40, 60, 0.90);
    color: rgba(255, 255, 255, 255);
}

/* Window Content */
window-content {
    background: rgba(30, 30, 50, 0.95);
}

/* App-specific colors */
app-settings.sidebar-item {
    background: rgba(255, 255, 255, 0.08);
}

app-terminal {
    background: rgb(18, 18, 30);
    color: rgb(100, 220, 100);
}

app-browser.urlbar {
    background: rgba(255, 255, 255, 0.10);
}
"#
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
    r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Night — OLED Dark
   Preset: night
   Spec: spec-theme-night.md
   ═══════════════════════════════════════════════════════ */

/* Desktop */
desktop-background {
    background: rgb(0, 0, 0);
}

/* Windows */
window {
    background: rgba(10, 10, 10, 0.92);
    border-color: rgba(255, 255, 255, 0.10);
    border-width: 1;
    border-radius: 16;
    box-shadow-color: rgba(0, 0, 0, 0.70);
    glass-tint: rgba(10, 10, 10, 0.88);
}

window.focused {
    border-color: rgba(255, 255, 255, 0.18);
    titlebar-background: rgba(12, 12, 12, 0.98);
}

titlebar {
    background: rgba(12, 12, 12, 0.98);
    color: rgba(255, 255, 255, 255);
    height: 36;
}

titlebar.focused {
    background: rgba(14, 14, 14, 0.98);
}

titlebar-button {
    width: 28;
    height: 28;
    margin-right: 4;
    border-radius: 14;
}

/* Close: dimmer red for OLED (avoids bloom) */
close-button {
    background: rgba(255, 69, 58, 0.70);
    color: rgba(255, 255, 255, 0.94);
}

close-button:hover {
    background: rgba(255, 69, 58, 0.85);
}

maximize-button {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.80);
}

maximize-button:hover {
    background: rgba(255, 255, 255, 0.10);
}

minimize-button {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.80);
}

minimize-button:hover {
    background: rgba(255, 255, 255, 0.10);
}

pin-button {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.80);
}

pin-button:hover {
    background: rgba(255, 255, 255, 0.10);
}

pin-button.active {
    background: rgba(10, 132, 255, 0.65);
    color: rgba(255, 255, 255, 1.0);
}

pin-button.active:hover {
    background: rgba(10, 132, 255, 0.85);
}

/* Dock — true black translucent */
dock {
    background: rgba(10, 10, 10, 0.88);
    border-top-color: rgba(255, 255, 255, 0.05);
    padding: 12;
    border-width: 1;
    blur-radius: 10;
}

dock-item {
    color: rgba(255, 255, 255, 0.80);
}

dock-item.active {
    color: rgba(255, 255, 255, 1.0);
}

dock-item:hover {
    background: rgba(255, 255, 255, 0.10);
}

/* Status Bar — near-black */
statusbar {
    background: rgba(0, 0, 0, 0.95);
    border-bottom-color: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 255);
    height: 28;
    padding: 8;
    border-width: 1;
    blur-radius: 5;
}

status-indicator.connected {
    color: rgb(48, 209, 88);
}

status-indicator.degraded {
    color: rgb(255, 214, 10);
}

notification-indicator.active {
    color: rgb(255, 69, 58);
}

notification-indicator {
    color: rgba(100, 210, 255, 0.55);
}

status-tray {
    background: rgba(255, 255, 255, 0.08);
}

/* Launcher — near-black, restrained blur */
launcher-overlay {
    background: rgba(0, 0, 0, 0.60);
}

launcher {
    background: rgba(4, 4, 4, 0.98);
    width: 60;
    height: 70;
    padding: 16;
    blur-radius: 20;
}

launcher-search {
    background: rgba(255, 255, 255, 0.05);
    height: 36;
}

launcher-item {
    background: transparent;
    height: 40;
    margin-bottom: 4;
}

launcher-item.selected {
    background: rgba(10, 132, 255, 0.25);
}

/* Notifications — dark */
notification {
    background: rgba(14, 14, 14, 0.96);
    width: 320;
    height: 80;
    margin-bottom: 8;
    margin-right: 12;
    margin-top: 32;
    blur-radius: 10;
}

/* Menus */
menu {
    blur-radius: 10;
    border-radius: 10;
    padding: 4;
    height: 28;
}

/* Cursor */
cursor {
    color: rgba(255, 255, 255, 255);
}

menu-item:hover {
    background: rgba(10, 132, 255, 0.25);
}

menu-separator {
    background: rgba(255, 255, 255, 0.10);
}

/* Loading */
loading-overlay {
    background: rgba(0, 0, 0, 0.90);
}

loading-panel {
    background: rgba(14, 14, 14, 0.96);
    color: rgba(255, 255, 255, 255);
}

/* Window Content */
window-content {
    background: rgba(10, 10, 10, 0.95);
}

/* App-specific */
app-settings.sidebar-item {
    background: rgba(255, 255, 255, 0.06);
}

app-terminal {
    background: rgb(0, 0, 0);
    color: rgb(80, 220, 80);
}

app-browser.urlbar {
    background: rgba(255, 255, 255, 0.08);
}
"#
}

/// Create the Sunset warm dark theme CSS (spec-theme-sunset.md)
///
/// Amber/orange tones, warm glass tint, full effects. The golden-hour theme.
pub fn sunset_css() -> &'static str {
    r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Sunset — Warm Dark
   Preset: sunset
   Spec: spec-theme-sunset.md
   ═══════════════════════════════════════════════════════ */

/* Desktop */
desktop-background {
    background: rgb(26, 16, 8);
}

/* Windows */
window {
    background: rgba(32, 22, 10, 0.78);
    border-color: rgba(255, 180, 80, 0.12);
    border-width: 1;
    border-radius: 16;
    box-shadow-color: rgba(20, 10, 0, 0.40);
    glass-tint: rgba(32, 22, 10, 0.72);
}

window.focused {
    border-color: rgba(255, 180, 80, 0.22);
    titlebar-background: rgba(40, 28, 14, 0.65);
}

titlebar {
    background: rgba(40, 28, 14, 0.60);
    color: rgba(255, 245, 230, 255);
    height: 36;
}

titlebar.focused {
    background: rgba(40, 28, 14, 0.70);
}

titlebar-button {
    width: 28;
    height: 28;
    margin-right: 4;
    border-radius: 14;
}

/* Close: warm red */
close-button {
    background: rgba(255, 107, 107, 0.75);
    color: rgba(255, 245, 230, 0.94);
}

close-button:hover {
    background: rgba(255, 107, 107, 1.0);
}

maximize-button {
    background: rgba(255, 200, 120, 0.06);
    color: rgba(255, 245, 230, 0.72);
}

maximize-button:hover {
    background: rgba(255, 200, 120, 0.10);
}

minimize-button {
    background: rgba(255, 200, 120, 0.06);
    color: rgba(255, 245, 230, 0.72);
}

minimize-button:hover {
    background: rgba(255, 200, 120, 0.10);
}

pin-button {
    background: rgba(255, 200, 120, 0.06);
    color: rgba(255, 245, 230, 0.72);
}

pin-button:hover {
    background: rgba(255, 200, 120, 0.10);
}

pin-button.active {
    background: rgba(255, 159, 10, 0.65);
    color: rgba(26, 14, 0, 1.0);
}

pin-button.active:hover {
    background: rgba(255, 159, 10, 0.85);
}

/* Dock — warm translucent */
dock {
    background: rgba(32, 22, 10, 0.72);
    border-top-color: rgba(255, 180, 80, 0.06);
    padding: 12;
    border-width: 1;
    blur-radius: 20;
}

dock-item {
    color: rgba(255, 245, 230, 0.72);
}

dock-item.active {
    color: rgba(255, 159, 10, 1.0);
}

dock-item:hover {
    background: rgba(255, 200, 120, 0.10);
}

/* Status Bar — warm dark */
statusbar {
    background: rgba(20, 14, 4, 0.90);
    border-bottom-color: rgba(255, 180, 80, 0.06);
    color: rgba(255, 245, 230, 255);
    height: 28;
    padding: 8;
    border-width: 1;
    blur-radius: 10;
}

status-indicator.connected {
    color: rgb(52, 199, 89);
}

status-indicator.degraded {
    color: rgb(255, 214, 10);
}

notification-indicator.active {
    color: rgb(255, 107, 107);
}

notification-indicator {
    color: rgba(255, 179, 64, 0.60);
}

status-tray {
    background: rgba(255, 200, 120, 0.08);
}

/* Launcher — warm amber */
launcher-overlay {
    background: rgba(0, 0, 0, 0.45);
}

launcher {
    background: rgba(16, 10, 2, 0.96);
    width: 60;
    height: 70;
    padding: 16;
    blur-radius: 40;
}

launcher-search {
    background: rgba(255, 200, 120, 0.06);
    height: 36;
}

launcher-item {
    background: transparent;
    height: 40;
    margin-bottom: 4;
}

launcher-item.selected {
    background: rgba(255, 159, 10, 0.25);
}

/* Notifications — warm dark */
notification {
    background: rgba(36, 26, 12, 0.94);
    width: 320;
    height: 80;
    margin-bottom: 8;
    margin-right: 12;
    margin-top: 32;
    blur-radius: 20;
}

/* Menus */
menu {
    blur-radius: 20;
    border-radius: 10;
    padding: 4;
    height: 28;
}

/* Cursor */
cursor {
    color: rgba(255, 245, 230, 255);
}

menu-item:hover {
    background: rgba(255, 159, 10, 0.25);
}

menu-separator {
    background: rgba(255, 180, 80, 0.12);
}

/* Loading */
loading-overlay {
    background: rgba(20, 10, 0, 0.85);
}

loading-panel {
    background: rgba(36, 26, 12, 0.94);
    color: rgba(255, 245, 230, 255);
}

/* Window Content */
window-content {
    background: rgba(26, 16, 8, 0.95);
}

/* App-specific */
app-settings.sidebar-item {
    background: rgba(255, 200, 120, 0.06);
}

app-terminal {
    background: rgb(18, 10, 2);
    color: rgb(255, 179, 64);
}

app-browser.urlbar {
    background: rgba(255, 200, 120, 0.08);
}
"#
}

/// Create the Midday tarnished-white light theme CSS (spec-theme-midday.md)
///
/// Warm off-white surfaces, dark text, deep teal accent, light-mode glass.
pub fn midday_css() -> &'static str {
    r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Midday — Tarnished White Light
   Preset: midday
   Spec: spec-theme-midday.md
   ═══════════════════════════════════════════════════════ */

/* Desktop */
desktop-background {
    background: rgb(245, 240, 232);
}

/* Windows */
window {
    background: rgba(248, 244, 238, 0.82);
    border-color: rgba(28, 27, 24, 0.10);
    border-width: 1;
    border-radius: 16;
    box-shadow-color: rgba(28, 20, 8, 0.12);
    glass-tint: rgba(248, 244, 238, 0.78);
}

window.focused {
    border-color: rgba(28, 27, 24, 0.18);
    titlebar-background: rgba(240, 236, 228, 0.70);
}

titlebar {
    background: rgba(240, 236, 228, 0.65);
    color: rgba(28, 27, 24, 255);
    height: 36;
}

titlebar.focused {
    background: rgba(240, 236, 228, 0.75);
}

titlebar-button {
    width: 28;
    height: 28;
    margin-right: 4;
    border-radius: 14;
}

/* Close: standard red */
close-button {
    background: rgba(215, 0, 21, 0.80);
    color: rgba(255, 255, 255, 0.94);
}

close-button:hover {
    background: rgba(215, 0, 21, 1.0);
}

maximize-button {
    background: rgba(28, 27, 24, 0.04);
    color: rgba(28, 27, 24, 0.62);
}

maximize-button:hover {
    background: rgba(28, 27, 24, 0.07);
}

minimize-button {
    background: rgba(28, 27, 24, 0.04);
    color: rgba(28, 27, 24, 0.62);
}

minimize-button:hover {
    background: rgba(28, 27, 24, 0.07);
}

pin-button {
    background: rgba(28, 27, 24, 0.04);
    color: rgba(28, 27, 24, 0.62);
}

pin-button:hover {
    background: rgba(28, 27, 24, 0.07);
}

pin-button.active {
    background: rgba(0, 113, 179, 0.65);
    color: rgba(255, 255, 255, 1.0);
}

pin-button.active:hover {
    background: rgba(0, 113, 179, 0.85);
}

/* Dock — light glass */
dock {
    background: rgba(248, 244, 238, 0.78);
    border-top-color: rgba(28, 27, 24, 0.05);
    padding: 12;
    border-width: 1;
    blur-radius: 20;
}

dock-item {
    color: rgba(28, 27, 24, 0.62);
}

dock-item.active {
    color: rgba(28, 27, 24, 1.0);
}

dock-item:hover {
    background: rgba(28, 27, 24, 0.07);
}

/* Status Bar — warm light */
statusbar {
    background: rgba(242, 238, 230, 0.92);
    border-bottom-color: rgba(28, 27, 24, 0.10);
    color: rgba(28, 27, 24, 255);
    height: 28;
    padding: 8;
    border-width: 1;
    blur-radius: 10;
}

status-indicator.connected {
    color: rgb(36, 138, 61);
}

status-indicator.degraded {
    color: rgb(178, 80, 0);
}

notification-indicator.active {
    color: rgb(215, 0, 21);
}

notification-indicator {
    color: rgba(0, 113, 179, 0.55);
}

status-tray {
    background: rgba(28, 27, 24, 0.04);
}

/* Launcher — warm bright */
launcher-overlay {
    background: rgba(28, 27, 24, 0.20);
}

launcher {
    background: rgba(248, 244, 238, 0.97);
    width: 60;
    height: 70;
    padding: 16;
    blur-radius: 40;
}

launcher-search {
    background: rgba(255, 255, 255, 0.65);
    height: 36;
}

launcher-item {
    background: transparent;
    height: 40;
    margin-bottom: 4;
}

launcher-item.selected {
    background: rgba(0, 113, 179, 0.15);
}

/* Notifications — light */
notification {
    background: rgba(248, 244, 238, 0.94);
    width: 320;
    height: 80;
    margin-bottom: 8;
    margin-right: 12;
    margin-top: 32;
    blur-radius: 20;
}

/* Menus */
menu {
    blur-radius: 20;
    border-radius: 10;
    padding: 4;
    height: 28;
}

/* Cursor */
cursor {
    color: rgba(28, 27, 24, 255);
}

menu-item:hover {
    background: rgba(0, 113, 179, 0.15);
}

menu-separator {
    background: rgba(28, 27, 24, 0.10);
}

/* Loading */
loading-overlay {
    background: rgba(245, 240, 232, 0.85);
}

loading-panel {
    background: rgba(248, 244, 238, 0.94);
    color: rgba(28, 27, 24, 255);
}

/* Window Content */
window-content {
    background: rgba(250, 246, 240, 0.95);
}

/* App-specific */
app-settings.sidebar-item {
    background: rgba(28, 27, 24, 0.04);
}

app-terminal {
    background: rgb(248, 244, 238);
    color: rgb(28, 27, 24);
}

app-browser.urlbar {
    background: rgba(255, 255, 255, 0.65);
}
"#
}

/// Available theme preset IDs.
pub enum ThemePreset {
    /// Standard Liquid Glass dark theme (default).
    LiquidGlass,
    /// OLED-optimized dark theme with true black backgrounds.
    Night,
    /// Warm dark theme with amber/orange tones.
    Sunset,
    /// Tarnished white light theme.
    Midday,
}

impl ThemePreset {
    /// Get the CSS string for this theme preset.
    pub fn css(&self) -> &'static str {
        match self {
            ThemePreset::LiquidGlass => default_liquid_glass_css(),
            ThemePreset::Night => night_css(),
            ThemePreset::Sunset => sunset_css(),
            ThemePreset::Midday => midday_css(),
        }
    }

    /// Parse a theme ID string to a preset.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "liquid-glass" | "standard" | "default" => Some(ThemePreset::LiquidGlass),
            "night" => Some(ThemePreset::Night),
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
        assert!(ThemePreset::from_id("liquid-glass").is_some());
        assert!(ThemePreset::from_id("night").is_some());
        assert!(ThemePreset::from_id("sunset").is_some());
        assert!(ThemePreset::from_id("midday").is_some());
        assert!(ThemePreset::from_id("default").is_some());
        assert!(ThemePreset::from_id("standard").is_some());
        assert!(ThemePreset::from_id("nonexistent").is_none());
    }
}
