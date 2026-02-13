//! CSS theme loading and integration with ShellTheme

use liquide_compositor::pixel::Color;
use liquide_theme_css::{ThemeEngine, ThemeParser, Result as CssResult};
use liquide_theme_css::prelude::PropertyValue;
use std::path::Path;
use tracing::{info, warn};

use crate::theme::ShellTheme;

/// Load a CSS theme and convert it to ShellTheme
pub fn load_css_theme<P: AsRef<Path>>(path: P) -> CssResult<ShellTheme> {
    let parser = ThemeParser::new();
    let stylesheet = parser.parse_file(path)?;
    let engine = ThemeEngine::new(stylesheet);
    
    info!("Loaded CSS theme with {} rules", engine.stylesheet().rule_count());
    
    Ok(css_to_shell_theme(&engine))
}

/// Convert CSS theme engine to ShellTheme
pub fn css_to_shell_theme(engine: &ThemeEngine) -> ShellTheme {
    ShellTheme {
        // Desktop
        desktop_background: query_color(engine, "desktop", &[], &[], "background")
            .unwrap_or_else(|| Color::new(30, 30, 40, 255)),
        
        // Window decorations
        window_title_bar_focused: query_color(engine, "window", &["focused".into()], &[], "titlebar-background")
            .or_else(|| query_color(engine, "titlebar", &["focused".into()], &[], "background"))
            .unwrap_or_else(|| Color::new(60, 60, 70, 240)),
        
        window_title_bar_unfocused: query_color(engine, "window", &[], &[], "titlebar-background")
            .or_else(|| query_color(engine, "titlebar", &[], &[], "background"))
            .unwrap_or_else(|| Color::new(45, 45, 55, 220)),
        
        window_title_text: query_color(engine, "titlebar", &[], &[], "color")
            .unwrap_or_else(|| Color::new(236, 239, 244, 255)),
        
        window_border_focused: query_color(engine, "window", &["focused".into()], &[], "border-color")
            .unwrap_or_else(|| Color::new(94, 129, 172, 200)),
        
        window_border_unfocused: query_color(engine, "window", &[], &[], "border-color")
            .unwrap_or_else(|| Color::new(76, 86, 106, 150)),
        
        window_shadow: query_color(engine, "window", &[], &[], "box-shadow-color")
            .unwrap_or_else(|| Color::new(0, 0, 0, 80)),
        
        window_glass_tint: query_color(engine, "window", &[], &[], "glass-tint")
            .or_else(|| query_color(engine, "window", &[], &[], "background"))
            .unwrap_or_else(|| Color::new(62, 62, 72, 200)),
        
        // Dock
        dock_glass_tint: query_color(engine, "dock", &[], &[], "background")
            .unwrap_or_else(|| Color::new(46, 52, 64, 225)),
        
        dock_border: query_color(engine, "dock", &[], &[], "border-top-color")
            .or_else(|| query_color(engine, "dock", &[], &[], "border-color"))
            .unwrap_or_else(|| Color::new(76, 86, 106, 255)),
        
        dock_item_active: query_color(engine, "dock-item", &["active".into()], &[], "color")
            .unwrap_or_else(|| Color::new(236, 239, 244, 255)),
        
        dock_item_inactive: query_color(engine, "dock-item", &[], &[], "color")
            .unwrap_or_else(|| Color::new(216, 222, 233, 200)),
        
        dock_hover_highlight: query_color(engine, "dock-item", &[], &["hover".into()], "background")
            .unwrap_or_else(|| Color::new(94, 129, 172, 60)),
        
        // Status bar
        status_bar_glass_tint: query_color(engine, "statusbar", &[], &[], "background")
            .or_else(|| query_color(engine, "status-bar", &[], &[], "background"))
            .unwrap_or_else(|| Color::new(59, 66, 82, 240)),
        
        status_bar_border: query_color(engine, "statusbar", &[], &[], "border-bottom-color")
            .or_else(|| query_color(engine, "status-bar", &[], &[], "border-color"))
            .unwrap_or_else(|| Color::new(76, 86, 106, 255)),
        
        status_bar_text: query_color(engine, "statusbar", &[], &[], "color")
            .or_else(|| query_color(engine, "status-bar", &[], &[], "color"))
            .unwrap_or_else(|| Color::new(236, 239, 244, 255)),
        
        status_bar_connected: query_color(engine, "status-indicator", &["connected".into()], &[], "color")
            .unwrap_or_else(|| Color::new(163, 190, 140, 255)),
        
        status_bar_degraded: query_color(engine, "status-indicator", &["degraded".into()], &[], "color")
            .unwrap_or_else(|| Color::new(235, 203, 139, 255)),
        
        status_bar_notification_active: query_color(engine, "notification-indicator", &["active".into()], &[], "color")
            .unwrap_or_else(|| Color::new(191, 97, 106, 255)),
        
        status_bar_notification_inactive: query_color(engine, "notification-indicator", &[], &[], "color")
            .unwrap_or_else(|| Color::new(129, 161, 193, 160)),
        
        status_bar_tray: query_color(engine, "status-tray", &[], &[], "background")
            .unwrap_or_else(|| Color::new(67, 76, 94, 200)),
        
        // Launcher
        launcher_overlay: query_color(engine, "launcher-overlay", &[], &[], "background")
            .unwrap_or_else(|| Color::new(0, 0, 0, 120)),
        
        launcher_glass_tint: query_color(engine, "launcher", &[], &[], "background")
            .unwrap_or_else(|| Color::new(46, 52, 64, 245)),
        
        launcher_search_bar: query_color(engine, "launcher-search", &[], &[], "background")
            .or_else(|| query_color(engine, "launcher", &[], &[], "input-background"))
            .unwrap_or_else(|| Color::new(59, 66, 82, 255)),
        
        launcher_item_selected: query_color(engine, "launcher-item", &["selected".into()], &[], "background")
            .unwrap_or_else(|| Color::new(94, 129, 172, 100)),
        
        launcher_item_normal: query_color(engine, "launcher-item", &[], &[], "background")
            .unwrap_or_else(|| Color::new(46, 52, 64, 0)),
        
        // Notifications
        notification_glass_tint: query_color(engine, "notification", &[], &[], "background")
            .unwrap_or_else(|| Color::new(59, 66, 82, 245)),
        
        // Cursor
        cursor_color: query_color(engine, "cursor", &[], &[], "color")
            .unwrap_or_else(|| Color::new(236, 239, 244, 255)),
        
        // Context / session menus
        menu_item_hover: query_color(engine, "menu-item", &[], &["hover".into()], "background")
            .unwrap_or_else(|| Color::new(94, 129, 172, 100)),
        
        // Loading overlay
        loading_overlay: query_color(engine, "loading-overlay", &[], &[], "background")
            .unwrap_or_else(|| Color::new(46, 52, 64, 220)),
        
        loading_glass_tint: query_color(engine, "loading-panel", &[], &[], "background")
            .unwrap_or_else(|| Color::new(59, 66, 82, 240)),
        
        loading_text: query_color(engine, "loading-panel", &[], &[], "color")
            .unwrap_or_else(|| Color::new(236, 239, 244, 255)),
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

/// Create a default Nord dark theme CSS
pub fn default_nord_css() -> &'static str {
    r#"
/* Nord Dark Theme for Liquide Desktop */

:root {
    --nord0: #2e3440;
    --nord1: #3b4252;
    --nord2: #434c5e;
    --nord3: #4c566a;
    --nord4: #d8dee9;
    --nord5: #e5e9f0;
    --nord6: #eceff4;
    --nord7: #8fbcbb;
    --nord8: #88c0d0;
    --nord9: #81a1c1;
    --nord10: #5e81ac;
    --nord11: #bf616a;
    --nord12: #d08770;
    --nord13: #ebcb8b;
    --nord14: #a3be8c;
    --nord15: #b48ead;
}

/* Desktop */
desktop {
    background: var(--nord0);
}

/* Windows */
window {
    background: var(--nord0);
    border-color: var(--nord3);
    box-shadow-color: rgba(0, 0, 0, 0.3);
    glass-tint: var(--nord1);
}

window.focused {
    border-color: var(--nord10);
    titlebar-background: var(--nord1);
}

titlebar {
    background: var(--nord1);
    color: var(--nord6);
}

titlebar.focused {
    background: var(--nord2);
}

/* Dock */
dock {
    background: rgba(46, 52, 64, 0.95);
    border-top-color: var(--nord3);
}

dock-item {
    color: rgba(216, 222, 233, 0.8);
}

dock-item.active {
    color: var(--nord6);
}

dock-item:hover {
    background: rgba(94, 129, 172, 0.25);
}

/* Status Bar */
statusbar {
    background: rgba(59, 66, 82, 0.95);
    border-bottom-color: var(--nord3);
    color: var(--nord6);
}

status-indicator.connected {
    color: var(--nord14);
}

status-indicator.degraded {
    color: var(--nord13);
}

notification-indicator.active {
    color: var(--nord11);
}

notification-indicator {
    color: rgba(129, 161, 193, 0.6);
}

status-tray {
    background: rgba(67, 76, 94, 0.8);
}

/* Launcher */
launcher-overlay {
    background: rgba(0, 0, 0, 0.5);
}

launcher {
    background: rgba(46, 52, 64, 0.96);
}

launcher-search {
    background: var(--nord1);
}

launcher-item {
    background: transparent;
}

launcher-item.selected {
    background: rgba(94, 129, 172, 0.4);
}

/* Notifications */
notification {
    background: rgba(59, 66, 82, 0.96);
}

/* Cursor */
cursor {
    color: var(--nord6);
}

/* Menus */
menu-item:hover {
    background: rgba(94, 129, 172, 0.4);
}

/* Loading */
loading-overlay {
    background: rgba(46, 52, 64, 0.9);
}

loading-panel {
    background: rgba(59, 66, 82, 0.95);
    color: var(--nord6);
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_nord_theme() {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(default_nord_css()).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        
        let theme = css_to_shell_theme(&engine);
        
        // Verify some key colors are loaded
        assert_eq!(theme.desktop_background.r, 46);
        assert_eq!(theme.desktop_background.g, 52);
        assert_eq!(theme.desktop_background.b, 64);
    }
}
