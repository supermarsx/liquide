//! Top-level shell — orchestrates windows, workspaces, focus, layout,
//! dock, status bar, launcher, tiling, shortcuts, notifications, and
//! seamless window mode.

use std::collections::HashMap;

use liquide_compositor::geometry::Rect;
use liquide_input::KeyEvent;

use crate::app_history::AppHistory;
use crate::config::ShellConfig;
use crate::decoration::DecorationStyle;
use crate::dock::Dock;
use crate::focus::{FocusManager, FocusPolicy};
use crate::history::{WindowEventKind, WindowHistory};
use crate::launcher::Launcher;
use crate::layout::{FloatingLayout, LayoutPolicy};
use crate::notification::NotificationManager;
use crate::screen_time::ScreenTimeTracker;
use crate::seamless::SeamlessManager;
use crate::shortcuts::{ShellAction, ShortcutManager};
use crate::stats::StatsCollector;
use crate::status_bar::ShellStatusBar;
use crate::tiling::TilingEngine;
use crate::window::{Window, WindowId, WindowState};
use crate::workspace::WorkspaceManager;
use crate::{ShellError, Result};

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
            dock: Dock::new(config.dock.clone()),
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher: Launcher::new(config.launcher.clone()),
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
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
            dock: Dock::new(config.dock.clone()),
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher: Launcher::new(config.launcher.clone()),
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
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
            self.app_history
                .record_open(&app_id_str, id, bounds, ts);
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

    /// Handle a key event, returning the matching shell action if any.
    #[must_use]
    pub fn handle_key_event(&self, event: &KeyEvent) -> Option<&ShellAction> {
        self.shortcuts.handle_key_event(event)
    }
}
