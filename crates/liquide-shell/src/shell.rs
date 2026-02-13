//! Top-level shell — orchestrates windows, workspaces, focus, layout,
//! dock, status bar, launcher, tiling, shortcuts, notifications, and
//! seamless window mode.

use std::collections::HashMap;

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::scene::{DecorationButtons, NodeProperties, SceneNode, SceneNodeKind};
use liquide_input::KeyEvent;
use liquide_platform::PlatformEvent;

use crate::app_history::AppHistory;
use crate::config::ShellConfig;
use crate::decoration::{DecorationStyle, HitZone, hit_test_decoration};
use crate::dock::Dock;
use crate::focus::{FocusManager, FocusPolicy};
use crate::history::{WindowEventKind, WindowHistory};
use crate::launcher::{Launcher, LauncherApp, SearchResultKind};
use crate::layout::{FloatingLayout, LayoutPolicy};
use crate::notification::NotificationManager;
use crate::screen_time::ScreenTimeTracker;
use crate::seamless::SeamlessManager;
use crate::shortcuts::{ShellAction, ShortcutManager};
use crate::stats::StatsCollector;
use crate::status_bar::ShellStatusBar;
use crate::theme::ShellTheme;
use crate::tiling::TilingEngine;
use crate::window::{Window, WindowFlags, WindowId, WindowState};
use crate::workspace::WorkspaceManager;
use crate::{Result, ShellError};

/// A configurable item for the session / end-session dialog.
#[derive(Debug, Clone)]
pub struct SessionMenuItem {
    /// Display label shown in the menu.
    pub label: String,
    /// Icon name (resolved via `icon_id_for_name`).
    pub icon: String,
    /// Action to execute when clicked.
    pub action: ShellAction,
}

impl SessionMenuItem {
    /// Create a new session menu item.
    #[must_use]
    pub fn new(label: impl Into<String>, icon: impl Into<String>, action: ShellAction) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
            action,
        }
    }

    /// Default session menu items: Lock, Log Out, Restart, Shut Down.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new("Lock", "power", ShellAction::LockSession),
            Self::new("Log Out", "power", ShellAction::ShowDesktop),
            Self::new("Restart", "power", ShellAction::ShowDesktop),
            Self::new("Shut Down", "power", ShellAction::ShowDesktop),
        ]
    }
}

/// A configurable item for the desktop right-click context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    /// Display label shown in the menu.
    pub label: String,
    /// Icon name (resolved via `icon_id_for_name`).
    pub icon: String,
    /// Action to execute when clicked.
    pub action: ShellAction,
}

impl ContextMenuItem {
    /// Create a new context menu item.
    #[must_use]
    pub fn new(label: impl Into<String>, icon: impl Into<String>, action: ShellAction) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
            action,
        }
    }

    /// Default context menu items for the desktop surface.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new(
                "Configure Desktop & Wallpaper",
                "preferences-system",
                ShellAction::OpenSettings,
            ),
            Self::new(
                "Display Settings",
                "preferences-system",
                ShellAction::OpenSettings,
            ),
        ]
    }
}

/// The top-level shell managing all windows and workspaces.
pub struct Shell {
    windows: HashMap<WindowId, Window>,
    workspaces: WorkspaceManager,
    focus: FocusManager,
    layout: Box<dyn LayoutPolicy>,
    decoration_style: DecorationStyle,
    next_window_id: u64,
    screen_rect: Rect,
    window_history: WindowHistory,
    app_history: AppHistory,
    screen_time: ScreenTimeTracker,
    next_event_timestamp: u64,
    // New subsystems
    dock: Dock,
    status_bar: ShellStatusBar,
    launcher: Launcher,
    tiling: TilingEngine,
    shortcuts: ShortcutManager,
    notifications: NotificationManager,
    seamless: SeamlessManager,
    config: ShellConfig,
    theme: ShellTheme,
    session_menu_visible: bool,
    /// Desktop right-click context menu state.
    context_menu_visible: bool,
    /// Position where context menu was opened.
    context_menu_pos: Point,
    /// Configurable session dialog items.
    session_menu_items: Vec<SessionMenuItem>,
}

impl Shell {
    /// Create a new shell for the given screen dimensions.
    #[must_use]
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        let config = ShellConfig::default();
        Self::from_config(config, screen_width, screen_height)
    }

    /// Create a new shell from a full configuration.
    #[must_use]
    pub fn from_config(config: ShellConfig, screen_width: f32, screen_height: f32) -> Self {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let mut dock = Dock::new(config.dock.clone());
        // Add default pinned apps so the dock is visible at startup.
        dock.add_pinned("com.liquide.files", "Files", "folder");
        dock.add_pinned("com.liquide.terminal", "Terminal", "terminal");
        dock.add_pinned("com.liquide.browser", "Browser", "web-browser");
        dock.add_pinned("com.liquide.settings", "Settings", "preferences-system");

        let mut launcher = Launcher::new(config.launcher.clone());
        // Register default apps so they appear in launcher search.
        Self::register_default_apps(&mut launcher);

        Self {
            windows: HashMap::new(),
            workspaces: WorkspaceManager::new(),
            focus: FocusManager::new(FocusPolicy::ClickToFocus),
            layout: Box::new(FloatingLayout),
            decoration_style: DecorationStyle::default(),
            next_window_id: 1,
            screen_rect: Rect::new(0.0, 0.0, screen_width, screen_height),
            window_history: WindowHistory::new(1000),
            app_history: AppHistory::new(100),
            screen_time: ScreenTimeTracker::new(now_us, 1),
            next_event_timestamp: 1,
            dock,
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher,
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
            theme: ShellTheme::default_dark(),
            session_menu_visible: false,
            context_menu_visible: false,
            context_menu_pos: Point::new(0.0, 0.0),
            session_menu_items: SessionMenuItem::defaults(),
        }
    }

    /// Create a new shell with custom history capacities.
    #[must_use]
    pub fn with_history_capacity(
        screen_width: f32,
        screen_height: f32,
        window_history_capacity: usize,
        app_history_capacity: usize,
    ) -> Self {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let config = ShellConfig::default();
        let mut dock = Dock::new(config.dock.clone());
        dock.add_pinned("com.liquide.files", "Files", "folder");
        dock.add_pinned("com.liquide.terminal", "Terminal", "terminal");
        dock.add_pinned("com.liquide.browser", "Browser", "web-browser");
        dock.add_pinned("com.liquide.settings", "Settings", "preferences-system");
        let mut launcher = Launcher::new(config.launcher.clone());
        Self::register_default_apps(&mut launcher);
        Self {
            windows: HashMap::new(),
            workspaces: WorkspaceManager::new(),
            focus: FocusManager::new(FocusPolicy::ClickToFocus),
            layout: Box::new(FloatingLayout),
            decoration_style: DecorationStyle::default(),
            next_window_id: 1,
            screen_rect: Rect::new(0.0, 0.0, screen_width, screen_height),
            window_history: WindowHistory::new(window_history_capacity),
            app_history: AppHistory::new(app_history_capacity),
            screen_time: ScreenTimeTracker::new(now_us, 1),
            next_event_timestamp: 1,
            dock,
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher,
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
            theme: ShellTheme::default_dark(),
            session_menu_visible: false,
            context_menu_visible: false,
            context_menu_pos: Point::new(0.0, 0.0),
            session_menu_items: SessionMenuItem::defaults(),
        }
    }

    /// Get the next event timestamp and advance the counter.
    fn next_timestamp(&mut self) -> u64 {
        let ts = self.next_event_timestamp;
        self.next_event_timestamp += 1;
        ts
    }

    // ====================================================================
    // Window operations
    // ====================================================================

    /// Open a new window. Returns its ID.
    pub fn open_window(&mut self, title: impl Into<String>, bounds: Rect) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let window = Window::new(id, title, bounds);
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Opened, ts);
        id
    }

    /// Open a new window with an application ID. Returns its ID.
    pub fn open_window_with_app(
        &mut self,
        title: impl Into<String>,
        bounds: Rect,
        app_id: impl Into<String>,
    ) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let app_id_str: String = app_id.into();
        let mut window = Window::new(id, title, bounds);
        window.app_id = app_id_str.clone();
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Opened, ts);
        if !app_id_str.is_empty() {
            self.app_history.record_open(&app_id_str, id, bounds, ts);
            self.screen_time.feed_open(&app_id_str, id, ts);
        }
        id
    }

    /// Close a window. Returns the removed window.
    pub fn close_window(&mut self, id: WindowId) -> Result<Window> {
        let window = self
            .windows
            .remove(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        self.workspaces.active_mut().remove_window(id);
        self.focus.remove_window(id);

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Closed, ts);
        if !window.app_id.is_empty() {
            self.app_history
                .record_close(&window.app_id, id, window.bounds, ts);
            self.screen_time.feed_close(&window.app_id, id, ts);
            self.dock.remove_running(&window.app_id);
        }
        Ok(window)
    }

    /// Get a window by ID.
    pub fn window(&self, id: WindowId) -> Result<&Window> {
        self.windows
            .get(&id)
            .ok_or(ShellError::WindowNotFound { id })
    }

    /// Get a window mutably by ID.
    pub fn window_mut(&mut self, id: WindowId) -> Result<&mut Window> {
        self.windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })
    }

    /// Move a window to a new position.
    pub fn move_window(&mut self, id: WindowId, x: f32, y: f32) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.x = x;
        win.bounds.y = y;
        let to = win.bounds;

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Moved { from, to }, ts);
        Ok(())
    }

    /// Resize a window.
    pub fn resize_window(&mut self, id: WindowId, width: f32, height: f32) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.width = width;
        win.bounds.height = height;
        let to = win.bounds;

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Resized { from, to }, ts);
        Ok(())
    }

    /// Minimize a window.
    pub fn minimize(&mut self, id: WindowId) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        win.save_bounds();
        win.state = WindowState::Minimized;
        win.visible = false;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Minimized,
            },
            ts,
        );
        if from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::VisibilityChanged {
                    from: true,
                    to: false,
                },
                ts2,
            );
        }
        Ok(())
    }

    /// Maximize a window to fill the screen.
    pub fn maximize(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_bounds = win.bounds;
        win.save_bounds();
        win.state = WindowState::Maximized;
        win.bounds = screen;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Maximized,
            },
            ts,
        );
        let ts2 = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::Resized {
                from: from_bounds,
                to: screen,
            },
            ts2,
        );
        Ok(())
    }

    /// Restore a window from minimized/maximized/fullscreen.
    pub fn restore(&mut self, id: WindowId) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        let from_bounds = win.bounds;
        win.restore_bounds();
        win.state = WindowState::Normal;
        win.visible = true;
        let to_bounds = win.bounds;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Normal,
            },
            ts,
        );
        if !from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::VisibilityChanged {
                    from: false,
                    to: true,
                },
                ts2,
            );
        }
        if from_bounds != to_bounds {
            let ts3 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::Resized {
                    from: from_bounds,
                    to: to_bounds,
                },
                ts3,
            );
        }
        Ok(())
    }

    /// Toggle fullscreen.
    pub fn toggle_fullscreen(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_bounds = win.bounds;
        if win.state == WindowState::Fullscreen {
            win.restore_bounds();
            win.state = WindowState::Normal;
        } else {
            win.save_bounds();
            win.state = WindowState::Fullscreen;
            win.bounds = screen;
        }
        let to_state = win.state;
        let to_bounds = win.bounds;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: to_state,
            },
            ts,
        );
        if from_bounds != to_bounds {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::Resized {
                    from: from_bounds,
                    to: to_bounds,
                },
                ts2,
            );
        }
        Ok(())
    }

    /// Set focus to a window.
    pub fn set_focus(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        let prev_focused = self.focus.focused();
        self.focus.set_focus(id);

        if let Some(prev_id) = prev_focused {
            if prev_id != id {
                let ts = self.next_timestamp();
                self.window_history
                    .record_at(prev_id, WindowEventKind::Unfocused, ts);
                self.screen_time.feed_unfocus(ts);
            }
        }
        let ts2 = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Focused, ts2);

        let app_id = self
            .windows
            .get(&id)
            .map(|w| w.app_id.clone())
            .unwrap_or_default();
        if !app_id.is_empty() {
            self.screen_time.feed_focus(&app_id, id, ts2);
        }
        Ok(())
    }

    /// Get the number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Get all visible windows, sorted by z_order (ascending).
    #[must_use]
    pub fn visible_windows(&self) -> Vec<&Window> {
        let mut visible: Vec<&Window> = self.windows.values().filter(|w| w.visible).collect();
        visible.sort_by_key(|w| w.z_order);
        visible
    }

    /// Set the layout policy.
    pub fn set_layout(&mut self, layout: Box<dyn LayoutPolicy>) {
        self.layout = layout;
    }

    /// Apply the current layout to visible windows.
    pub fn arrange_windows(&mut self) {
        let screen = self.screen_rect;
        let mut visible_ids: Vec<WindowId> = self
            .windows
            .values()
            .filter(|w| w.visible)
            .map(|w| w.id)
            .collect();
        visible_ids.sort_by_key(|id| id.0);

        let mut window_vec: Vec<Window> = visible_ids
            .iter()
            .filter_map(|id| self.windows.get(id).cloned())
            .collect();

        self.layout.arrange(&mut window_vec, screen);

        for win in window_vec {
            if let Some(existing) = self.windows.get_mut(&win.id) {
                existing.bounds = win.bounds;
            }
        }
    }

    /// Get the screen rect.
    #[must_use]
    pub fn screen_rect(&self) -> Rect {
        self.screen_rect
    }

    /// Resize the screen.
    pub fn resize_screen(&mut self, width: f32, height: f32) {
        self.screen_rect = Rect::new(0.0, 0.0, width, height);
    }

    /// Get the focus manager.
    #[must_use]
    pub fn focus_manager(&self) -> &FocusManager {
        &self.focus
    }

    /// Get the decoration style.
    #[must_use]
    pub fn decoration_style(&self) -> &DecorationStyle {
        &self.decoration_style
    }

    /// Get the focus manager mutably.
    pub fn focus_manager_mut(&mut self) -> &mut FocusManager {
        &mut self.focus
    }

    /// Get the workspace manager.
    #[must_use]
    pub fn workspace_manager(&self) -> &WorkspaceManager {
        &self.workspaces
    }

    /// Get the workspace manager mutably.
    pub fn workspace_manager_mut(&mut self) -> &mut WorkspaceManager {
        &mut self.workspaces
    }

    /// Raise a window to the top (highest z_order).
    pub fn raise_window(&mut self, id: WindowId) -> Result<()> {
        let max_z = self.windows.values().map(|w| w.z_order).max().unwrap_or(0);
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_z = win.z_order;
        win.z_order = max_z + 1;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::ZOrderChanged {
                from: from_z,
                to: max_z + 1,
            },
            ts,
        );
        Ok(())
    }

    /// Lower a window to the bottom (lowest z_order).
    pub fn lower_window(&mut self, id: WindowId) -> Result<()> {
        let min_z = self.windows.values().map(|w| w.z_order).min().unwrap_or(0);
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_z = win.z_order;
        win.z_order = min_z - 1;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::ZOrderChanged {
                from: from_z,
                to: min_z - 1,
            },
            ts,
        );
        Ok(())
    }

    /// Set the decoration style.
    pub fn set_decoration_style(&mut self, style: DecorationStyle) {
        self.decoration_style = style;
    }

    /// Get the window history.
    #[must_use]
    pub fn window_history(&self) -> &WindowHistory {
        &self.window_history
    }

    /// Get the application history.
    #[must_use]
    pub fn app_history(&self) -> &AppHistory {
        &self.app_history
    }

    /// Get a statistics collector for the current history.
    #[must_use]
    pub fn stats(&self) -> StatsCollector<'_> {
        StatsCollector::new(&self.window_history, &self.app_history)
    }

    /// Get the screen time tracker.
    #[must_use]
    pub fn screen_time(&self) -> &ScreenTimeTracker {
        &self.screen_time
    }

    /// Get the screen time tracker mutably.
    pub fn screen_time_mut(&mut self) -> &mut ScreenTimeTracker {
        &mut self.screen_time
    }

    // ====================================================================
    // New subsystem accessors
    // ====================================================================

    /// Get the dock.
    #[must_use]
    pub fn dock(&self) -> &Dock {
        &self.dock
    }

    /// Get the dock mutably.
    pub fn dock_mut(&mut self) -> &mut Dock {
        &mut self.dock
    }

    /// Get the status bar.
    #[must_use]
    pub fn status_bar(&self) -> &ShellStatusBar {
        &self.status_bar
    }

    /// Get the status bar mutably.
    pub fn status_bar_mut(&mut self) -> &mut ShellStatusBar {
        &mut self.status_bar
    }

    /// Get the app launcher.
    #[must_use]
    pub fn launcher(&self) -> &Launcher {
        &self.launcher
    }

    /// Get the app launcher mutably.
    pub fn launcher_mut(&mut self) -> &mut Launcher {
        &mut self.launcher
    }

    /// Get the tiling engine.
    #[must_use]
    pub fn tiling(&self) -> &TilingEngine {
        &self.tiling
    }

    /// Get the tiling engine mutably.
    pub fn tiling_mut(&mut self) -> &mut TilingEngine {
        &mut self.tiling
    }

    /// Get the shortcut manager.
    #[must_use]
    pub fn shortcuts(&self) -> &ShortcutManager {
        &self.shortcuts
    }

    /// Get the shortcut manager mutably.
    pub fn shortcuts_mut(&mut self) -> &mut ShortcutManager {
        &mut self.shortcuts
    }

    /// Get the notification manager.
    #[must_use]
    pub fn notifications(&self) -> &NotificationManager {
        &self.notifications
    }

    /// Get the notification manager mutably.
    pub fn notifications_mut(&mut self) -> &mut NotificationManager {
        &mut self.notifications
    }

    /// Get the seamless manager.
    #[must_use]
    pub fn seamless(&self) -> &SeamlessManager {
        &self.seamless
    }

    /// Get the seamless manager mutably.
    pub fn seamless_mut(&mut self) -> &mut SeamlessManager {
        &mut self.seamless
    }

    /// Get the shell configuration.
    #[must_use]
    pub fn config(&self) -> &ShellConfig {
        &self.config
    }

    /// Get the current shell theme.
    #[must_use]
    pub fn theme(&self) -> &ShellTheme {
        &self.theme
    }

    /// Set the shell theme.
    pub fn set_theme(&mut self, theme: ShellTheme) {
        self.theme = theme;
    }

    /// Handle a key event, returning the matching shell action if any.
    #[must_use]
    pub fn handle_key_event(&self, event: &KeyEvent) -> Option<&ShellAction> {
        self.shortcuts.handle_key_event(event)
    }

    /// Whether the session menu is currently visible.
    #[must_use]
    pub fn session_menu_visible(&self) -> bool {
        self.session_menu_visible
    }

    /// Toggle the session menu overlay.
    pub fn toggle_session_menu(&mut self) {
        self.session_menu_visible = !self.session_menu_visible;
    }

    // ====================================================================
    // App launching helpers
    // ====================================================================

    /// Register built-in applications with the launcher.
    fn register_default_apps(launcher: &mut Launcher) {
        let defaults = [
            ("com.liquide.files", "Files", "folder", "File manager"),
            (
                "com.liquide.terminal",
                "Terminal",
                "terminal",
                "Command line",
            ),
            (
                "com.liquide.browser",
                "Browser",
                "web-browser",
                "Web browser",
            ),
            (
                "com.liquide.settings",
                "Settings",
                "preferences-system",
                "System settings",
            ),
            (
                "com.liquide.calculator",
                "Calculator",
                "calculator",
                "Calculator",
            ),
        ];
        for (app_id, name, icon, desc) in &defaults {
            launcher.add_app(LauncherApp {
                app_id: app_id.to_string(),
                name: name.to_string(),
                description: Some(desc.to_string()),
                icon: Some(icon.to_string()),
                exec: None,
                categories: Vec::new(),
                keywords: Vec::new(),
                terminal: false,
                no_display: false,
                launch_count: 0,
                last_launched_us: 0,
            });
        }
    }

    /// Open a new window for the given application, or focus an existing one.
    ///
    /// Returns the window ID of the focused or newly created window.
    pub fn open_app_window(&mut self, app_id: &str) -> WindowId {
        // Check if a window with this app_id is already open.
        if let Some(existing) = self
            .windows
            .values()
            .find(|w| w.app_id == app_id && w.visible)
        {
            let wid = existing.id;
            let _ = self.set_focus(wid);
            let _ = self.raise_window(wid);
            return wid;
        }

        let screen = self.screen_rect;
        let (title, w, h): (&str, f32, f32) = match app_id {
            "com.liquide.settings" => ("Settings", 700.0, 500.0),
            "com.liquide.terminal" => ("Terminal", 720.0, 480.0),
            "com.liquide.files" => ("Files", 800.0, 550.0),
            "com.liquide.browser" => ("Browser", 900.0, 600.0),
            "com.liquide.calculator" => ("Calculator", 360.0, 420.0),
            _ => ("Application", 640.0, 480.0),
        };
        let x = (screen.width - w) / 2.0;
        let y = (screen.height - h) / 2.0;

        let id = self.open_window_with_app(title, Rect::new(x, y, w, h), app_id);
        self.dock.add_running(app_id);
        let _ = self.set_focus(id);
        let _ = self.raise_window(id);
        id
    }

    /// Map a `KeyCode` to a lowercase character for text input.
    fn keycode_to_char(key: liquide_input::keyboard::KeyCode) -> Option<char> {
        use liquide_input::keyboard::KeyCode;
        match key {
            KeyCode::A => Some('a'),
            KeyCode::B => Some('b'),
            KeyCode::C => Some('c'),
            KeyCode::D => Some('d'),
            KeyCode::E => Some('e'),
            KeyCode::F => Some('f'),
            KeyCode::G => Some('g'),
            KeyCode::H => Some('h'),
            KeyCode::I => Some('i'),
            KeyCode::J => Some('j'),
            KeyCode::K => Some('k'),
            KeyCode::L => Some('l'),
            KeyCode::M => Some('m'),
            KeyCode::N => Some('n'),
            KeyCode::O => Some('o'),
            KeyCode::P => Some('p'),
            KeyCode::Q => Some('q'),
            KeyCode::R => Some('r'),
            KeyCode::S => Some('s'),
            KeyCode::T => Some('t'),
            KeyCode::U => Some('u'),
            KeyCode::V => Some('v'),
            KeyCode::W => Some('w'),
            KeyCode::X => Some('x'),
            KeyCode::Y => Some('y'),
            KeyCode::Z => Some('z'),
            KeyCode::Digit0 => Some('0'),
            KeyCode::Digit1 => Some('1'),
            KeyCode::Digit2 => Some('2'),
            KeyCode::Digit3 => Some('3'),
            KeyCode::Digit4 => Some('4'),
            KeyCode::Digit5 => Some('5'),
            KeyCode::Digit6 => Some('6'),
            KeyCode::Digit7 => Some('7'),
            KeyCode::Digit8 => Some('8'),
            KeyCode::Digit9 => Some('9'),
            KeyCode::Space => Some(' '),
            KeyCode::Minus => Some('-'),
            KeyCode::Equal => Some('='),
            KeyCode::Period => Some('.'),
            KeyCode::Comma => Some(','),
            KeyCode::Slash => Some('/'),
            _ => None,
        }
    }

    /// Build the complete shell scene graph.
    ///
    /// Assembles: background, active workspace with window decorations,
    /// status bar, dock, notifications, and launcher overlay.
    pub fn build_scene(&self) -> SceneNode {
        use crate::scene_builder::*;
        use liquide_compositor::scene::GlassParams;

        let screen = self.screen_rect;
        let theme = &self.theme;

        let mut root = SceneNode::new(NODE_ROOT, SceneNodeKind::Root, NodeProperties::new(screen));

        // Background
        root.add_child(solid_rect(
            NODE_BACKGROUND,
            theme.desktop_background,
            screen,
            0,
        ));

        // Active workspace
        let ws = self.workspaces.active();
        let ws_id = NODE_WORKSPACE_BASE + ws.id.0 as u64;
        let mut ws_node = SceneNode::new(
            ws_id,
            SceneNodeKind::Workspace { index: ws.id.0 },
            NodeProperties::new(screen).with_z_order(1),
        );

        // Windows back-to-front
        for window in &self.visible_windows() {
            let win_base = NODE_WINDOW_BASE + window.id.0 * NODE_WINDOW_STRIDE;

            // Shadow
            let shadow_bounds = Rect::new(
                window.bounds.x - 4.0,
                window.bounds.y - 2.0,
                window.bounds.width + 8.0,
                window.bounds.height + 6.0,
            );
            ws_node.add_child(SceneNode::new(
                win_base,
                SceneNodeKind::Shadow {
                    spread: 4.0,
                    blur_radius: 12.0,
                    color: theme.window_shadow,
                },
                NodeProperties::new(shadow_bounds).with_z_order(window.z_order as u32 * 10),
            ));

            // Decoration
            if window.flags.contains(WindowFlags::DECORATED) {
                let is_focused = self.focus.focused() == Some(window.id);
                let title_bg = if is_focused {
                    theme.window_title_bar_focused
                } else {
                    theme.window_title_bar_unfocused
                };
                ws_node.add_child(SceneNode::new(
                    win_base + 1,
                    SceneNodeKind::Decoration {
                        title: Some(window.title.clone()),
                        title_color: theme.window_title_text,
                        background: title_bg,
                        border_color: if is_focused {
                            theme.window_border_focused
                        } else {
                            theme.window_border_unfocused
                        },
                        border_width: self.decoration_style.border_width,
                        corner_radius: self.decoration_style.corner_radius,
                        button_state: DecorationButtons {
                            close: true,
                            maximize: true,
                            minimize: true,
                        },
                    },
                    NodeProperties::new(window.bounds).with_z_order(window.z_order as u32 * 10 + 1),
                ));
            }

            // Content surface
            let title_h = if window.flags.contains(WindowFlags::DECORATED) {
                self.decoration_style.title_bar_height
            } else {
                0.0
            };
            let content_bounds = Rect::new(
                window.bounds.x,
                window.bounds.y + title_h,
                window.bounds.width,
                (window.bounds.height - title_h).max(0.0),
            );
            let z_content = window.z_order as u32 * 10 + 2;

            // Background fill for the content area.
            let content_bg = liquide_compositor::pixel::Color::new(35, 35, 40, 255);
            ws_node.add_child(solid_rect(
                win_base + 2,
                content_bg,
                content_bounds,
                z_content,
            ));

            // App-specific content nodes.
            self.build_window_content(
                &mut ws_node,
                window,
                content_bounds,
                win_base,
                z_content,
                theme,
            );
        }
        root.add_child(ws_node);

        // Status bar
        root.add_child(self.status_bar.build_scene(screen, theme));

        // Dock
        if self.dock.is_visible() || !self.dock.config().auto_hide {
            root.add_child(self.dock.build_scene(screen, theme));
        }

        // Notifications
        root.add_child(self.notifications.build_scene(screen, theme));

        // Launcher (on top of everything)
        if self.launcher.is_visible() {
            root.add_child(self.launcher.build_scene(screen, theme));
        }

        // Session menu (anchored below the session button on the status bar)
        if self.session_menu_visible {
            let menu_w = 180.0_f32;
            let item_h = 36.0_f32;
            let menu_h = 16.0 + self.session_menu_items.len() as f32 * item_h;
            let bar_h = self.status_bar.config().height as f32;
            let menu_x = screen.width - menu_w - 8.0;
            let menu_y = bar_h + 4.0;
            let menu_bounds = Rect::new(menu_x, menu_y, menu_w, menu_h);

            root.add_child(SceneNode::new(
                NODE_SESSION_MENU,
                SceneNodeKind::Glass(GlassParams {
                    blur_radius: 20,
                    tint_color: theme.dock_glass_tint,
                    inner_glow: true,
                    parallax: false,
                }),
                NodeProperties::new(menu_bounds).with_z_order(990),
            ));

            for (i, item) in self.session_menu_items.iter().enumerate() {
                let iy = menu_y + 8.0 + i as f32 * item_h;
                let icon_id = icon_id_for_name(&item.icon);
                root.add_child(icon_node(
                    NODE_SESSION_MENU + 10 + i as u64 * 2,
                    icon_id,
                    theme.status_bar_text,
                    Rect::new(menu_x + 14.0, iy + 4.0, 24.0, 24.0),
                    991,
                ));
                root.add_child(text_node(
                    NODE_SESSION_MENU + 11 + i as u64 * 2,
                    item.label.clone(),
                    theme.status_bar_text,
                    Rect::new(menu_x + 44.0, iy + 6.0, menu_w - 60.0, 20.0),
                    991,
                    1,
                ));
            }
        }

        // Desktop right-click context menu
        if self.context_menu_visible {
            let ctx_items = ContextMenuItem::defaults();
            let ctx_item_h = 36.0_f32;
            let ctx_w = 260.0_f32;
            let ctx_h = 16.0 + ctx_items.len() as f32 * ctx_item_h;
            // Clamp position so menu stays on-screen.
            let ctx_x = self
                .context_menu_pos
                .x
                .min(screen.width - ctx_w - 4.0)
                .max(0.0);
            let ctx_y = self
                .context_menu_pos
                .y
                .min(screen.height - ctx_h - 4.0)
                .max(0.0);
            let ctx_bounds = Rect::new(ctx_x, ctx_y, ctx_w, ctx_h);

            root.add_child(SceneNode::new(
                NODE_CONTEXT_MENU,
                SceneNodeKind::Glass(GlassParams {
                    blur_radius: 20,
                    tint_color: theme.dock_glass_tint,
                    inner_glow: true,
                    parallax: false,
                }),
                NodeProperties::new(ctx_bounds).with_z_order(995),
            ));

            for (i, item) in ctx_items.iter().enumerate() {
                let iy = ctx_y + 8.0 + i as f32 * ctx_item_h;
                let icon_id = icon_id_for_name(&item.icon);
                root.add_child(icon_node(
                    NODE_CONTEXT_MENU + 10 + i as u64 * 2,
                    icon_id,
                    theme.status_bar_text,
                    Rect::new(ctx_x + 12.0, iy + 4.0, 24.0, 24.0),
                    996,
                ));
                root.add_child(text_node(
                    NODE_CONTEXT_MENU + 11 + i as u64 * 2,
                    item.label.clone(),
                    theme.status_bar_text,
                    Rect::new(ctx_x + 44.0, iy + 6.0, ctx_w - 60.0, 20.0),
                    996,
                    1,
                ));
            }
        }

        root
    }

    /// Render app-specific content inside a window's content area.
    fn build_window_content(
        &self,
        parent: &mut SceneNode,
        window: &Window,
        content: Rect,
        win_base: u64,
        z: u32,
        theme: &ShellTheme,
    ) {
        use crate::scene_builder::*;

        let text_color = theme.status_bar_text;
        let cx = content.x;
        let cy = content.y;
        let cw = content.width;

        match window.app_id.as_str() {
            "com.liquide.settings" => {
                // Settings heading
                parent.add_child(icon_node(
                    win_base + 3,
                    4,
                    text_color,
                    Rect::new(cx + 20.0, cy + 16.0, 28.0, 28.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "Settings".into(),
                    text_color,
                    Rect::new(cx + 56.0, cy + 20.0, 200.0, 20.0),
                    z + 1,
                    1,
                ));
                // Category list
                let categories = [
                    "Display",
                    "Input",
                    "Audio",
                    "Network",
                    "Appearance",
                    "Privacy",
                    "Users",
                    "System",
                ];
                for (i, cat) in categories.iter().enumerate() {
                    let iy = cy + 60.0 + i as f32 * 32.0;
                    // Sidebar item background
                    let item_bg = liquide_compositor::pixel::Color::new(45, 45, 55, 200);
                    parent.add_child(solid_rect(
                        win_base + 5 + i as u64,
                        item_bg,
                        Rect::new(cx + 8.0, iy, 160.0, 28.0),
                        z + 1,
                    ));
                    parent.add_child(text_node(
                        win_base + 50 + i as u64,
                        cat.to_string(),
                        text_color,
                        Rect::new(cx + 16.0, iy + 4.0, 140.0, 20.0),
                        z + 2,
                        1,
                    ));
                }
            }
            "com.liquide.terminal" => {
                // Dark terminal background
                let term_bg = liquide_compositor::pixel::Color::new(20, 20, 25, 255);
                parent.add_child(solid_rect(win_base + 3, term_bg, content, z + 1));
                parent.add_child(text_node(
                    win_base + 4,
                    "user@liquide:~$".into(),
                    liquide_compositor::pixel::Color::new(100, 220, 100, 255),
                    Rect::new(cx + 12.0, cy + 12.0, cw - 24.0, 20.0),
                    z + 2,
                    1,
                ));
            }
            "com.liquide.files" => {
                parent.add_child(icon_node(
                    win_base + 3,
                    1,
                    text_color,
                    Rect::new(cx + 20.0, cy + 16.0, 28.0, 28.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "Home".into(),
                    text_color,
                    Rect::new(cx + 56.0, cy + 20.0, 200.0, 20.0),
                    z + 1,
                    1,
                ));
                let folders = ["Documents", "Downloads", "Pictures", "Music", "Desktop"];
                for (i, name) in folders.iter().enumerate() {
                    let iy = cy + 60.0 + i as f32 * 32.0;
                    parent.add_child(icon_node(
                        win_base + 5 + i as u64,
                        1,
                        text_color,
                        Rect::new(cx + 24.0, iy + 2.0, 24.0, 24.0),
                        z + 1,
                    ));
                    parent.add_child(text_node(
                        win_base + 50 + i as u64,
                        name.to_string(),
                        text_color,
                        Rect::new(cx + 56.0, iy + 4.0, 200.0, 20.0),
                        z + 2,
                        1,
                    ));
                }
            }
            "com.liquide.browser" => {
                // URL bar
                let bar_bg = liquide_compositor::pixel::Color::new(55, 55, 65, 255);
                parent.add_child(solid_rect(
                    win_base + 3,
                    bar_bg,
                    Rect::new(cx + 8.0, cy + 8.0, cw - 16.0, 32.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "liquide://home".into(),
                    text_color,
                    Rect::new(cx + 16.0, cy + 14.0, cw - 32.0, 20.0),
                    z + 2,
                    1,
                ));
                // Page placeholder
                parent.add_child(text_node(
                    win_base + 5,
                    "Welcome to Liquide Browser".into(),
                    text_color,
                    Rect::new(cx + 20.0, cy + 60.0, cw - 40.0, 20.0),
                    z + 2,
                    1,
                ));
            }
            "com.liquide.calculator" => {
                parent.add_child(icon_node(
                    win_base + 3,
                    5,
                    text_color,
                    Rect::new(cx + cw / 2.0 - 24.0, cy + 20.0, 48.0, 48.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "0".into(),
                    text_color,
                    Rect::new(cx + 16.0, cy + 80.0, cw - 32.0, 24.0),
                    z + 1,
                    1,
                ));
            }
            _ => {
                // Generic: show the window title centered
                parent.add_child(text_node(
                    win_base + 3,
                    window.title.clone(),
                    text_color,
                    Rect::new(cx + 20.0, cy + content.height / 2.0 - 10.0, cw - 40.0, 20.0),
                    z + 1,
                    1,
                ));
            }
        }
    }

    /// Handle a platform event and return any resulting shell action.
    pub fn handle_platform_event(&mut self, event: &PlatformEvent) -> Option<ShellAction> {
        use liquide_input::keyboard::{KeyCode, KeyState};
        use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};

        match event {
            PlatformEvent::KeyInput { event: ke, .. } => {
                if ke.state != KeyState::Pressed {
                    return None;
                }

                // When the launcher is visible, route keyboard input to it.
                if self.launcher.is_visible() {
                    match ke.key {
                        KeyCode::Escape => {
                            self.launcher.close();
                            return Some(ShellAction::OpenLauncher); // toggles → redraws
                        }
                        KeyCode::ArrowUp => {
                            self.launcher.select_prev();
                            return Some(ShellAction::OpenLauncher);
                        }
                        KeyCode::ArrowDown => {
                            self.launcher.select_next();
                            return Some(ShellAction::OpenLauncher);
                        }
                        KeyCode::Enter => {
                            if let Some(kind) = self.launcher.activate_selected().cloned() {
                                self.launcher.close();
                                match kind {
                                    SearchResultKind::Application { ref app_id } => {
                                        self.open_app_window(app_id);
                                    }
                                    _ => {}
                                }
                            } else {
                                self.launcher.close();
                            }
                            return Some(ShellAction::OpenLauncher);
                        }
                        KeyCode::Backspace => {
                            let q = self.launcher.query().to_string();
                            if !q.is_empty() {
                                let new_q = &q[..q.len() - 1];
                                self.launcher.set_query(new_q);
                            }
                            return Some(ShellAction::OpenLauncher);
                        }
                        other => {
                            if let Some(ch) = Self::keycode_to_char(other) {
                                let mut q = self.launcher.query().to_string();
                                q.push(ch);
                                self.launcher.set_query(&q);
                                return Some(ShellAction::OpenLauncher);
                            }
                            return None;
                        }
                    }
                }

                // When the context menu is visible, Escape closes it.
                if self.context_menu_visible && ke.key == KeyCode::Escape {
                    self.context_menu_visible = false;
                    return Some(ShellAction::OpenLauncher); // triggers redraw
                }

                // When the session menu is visible, Escape closes it.
                if self.session_menu_visible && ke.key == KeyCode::Escape {
                    self.session_menu_visible = false;
                    return Some(ShellAction::OpenLauncher); // triggers redraw
                }

                // Normal shortcut dispatch.
                self.shortcuts.handle_key_event(ke).cloned()
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                match me {
                    MouseEvent::Move { x, y } => {
                        // Update dock hover
                        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
                        if dock_bounds.contains(Point::new(*x, *y)) {
                            let item_rects = self.dock.compute_item_rects(self.screen_rect);
                            let mut found = None;
                            for (i, (_, rect)) in item_rects.iter().enumerate() {
                                if rect.contains(Point::new(*x, *y)) {
                                    found = Some(i);
                                    break;
                                }
                            }
                            if let Some(idx) = found {
                                self.dock.on_hover(idx);
                            } else {
                                self.dock.on_hover_leave();
                            }
                        } else {
                            self.dock.on_hover_leave();
                        }
                        None
                    }
                    MouseEvent::Button {
                        button,
                        state,
                        x,
                        y,
                    } => {
                        if *state != ButtonState::Pressed {
                            return None;
                        }
                        let pt = Point::new(*x, *y);

                        // --- Right-click: desktop context menu ---
                        if *button == MouseButton::Right {
                            // Close any open menus first.
                            self.session_menu_visible = false;

                            // Only show context menu when clicking empty desktop
                            // (not on dock, status bar, or window).
                            let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
                            let dock_bounds = self.dock.compute_bounds(self.screen_rect);
                            let on_window = self
                                .visible_windows()
                                .iter()
                                .rev()
                                .any(|w| w.bounds.contains(pt));
                            if !bar_bounds.contains(pt) && !dock_bounds.contains(pt) && !on_window {
                                self.context_menu_visible = !self.context_menu_visible;
                                self.context_menu_pos = pt;
                                return Some(ShellAction::OpenLauncher); // trigger redraw
                            }
                            return None;
                        }

                        if *button != MouseButton::Left {
                            return None;
                        }

                        // --- Context menu interaction (left click) ---
                        if self.context_menu_visible {
                            let ctx_items = ContextMenuItem::defaults();
                            let ctx_item_h = 36.0_f32;
                            let ctx_w = 260.0_f32;
                            let ctx_h = 16.0 + ctx_items.len() as f32 * ctx_item_h;
                            let ctx_x = self
                                .context_menu_pos
                                .x
                                .min(self.screen_rect.width - ctx_w - 4.0)
                                .max(0.0);
                            let ctx_y = self
                                .context_menu_pos
                                .y
                                .min(self.screen_rect.height - ctx_h - 4.0)
                                .max(0.0);
                            let ctx_bounds = Rect::new(ctx_x, ctx_y, ctx_w, ctx_h);

                            if ctx_bounds.contains(pt) {
                                let rel_y = *y - ctx_y - 8.0;
                                let idx = (rel_y / ctx_item_h) as usize;
                                self.context_menu_visible = false;
                                if idx < ctx_items.len() {
                                    return Some(ctx_items[idx].action.clone());
                                }
                                return None;
                            }
                            // Click outside context menu → close it.
                            self.context_menu_visible = false;
                            // Fall through to normal click handling.
                        }

                        // --- Session menu interaction ---
                        if self.session_menu_visible {
                            let menu_w = 180.0_f32;
                            let item_h = 36.0_f32;
                            let menu_h = 16.0 + self.session_menu_items.len() as f32 * item_h;
                            let bar_h = self.status_bar.config().height as f32;
                            let menu_x = self.screen_rect.width - menu_w - 8.0;
                            let menu_y = bar_h + 4.0;
                            let menu_bounds = Rect::new(menu_x, menu_y, menu_w, menu_h);

                            if menu_bounds.contains(pt) {
                                let rel_y = *y - menu_y - 8.0;
                                let idx = (rel_y / item_h) as usize;
                                self.session_menu_visible = false;
                                if idx < self.session_menu_items.len() {
                                    return Some(self.session_menu_items[idx].action.clone());
                                }
                                return None;
                            }
                            // Click outside menu → close it.
                            self.session_menu_visible = false;
                            // Fall through to normal click handling.
                        }

                        // --- Launcher click handling ---
                        if self.launcher.is_visible() {
                            let screen = self.screen_rect;
                            let panel_w = screen.width * 0.6;
                            let panel_h = screen.height * 0.7;
                            let panel_x = screen.x + (screen.width - panel_w) / 2.0;
                            let panel_y = screen.y + (screen.height - panel_h) / 2.0;
                            let panel_bounds = Rect::new(panel_x, panel_y, panel_w, panel_h);

                            if !panel_bounds.contains(pt) {
                                // Click outside launcher → close it.
                                self.launcher.close();
                                return Some(ShellAction::OpenLauncher);
                            }

                            // Click inside the item area → select and activate.
                            let item_start_y = panel_y + 65.0;
                            let item_height = 40.0_f32;
                            let item_gap = 4.0_f32;
                            if *y >= item_start_y {
                                let rel_y = *y - item_start_y;
                                let idx = (rel_y / (item_height + item_gap)) as usize;
                                self.launcher.select_index(idx);
                                if let Some(kind) = self.launcher.activate_selected().cloned() {
                                    self.launcher.close();
                                    if let SearchResultKind::Application { ref app_id } = kind {
                                        self.open_app_window(app_id);
                                    }
                                }
                                return Some(ShellAction::OpenLauncher);
                            }

                            return None;
                        }

                        // --- Status bar click (session button) ---
                        let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
                        if bar_bounds.contains(pt) {
                            // Check if the click is on the session button (rightmost ~36px).
                            let session_x = self.screen_rect.width - 36.0;
                            if *x >= session_x {
                                self.session_menu_visible = !self.session_menu_visible;
                                return Some(ShellAction::OpenSessionMenu);
                            }
                            return None;
                        }

                        // --- Dock click (launch or focus app) ---
                        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
                        if dock_bounds.contains(pt) {
                            let item_rects = self.dock.compute_item_rects(self.screen_rect);
                            for (i, (_, rect)) in item_rects.iter().enumerate() {
                                if rect.contains(pt) {
                                    let items = self.dock.items();
                                    if i < items.len() {
                                        let app_id = items[i].app_id.clone();
                                        if !app_id.is_empty() {
                                            self.open_app_window(&app_id);
                                            // Return OpenLauncher to trigger a redraw
                                            // without minimizing all windows.
                                            return Some(ShellAction::OpenLauncher);
                                        }
                                    }
                                    break;
                                }
                            }
                            return None;
                        }

                        // --- Window click with decoration hit-testing ---
                        let mut clicked = None;
                        let tbh = self.decoration_style.title_bar_height;
                        for window in self.visible_windows().into_iter().rev() {
                            // The window.bounds includes the title bar, so
                            // a click anywhere in window.bounds can be a title/content hit.
                            if window.bounds.contains(pt) {
                                clicked = Some(window.id);
                                break;
                            }
                        }

                        if let Some(wid) = clicked {
                            let is_decorated = self
                                .windows
                                .get(&wid)
                                .map(|w| w.flags.contains(WindowFlags::DECORATED))
                                .unwrap_or(false);

                            if is_decorated {
                                let bounds = self.windows[&wid].bounds;
                                // hit_test_decoration expects the client (content) rect.
                                let client = Rect::new(
                                    bounds.x,
                                    bounds.y + tbh,
                                    bounds.width,
                                    (bounds.height - tbh).max(0.0),
                                );
                                let zone =
                                    hit_test_decoration(client, &self.decoration_style, *x, *y);
                                match zone {
                                    HitZone::CloseButton => {
                                        let _ = self.set_focus(wid);
                                        return Some(ShellAction::CloseWindow);
                                    }
                                    HitZone::MaximizeButton => {
                                        let _ = self.set_focus(wid);
                                        return Some(ShellAction::MaximizeWindow);
                                    }
                                    HitZone::MinimizeButton => {
                                        let _ = self.set_focus(wid);
                                        return Some(ShellAction::MinimizeWindow);
                                    }
                                    _ => {
                                        let _ = self.set_focus(wid);
                                        let _ = self.raise_window(wid);
                                    }
                                }
                            } else {
                                let _ = self.set_focus(wid);
                                let _ = self.raise_window(wid);
                            }
                        }
                        None
                    }
                    _ => None,
                }
            }
            PlatformEvent::WindowResized { width, height, .. } => {
                self.resize_screen(*width as f32, *height as f32);
                None
            }
            _ => None,
        }
    }

    /// Periodic tick — update clock, expire notifications.
    ///
    /// Returns `true` if something visually changed (notification expired,
    /// status bar updated, etc.) and a redraw is needed.
    pub fn tick(&mut self, now_us: u64) -> bool {
        self.status_bar.update_clock(now_us);
        self.status_bar
            .update_notification_count(self.notifications.unread_count() as u32);
        let expired = self.notifications.tick(now_us);
        let bar_dirty = self.status_bar.is_dirty();
        if bar_dirty {
            self.status_bar.mark_clean();
        }
        bar_dirty || !expired.is_empty()
    }

    /// Execute a shell action, returns true if a redraw is needed.
    pub fn execute_action(&mut self, action: &ShellAction) -> bool {
        match action {
            ShellAction::OpenLauncher => {
                self.launcher.toggle();
                true
            }
            ShellAction::CloseWindow => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.close_window(wid);
                }
                true
            }
            ShellAction::MaximizeWindow => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.maximize(wid);
                }
                true
            }
            ShellAction::MinimizeWindow => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.minimize(wid);
                }
                true
            }
            ShellAction::RestoreMinimize => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.restore(wid);
                }
                true
            }
            ShellAction::FullscreenToggle => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.toggle_fullscreen(wid);
                }
                true
            }
            ShellAction::SwitchWindowForward => {
                self.focus.focus_next();
                true
            }
            ShellAction::SwitchWindowBackward => {
                self.focus.focus_prev();
                true
            }
            ShellAction::TileLeft => {
                if let Some(wid) = self.focus.focused() {
                    let half_w = self.screen_rect.width / 2.0;
                    let _ = self.move_window(wid, 0.0, 0.0);
                    let _ = self.resize_window(wid, half_w, self.screen_rect.height);
                }
                true
            }
            ShellAction::TileRight => {
                if let Some(wid) = self.focus.focused() {
                    let half_w = self.screen_rect.width / 2.0;
                    let _ = self.move_window(wid, half_w, 0.0);
                    let _ = self.resize_window(wid, half_w, self.screen_rect.height);
                }
                true
            }
            ShellAction::WorkspaceNext => {
                let active = self.workspaces.active().id;
                let count = self.workspaces.workspace_count();
                if (active.0 as usize) < count - 1 {
                    let next = crate::workspace::WorkspaceId(active.0 + 1);
                    let _ = self.workspaces.switch_to(next);
                }
                true
            }
            ShellAction::WorkspacePrev => {
                let active = self.workspaces.active().id;
                if active.0 > 0 {
                    let prev = crate::workspace::WorkspaceId(active.0 - 1);
                    let _ = self.workspaces.switch_to(prev);
                }
                true
            }
            ShellAction::ShowDesktop => {
                let ids: Vec<_> = self.visible_windows().iter().map(|w| w.id).collect();
                for wid in ids {
                    let _ = self.minimize(wid);
                }
                true
            }
            ShellAction::WorkspaceAdd => {
                let n = self.workspaces.workspace_count();
                self.workspaces
                    .create_workspace(format!("Workspace {}", n + 1));
                true
            }
            ShellAction::OpenSettings => {
                self.open_app_window("com.liquide.settings");
                true
            }
            ShellAction::OpenTerminal => {
                self.open_app_window("com.liquide.terminal");
                true
            }
            ShellAction::OpenFileManager => {
                self.open_app_window("com.liquide.files");
                true
            }
            ShellAction::OpenTaskManager => {
                self.open_app_window("com.liquide.taskmanager");
                true
            }
            ShellAction::OpenSessionMenu => {
                self.session_menu_visible = !self.session_menu_visible;
                true
            }
            ShellAction::LockSession => {
                // Visual feedback only — no real lock in a simulated shell.
                true
            }
            ShellAction::LaunchDockApp(n) => {
                let idx = (*n as usize).saturating_sub(1);
                let app_id = self.dock.items().get(idx).map(|i| i.app_id.clone());
                if let Some(aid) = app_id {
                    if !aid.is_empty() {
                        self.open_app_window(&aid);
                    }
                }
                true
            }
            _ => false,
        }
    }
}
