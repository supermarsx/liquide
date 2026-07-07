//! Shell-specific theme system.
//!
//! `ShellTheme` is a CSS-derived colour cache: it is populated from the active
//! CSS theme via [`crate::theme_loader::css_to_shell_theme`] for the
//! by-design-imperative paint surfaces that have no CSS primitive (window
//! decoration frame colours, the cursor fill, and the terminal cell grid).
//! [`ShellTheme::default_dark`] is the hardcoded fallback used only when the
//! default CSS theme fails to parse.

use liquide_compositor::pixel::Color;

/// Shell-specific theme providing colors for all UI elements.
pub struct ShellTheme {
    // Desktop
    /// Background color for the desktop surface.
    pub desktop_background: Color,

    // Window decorations
    /// Title bar background when the window is focused.
    pub window_title_bar_focused: Color,
    /// Title bar background when the window is unfocused.
    pub window_title_bar_unfocused: Color,
    /// Title bar text color.
    pub window_title_text: Color,
    /// Window border color when focused.
    pub window_border_focused: Color,
    /// Window border color when unfocused.
    pub window_border_unfocused: Color,
    /// Window shadow color.
    pub window_shadow: Color,
    /// Glass tint color for window title bars (frosted glass effect).
    pub window_glass_tint: Color,

    // Dock
    /// Glass tint color for the dock panel.
    pub dock_glass_tint: Color,
    /// 1px accent border at the top edge of the dock.
    pub dock_border: Color,
    /// Color for a dock item whose application is running.
    pub dock_item_active: Color,
    /// Color for a dock item whose application is not running.
    pub dock_item_inactive: Color,
    /// Color for the hover highlight overlay on dock items.
    pub dock_hover_highlight: Color,

    // Status bar
    /// Glass tint color for the status bar.
    pub status_bar_glass_tint: Color,
    /// 1px accent border at the bottom edge of the status bar.
    pub status_bar_border: Color,
    /// Text color for the status bar clock and custom items.
    pub status_bar_text: Color,
    /// Color for a good connection quality indicator.
    pub status_bar_connected: Color,
    /// Color for a degraded connection quality indicator.
    pub status_bar_degraded: Color,
    /// Color for the notification indicator when there are unread notifications.
    pub status_bar_notification_active: Color,
    /// Color for the notification indicator when there are no unread notifications.
    pub status_bar_notification_inactive: Color,
    /// Color for the system tray area.
    pub status_bar_tray: Color,

    // Launcher
    /// Full-screen overlay tint behind the launcher.
    pub launcher_overlay: Color,
    /// Glass tint for the launcher panel.
    pub launcher_glass_tint: Color,
    /// Background color for the search bar.
    pub launcher_search_bar: Color,
    /// Color for the selected/highlighted launcher item.
    pub launcher_item_selected: Color,
    /// Color for a normal (non-selected) launcher item.
    pub launcher_item_normal: Color,

    // Notifications
    /// Glass tint color for notification cards.
    pub notification_glass_tint: Color,

    // Cursor
    /// Cursor fill color (CSS `cursor { color }`).
    pub cursor_color: Color,
    /// Cursor size multiplier (CSS `cursor { scale }`/`--cursor-scale`).
    /// `1.0` is the historic size; larger values grow the cursor.
    pub cursor_scale: f32,

    // Context / session menus
    /// Hover highlight color for context and session menu items.
    pub menu_item_hover: Color,
    /// Background color for menu separators.
    pub menu_separator: Color,

    // Tooltip (dock-hover bubble)
    /// Background fill for the tooltip bubble (CSS `--tooltip-bg`).
    pub tooltip_bg: Color,
    /// Text color for the tooltip label (CSS `--tooltip-text`).
    pub tooltip_text: Color,
    /// Border color for the tooltip bubble (CSS `--tooltip-border`).
    pub tooltip_border: Color,
    /// Corner radius (logical px) for the tooltip bubble (CSS `--tooltip-radius`).
    pub tooltip_radius: f32,
    /// Drop-shadow color for the tooltip bubble (CSS `--shadow-medium` color).
    pub tooltip_shadow: Color,

    // Loading overlay
    /// Full-screen loading overlay tint.
    pub loading_overlay: Color,
    /// Glass tint for the loading panel.
    pub loading_glass_tint: Color,
    /// Text color for loading messages.
    pub loading_text: Color,

    // Window content
    /// Generic window content background color.
    pub window_content_background: Color,

    // App-specific colors
    /// Settings app sidebar item background.
    pub app_settings_sidebar_item: Color,
    /// Terminal app background.
    pub app_terminal_background: Color,
    /// Terminal app text color.
    pub app_terminal_text: Color,
    /// Browser app URL bar background.
    pub app_browser_urlbar: Color,
}

impl ShellTheme {
    /// The default dark shell theme.
    ///
    /// Uses colors similar to the original hardcoded values: dark blue desktop
    /// background, dark semi-transparent glass tints, and blue accent colors.
    #[must_use]
    pub fn default_dark() -> Self {
        Self {
            // Desktop
            desktop_background: Color::new(30, 60, 90, 255),

            // Window decorations
            window_title_bar_focused: Color::new(60, 60, 70, 240),
            window_title_bar_unfocused: Color::new(45, 45, 50, 220),
            window_title_text: Color::new(220, 220, 220, 255),
            window_border_focused: Color::new(80, 140, 220, 200),
            window_border_unfocused: Color::new(60, 60, 60, 150),
            window_shadow: Color::new(0, 0, 0, 80),
            window_glass_tint: Color::new(25, 28, 38, 200),

            // Dock
            dock_glass_tint: Color::new(20, 22, 30, 240),
            dock_border: Color::new(100, 160, 240, 180),
            dock_item_active: Color::new(80, 150, 235, 220),
            dock_item_inactive: Color::new(160, 160, 170, 200),
            dock_hover_highlight: Color::new(255, 255, 255, 50),

            // Status bar
            status_bar_glass_tint: Color::new(15, 18, 28, 245),
            status_bar_border: Color::new(100, 160, 240, 160),
            status_bar_text: Color::new(220, 220, 220, 255),
            status_bar_connected: Color::new(60, 200, 60, 255),
            status_bar_degraded: Color::new(220, 180, 40, 255),
            status_bar_notification_active: Color::new(60, 140, 255, 255),
            status_bar_notification_inactive: Color::new(160, 160, 160, 200),
            status_bar_tray: Color::new(100, 100, 100, 150),

            // Launcher
            launcher_overlay: Color::new(0, 0, 0, 140),
            launcher_glass_tint: Color::new(30, 30, 40, 220),
            launcher_search_bar: Color::new(50, 50, 60, 200),
            launcher_item_selected: Color::new(60, 120, 200, 180),
            launcher_item_normal: Color::new(60, 60, 70, 140),

            // Notifications
            notification_glass_tint: Color::new(50, 50, 60, 210),

            // Cursor
            cursor_color: Color::new(255, 255, 255, 255),
            cursor_scale: 1.0,
            menu_item_hover: Color::new(255, 255, 255, 40),
            menu_separator: Color::new(255, 255, 255, 40),

            // Tooltip — macOS-restrained dark bubble: near-opaque dark fill,
            // light text, hairline border, subtle drop shadow. Matches the
            // `--tooltip-*` tokens in variables.css so the imperative bubble reads
            // the same as the CSS spec even on the fallback path.
            tooltip_bg: Color::new(39, 39, 42, 245),
            tooltip_text: Color::new(250, 250, 250, 235),
            tooltip_border: Color::new(255, 255, 255, 26),
            tooltip_radius: 6.0,
            tooltip_shadow: Color::new(0, 0, 0, 115),

            // Loading overlay
            loading_overlay: Color::new(0, 0, 0, 180),
            loading_glass_tint: Color::new(40, 40, 50, 220),
            loading_text: Color::new(220, 220, 220, 255),

            // Window content
            //
            // Translucent by default (alpha < 255) so a window composited over
            // liquide's wallpaper/lower windows shows them faintly through — the
            // macOS "vibrancy" aesthetic. This is the hardcoded fallback used
            // only when the CSS theme fails to parse; the real value comes from
            // the active theme's `window-content { background: rgba(...) }`
            // (see theme_loader::css_to_shell_theme). The alpha is deliberately
            // high (~0.90) so window text stays readable. The renderer honors
            // this alpha (no forced-opaque clamp); a theme/app can lower it for
            // a glassier window or raise it to 255 for a fully opaque one.
            window_content_background: Color::new(30, 30, 34, 230),

            // App-specific
            app_settings_sidebar_item: Color::new(45, 45, 55, 200),
            app_terminal_background: Color::new(20, 20, 25, 255),
            app_terminal_text: Color::new(100, 220, 100, 255),
            app_browser_urlbar: Color::new(55, 55, 65, 255),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_loader::css_to_shell_theme;
    use liquide_theme_css::{ThemeEngine, ThemeParser};

    fn resolve_content_bg(css: &str) -> Color {
        let stylesheet = ThemeParser::new().parse_str(css).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        css_to_shell_theme(&engine).window_content_background
    }

    /// WINDOW TRANSPARENCY — alpha passthrough (the load-bearing correctness).
    ///
    /// A theme that sets a sub-1.0 alpha on `window-content { background }` must
    /// RESOLVE to that alpha in `ShellTheme.window_content_background`, NOT be
    /// clamped up to opaque 255. This is what lets a window composite
    /// translucently over liquide's wallpaper / lower windows.
    ///
    /// Teeth (no-fake-green): if the resolver forced the alpha to 255 (a
    /// forced-opaque regression), `bg.a` would be 255 and every assertion below
    /// fails. `0.85 * 255 = 216.75`, so a correct resolver yields ~217.
    #[test]
    fn window_content_background_alpha_passes_through() {
        let bg = resolve_content_bg("window-content { background: rgba(28, 28, 30, 0.85); }");
        assert_eq!(bg.r, 28, "red channel must pass through");
        assert_eq!(bg.g, 28, "green channel must pass through");
        assert_eq!(bg.b, 30, "blue channel must pass through");
        assert!(
            (216..=217).contains(&bg.a),
            "alpha 0.85 must resolve to ~217 (0.85*255), got {} — forced-opaque regression?",
            bg.a
        );
        assert!(
            bg.a < 255,
            "a translucent window-content must NOT be clamped to opaque 255 (got {})",
            bg.a
        );
    }

    /// A stronger differential: two themes with different sub-1.0 alphas must
    /// resolve to DIFFERENT alphas, and a more-transparent CSS value must yield
    /// a lower resolved alpha. A resolver that ignored/clamped alpha would make
    /// these equal (both 255) and fail.
    #[test]
    fn window_content_alpha_tracks_the_css_value() {
        let glassy = resolve_content_bg("window-content { background: rgba(20, 20, 24, 0.55); }").a;
        let solidish = resolve_content_bg("window-content { background: rgba(20, 20, 24, 0.95); }").a;
        assert!(
            glassy < solidish,
            "a more-transparent CSS window-content (0.55) must resolve to a lower alpha than 0.95 ({glassy} vs {solidish})"
        );
        assert!(glassy < 255 && solidish < 255, "neither may be forced opaque");
    }

    /// The hardcoded fallback (`default_dark`, used only when the CSS theme fails
    /// to parse) must ALSO be translucent — otherwise a parse failure silently
    /// forces every window opaque. Guards the removed `Color::new(35,35,40,255)`.
    #[test]
    fn default_dark_window_content_is_translucent() {
        let a = ShellTheme::default_dark().window_content_background.a;
        assert!(
            a < 255,
            "default_dark window_content_background must be translucent (<255), got {a} — hardcoded opaque default regressed"
        );
    }

    /// The shipped DEFAULT theme (macOS Dark) must ship translucent windows
    /// (vibrancy: wallpaper faintly shows) yet stay readable — alpha high enough
    /// that window text remains legible over the wallpaper. Drives the REAL
    /// shipped asset through the production resolver.
    #[test]
    fn macos_dark_ships_translucent_but_readable_windows() {
        let css = include_str!("../../../assets/themes/macos_dark.css");
        let a = resolve_content_bg(css).a;
        assert!(
            a < 255,
            "macos_dark window-content must be translucent (<255) so wallpaper shows through, got {a}"
        );
        assert!(
            a >= 200,
            "macos_dark window-content must stay mostly opaque (>=200) for text readability, got {a}"
        );
    }

    fn resolve_theme(css: &str) -> ShellTheme {
        let stylesheet = ThemeParser::new().parse_str(css).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        css_to_shell_theme(&engine)
    }

    /// fix-tooltip-render POLISH (color source): the tooltip colors must resolve
    /// from the `--tooltip-*` tokens on `tooltip-content` — a DARK fill + LIGHT
    /// text with real contrast, and the radius from `--tooltip-radius`.
    ///
    /// Teeth (no-fake-green): before the fix the imperative bubble read
    /// `launcher_search_bar` (light) and rendered a white, unreadable box. This
    /// asserts the resolver both EXPOSES a tooltip entry and sources it from the
    /// tooltip tokens (var() resolved). A non-default radius (9) proves the radius
    /// is genuinely CSS-sourced, not just the 6px fallback.
    #[test]
    fn tooltip_colors_resolve_from_tooltip_tokens_dark_bg_light_text() {
        let css = "\
            :root { \
              --tooltip-bg: rgba(39, 39, 42, 0.96); \
              --tooltip-text: rgba(250, 250, 250, 0.92); \
              --tooltip-border: rgba(255, 255, 255, 0.10); \
              --tooltip-radius: 9; \
            } \
            tooltip-content { \
              background: var(--tooltip-bg); \
              color: var(--tooltip-text); \
              border-color: var(--tooltip-border); \
              border-radius: var(--tooltip-radius); \
            }";
        let t = resolve_theme(css);
        assert!(
            t.tooltip_bg.r < 80 && t.tooltip_bg.g < 80 && t.tooltip_bg.b < 80,
            "tooltip_bg must resolve DARK from --tooltip-bg, got {:?}",
            t.tooltip_bg
        );
        assert!(
            t.tooltip_text.r > 200 && t.tooltip_text.g > 200 && t.tooltip_text.b > 200,
            "tooltip_text must resolve LIGHT from --tooltip-text, got {:?}",
            t.tooltip_text
        );
        let lum = |c: Color| c.r as u32 + c.g as u32 + c.b as u32;
        assert!(
            lum(t.tooltip_text) > lum(t.tooltip_bg) + 300,
            "tooltip label vs fill must have real contrast (NOT the white-on-white \
             box); bg={:?} text={:?}",
            t.tooltip_bg,
            t.tooltip_text
        );
        assert!(
            (t.tooltip_radius - 9.0).abs() < 0.5,
            "tooltip_radius must resolve from --tooltip-radius (9), got {} — is the \
             radius genuinely CSS-sourced?",
            t.tooltip_radius
        );
    }

    /// The shipped macOS-Dark theme's tooltip tokens must resolve to a dark,
    /// readable bubble through the REAL asset cascade (variables + components +
    /// theme) — the exact scenario that rendered a white box before the fix.
    #[test]
    fn macos_dark_tooltip_is_dark_with_light_text() {
        let css = concat!(
            include_str!("../../../assets/themes/variables.css"),
            include_str!("../../../assets/themes/components/tooltip.css"),
            include_str!("../../../assets/themes/macos_dark.css"),
        );
        let t = resolve_theme(css);
        let lum = |c: Color| c.r as u32 + c.g as u32 + c.b as u32;
        assert!(
            lum(t.tooltip_bg) < 300,
            "macos_dark tooltip fill must be dark, got {:?}",
            t.tooltip_bg
        );
        assert!(
            lum(t.tooltip_text) > 600,
            "macos_dark tooltip text must be light, got {:?}",
            t.tooltip_text
        );
    }

    /// The hardcoded fallback must ALSO ship a dark, readable tooltip (a parse
    /// failure must not silently reinstate the white box).
    #[test]
    fn default_dark_tooltip_is_dark_with_light_text() {
        let t = ShellTheme::default_dark();
        let lum = |c: Color| c.r as u32 + c.g as u32 + c.b as u32;
        assert!(lum(t.tooltip_bg) < 300, "fallback tooltip fill must be dark");
        assert!(
            lum(t.tooltip_text) > 600,
            "fallback tooltip text must be light"
        );
        assert!(t.tooltip_radius > 0.0, "fallback tooltip must be rounded");
    }
}
