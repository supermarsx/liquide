//! Shell-specific theme system.
//!
//! Maps the [`liquide_ui::Theme`] color palette to concrete colors used by
//! dock, status bar, launcher, notifications, window decorations, and other
//! shell UI elements.

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
    /// Cursor fill color.
    pub cursor_color: Color,

    // Context / session menus
    /// Hover highlight color for context and session menu items.
    pub menu_item_hover: Color,
    /// Background color for menu separators.
    pub menu_separator: Color,

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
    /// Create a shell theme from a UI theme.
    ///
    /// Maps the UI theme's semantic color palette to the shell's concrete
    /// color slots. Dark themes produce darker, semi-transparent glass tints;
    /// light themes produce lighter variants.
    #[must_use]
    pub fn from_ui_theme(theme: &liquide_ui::Theme) -> Self {
        let is_dark = theme.colors.background.r < 128;
        if is_dark {
            Self::from_ui_theme_dark(theme)
        } else {
            Self::from_ui_theme_light(theme)
        }
    }

    /// Map a dark UI theme to shell colors.
    fn from_ui_theme_dark(theme: &liquide_ui::Theme) -> Self {
        let c = &theme.colors;
        Self {
            desktop_background: Color::new(
                c.background.r.saturating_add(12),
                c.background.g.saturating_add(42),
                c.background.b.saturating_add(72),
                255,
            ),
            window_title_bar_focused: Color::new(
                c.surface.r.saturating_add(27),
                c.surface.g.saturating_add(27),
                c.surface.b.saturating_add(37),
                240,
            ),
            window_title_bar_unfocused: Color::new(
                c.surface.r.saturating_add(12),
                c.surface.g.saturating_add(12),
                c.surface.b.saturating_add(17),
                220,
            ),
            window_title_text: Color::new(c.foreground.r, c.foreground.g, c.foreground.b, 255),
            window_border_focused: Color::new(c.primary.r, c.primary.g, c.primary.b, 200),
            window_border_unfocused: Color::new(c.border.r, c.border.g, c.border.b, 150),
            window_shadow: Color::new(0, 0, 0, 80),
            window_glass_tint: Color::new(
                c.surface.r.saturating_add(17),
                c.surface.g.saturating_add(17),
                c.surface.b.saturating_add(27),
                200,
            ),
            dock_glass_tint: Color::new(
                c.surface.r.saturating_add(22),
                c.surface.g.saturating_add(25),
                c.surface.b.saturating_add(35),
                225,
            ),
            dock_border: Color::new(
                c.primary.r.min(120),
                c.primary.g.min(180),
                c.primary.b.min(240),
                100,
            ),
            dock_item_active: Color::new(c.primary.r, c.primary.g, c.primary.b, 220),
            dock_item_inactive: Color::new(140, 140, 150, 180),
            dock_hover_highlight: Color::new(255, 255, 255, 50),
            status_bar_glass_tint: Color::new(
                c.background.r.saturating_add(20),
                c.background.g.saturating_add(22),
                c.background.b.saturating_add(35),
                235,
            ),
            status_bar_border: Color::new(
                c.primary.r.min(100),
                c.primary.g.min(160),
                c.primary.b.min(220),
                80,
            ),
            status_bar_text: Color::new(c.foreground.r, c.foreground.g, c.foreground.b, 255),
            status_bar_connected: Color::new(c.success.r, c.success.g, c.success.b, 255),
            status_bar_degraded: Color::new(c.warning.r, c.warning.g, c.warning.b, 255),
            status_bar_notification_active: Color::new(c.primary.r, c.primary.g, c.primary.b, 255),
            status_bar_notification_inactive: Color::new(160, 160, 160, 200),
            status_bar_tray: Color::new(100, 100, 100, 150),
            launcher_overlay: Color::new(0, 0, 0, 140),
            launcher_glass_tint: Color::new(
                c.background.r.saturating_add(12),
                c.background.g.saturating_add(12),
                c.background.b.saturating_add(22),
                220,
            ),
            launcher_search_bar: Color::new(
                c.surface.r.saturating_add(17),
                c.surface.g.saturating_add(17),
                c.surface.b.saturating_add(27),
                200,
            ),
            launcher_item_selected: Color::new(c.primary.r, c.primary.g, c.primary.b, 180),
            launcher_item_normal: Color::new(
                c.surface.r.saturating_add(27),
                c.surface.g.saturating_add(27),
                c.surface.b.saturating_add(37),
                140,
            ),
            notification_glass_tint: Color::new(
                c.surface.r.saturating_add(17),
                c.surface.g.saturating_add(17),
                c.surface.b.saturating_add(27),
                210,
            ),
            cursor_color: Color::new(255, 255, 255, 255),
            menu_item_hover: Color::new(255, 255, 255, 30),
            menu_separator: Color::new(255, 255, 255, 40),
            loading_overlay: Color::new(0, 0, 0, 180),
            loading_glass_tint: Color::new(
                c.surface.r.saturating_add(7),
                c.surface.g.saturating_add(7),
                c.surface.b.saturating_add(7),
                220,
            ),
            loading_text: Color::new(c.foreground.r, c.foreground.g, c.foreground.b, 255),
            window_content_background: Color::new(
                c.surface.r.saturating_sub(10),
                c.surface.g.saturating_sub(10),
                c.surface.b.saturating_sub(10),
                255,
            ),
            app_settings_sidebar_item: Color::new(
                c.surface.r.saturating_add(10),
                c.surface.g.saturating_add(10),
                c.surface.b.saturating_add(15),
                200,
            ),
            app_terminal_background: Color::new(
                c.background.r.saturating_sub(15),
                c.background.g.saturating_sub(15),
                c.background.b.saturating_sub(20),
                255,
            ),
            app_terminal_text: Color::new(
                c.success.r.max(100),
                c.success.g.max(220),
                c.success.b.max(100),
                255,
            ),
            app_browser_urlbar: Color::new(
                c.surface.r.saturating_add(20),
                c.surface.g.saturating_add(20),
                c.surface.b.saturating_add(30),
                255,
            ),
        }
    }

    /// Map a light UI theme to shell colors.
    fn from_ui_theme_light(theme: &liquide_ui::Theme) -> Self {
        let c = &theme.colors;
        Self {
            desktop_background: Color::new(140, 180, 220, 255),
            window_title_bar_focused: Color::new(c.surface.r, c.surface.g, c.surface.b, 245),
            window_title_bar_unfocused: Color::new(
                c.background.r.saturating_sub(10),
                c.background.g.saturating_sub(10),
                c.background.b.saturating_sub(10),
                230,
            ),
            window_title_text: Color::new(c.foreground.r, c.foreground.g, c.foreground.b, 255),
            window_border_focused: Color::new(c.primary.r, c.primary.g, c.primary.b, 200),
            window_border_unfocused: Color::new(c.border.r, c.border.g, c.border.b, 150),
            window_shadow: Color::new(0, 0, 0, 40),
            window_glass_tint: Color::new(c.surface.r, c.surface.g, c.surface.b, 180),
            dock_glass_tint: Color::new(240, 240, 240, 200),
            dock_border: Color::new(200, 200, 205, 120),
            dock_item_active: Color::new(c.primary.r, c.primary.g, c.primary.b, 200),
            dock_item_inactive: Color::new(160, 160, 160, 160),
            dock_hover_highlight: Color::new(0, 0, 0, 25),
            status_bar_glass_tint: Color::new(245, 245, 245, 220),
            status_bar_border: Color::new(200, 200, 210, 100),
            status_bar_text: Color::new(c.foreground.r, c.foreground.g, c.foreground.b, 255),
            status_bar_connected: Color::new(c.success.r, c.success.g, c.success.b, 255),
            status_bar_degraded: Color::new(c.warning.r, c.warning.g, c.warning.b, 255),
            status_bar_notification_active: Color::new(c.primary.r, c.primary.g, c.primary.b, 255),
            status_bar_notification_inactive: Color::new(130, 130, 130, 180),
            status_bar_tray: Color::new(180, 180, 180, 150),
            launcher_overlay: Color::new(0, 0, 0, 80),
            launcher_glass_tint: Color::new(245, 245, 250, 230),
            launcher_search_bar: Color::new(235, 235, 240, 220),
            launcher_item_selected: Color::new(c.primary.r, c.primary.g, c.primary.b, 160),
            launcher_item_normal: Color::new(240, 240, 245, 140),
            notification_glass_tint: Color::new(248, 248, 252, 230),
            cursor_color: Color::new(0, 0, 0, 255),
            menu_item_hover: Color::new(0, 0, 0, 20),
            menu_separator: Color::new(0, 0, 0, 30),
            loading_overlay: Color::new(255, 255, 255, 160),
            loading_glass_tint: Color::new(245, 245, 245, 230),
            loading_text: Color::new(c.foreground.r, c.foreground.g, c.foreground.b, 255),
            window_content_background: Color::new(
                c.surface.r.saturating_sub(5),
                c.surface.g.saturating_sub(5),
                c.surface.b.saturating_sub(5),
                255,
            ),
            app_settings_sidebar_item: Color::new(
                c.surface.r.saturating_add(10),
                c.surface.g.saturating_add(10),
                c.surface.b.saturating_add(10),
                200,
            ),
            app_terminal_background: Color::new(
                c.background.r.saturating_sub(30),
                c.background.g.saturating_sub(30),
                c.background.b.saturating_sub(30),
                255,
            ),
            app_terminal_text: Color::new(
                c.success.r.saturating_sub(50),
                c.success.g.saturating_sub(20),
                c.success.b.saturating_sub(50),
                255,
            ),
            app_browser_urlbar: Color::new(
                c.surface.r.saturating_sub(15),
                c.surface.g.saturating_sub(15),
                c.surface.b.saturating_sub(15),
                255,
            ),
        }
    }

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
            menu_item_hover: Color::new(255, 255, 255, 40),
            menu_separator: Color::new(255, 255, 255, 40),

            // Loading overlay
            loading_overlay: Color::new(0, 0, 0, 180),
            loading_glass_tint: Color::new(40, 40, 50, 220),
            loading_text: Color::new(220, 220, 220, 255),

            // Window content
            window_content_background: Color::new(35, 35, 40, 255),

            // App-specific
            app_settings_sidebar_item: Color::new(45, 45, 55, 200),
            app_terminal_background: Color::new(20, 20, 25, 255),
            app_terminal_text: Color::new(100, 220, 100, 255),
            app_browser_urlbar: Color::new(55, 55, 65, 255),
        }
    }

    /// The default light shell theme.
    ///
    /// Uses a lighter blue desktop background, light/white semi-transparent
    /// glass effects, and light gray window decorations.
    #[must_use]
    pub fn default_light() -> Self {
        Self {
            // Desktop
            desktop_background: Color::new(140, 180, 220, 255),

            // Window decorations
            window_title_bar_focused: Color::new(240, 240, 245, 245),
            window_title_bar_unfocused: Color::new(220, 220, 225, 230),
            window_title_text: Color::new(40, 40, 40, 255),
            window_border_focused: Color::new(60, 130, 210, 200),
            window_border_unfocused: Color::new(190, 190, 190, 150),
            window_shadow: Color::new(0, 0, 0, 40),
            window_glass_tint: Color::new(230, 230, 235, 180),

            // Dock
            dock_glass_tint: Color::new(240, 240, 240, 200),
            dock_border: Color::new(200, 200, 205, 120),
            dock_item_active: Color::new(60, 130, 210, 200),
            dock_item_inactive: Color::new(160, 160, 160, 160),
            dock_hover_highlight: Color::new(0, 0, 0, 25),

            // Status bar
            status_bar_glass_tint: Color::new(245, 245, 245, 220),
            status_bar_border: Color::new(200, 200, 210, 100),
            status_bar_text: Color::new(40, 40, 40, 255),
            status_bar_connected: Color::new(50, 180, 50, 255),
            status_bar_degraded: Color::new(210, 170, 30, 255),
            status_bar_notification_active: Color::new(50, 120, 230, 255),
            status_bar_notification_inactive: Color::new(130, 130, 130, 180),
            status_bar_tray: Color::new(180, 180, 180, 150),

            // Launcher
            launcher_overlay: Color::new(0, 0, 0, 80),
            launcher_glass_tint: Color::new(245, 245, 250, 230),
            launcher_search_bar: Color::new(235, 235, 240, 220),
            launcher_item_selected: Color::new(50, 120, 210, 160),
            launcher_item_normal: Color::new(240, 240, 245, 140),

            // Notifications
            notification_glass_tint: Color::new(248, 248, 252, 230),

            // Cursor
            cursor_color: Color::new(0, 0, 0, 255),
            menu_item_hover: Color::new(0, 0, 0, 20),
            menu_separator: Color::new(0, 0, 0, 30),

            // Loading overlay
            loading_overlay: Color::new(255, 255, 255, 160),
            loading_glass_tint: Color::new(245, 245, 245, 230),
            loading_text: Color::new(40, 40, 40, 255),

            // Window content
            window_content_background: Color::new(250, 250, 252, 255),

            // App-specific
            app_settings_sidebar_item: Color::new(240, 240, 245, 200),
            app_terminal_background: Color::new(230, 230, 235, 255),
            app_terminal_text: Color::new(40, 120, 40, 255),
            app_browser_urlbar: Color::new(235, 235, 240, 255),
        }
    }
}
