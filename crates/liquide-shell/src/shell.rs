//! Top-level shell — orchestrates windows, workspaces, focus, layout,
//! dock, status bar, launcher, tiling, shortcuts, notifications, and
//! seamless window mode.

use std::collections::HashMap;

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{DecorationButtons, NodeProperties, SceneNode, SceneNodeKind};
use liquide_input::KeyEvent;
use liquide_platform::PlatformEvent;

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

    // ====================================================================
    // Scene graph, event handling, and periodic updates
    // ====================================================================

    /// Build the complete shell scene graph.
    ///
    /// Assembles: background, active workspace with window decorations,
    /// status bar, dock, notifications, and launcher overlay.
    pub fn build_scene(&self) -> SceneNode {
        use crate::scene_builder::*;
        use crate::window::WindowFlags;

        let screen = self.screen_rect;

        let mut root = SceneNode::new(
            NODE_ROOT,
            SceneNodeKind::Root,
            NodeProperties::new(screen),
        );

        // Background
        root.add_child(solid_rect(
            NODE_BACKGROUND,
            Color::new(30, 60, 90, 255),
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
                    color: Color::new(0, 0, 0, 80),
                },
                NodeProperties::new(shadow_bounds)
                    .with_z_order(window.z_order as u32 * 10),
            ));

            // Decoration
            if window.flags.contains(WindowFlags::DECORATED) {
                let is_focused = self.focus.focused() == Some(window.id);
                let title_bg = if is_focused {
                    Color::new(60, 60, 70, 240)
                } else {
                    Color::new(45, 45, 50, 220)
                };
                ws_node.add_child(SceneNode::new(
                    win_base + 1,
                    SceneNodeKind::Decoration {
                        title: Some(window.title.clone()),
                        title_color: Color::new(220, 220, 220, 255),
                        background: title_bg,
                        border_color: if is_focused {
                            Color::new(80, 140, 220, 200)
                        } else {
                            Color::new(60, 60, 60, 150)
                        },
                        border_width: self.decoration_style.border_width,
                        corner_radius: self.decoration_style.corner_radius,
                        button_state: DecorationButtons {
                            close: true,
                            maximize: true,
                            minimize: true,
                        },
                    },
                    NodeProperties::new(window.bounds)
                        .with_z_order(window.z_order as u32 * 10 + 1),
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
            ws_node.add_child(SceneNode::new(
                win_base + 2,
                SceneNodeKind::Surface {
                    surface_id: window.id.0,
                    buffer: None,
                },
                NodeProperties::new(content_bounds)
                    .with_z_order(window.z_order as u32 * 10 + 2)
                    .with_opacity(window.opacity),
            ));
        }
        root.add_child(ws_node);

        // Status bar
        root.add_child(self.status_bar.build_scene(screen));

        // Dock
        if self.dock.is_visible() || !self.dock.config().auto_hide {
            root.add_child(self.dock.build_scene(screen));
        }

        // Notifications
        root.add_child(self.notifications.build_scene(screen));

        // Launcher (on top of everything)
        if self.launcher.is_visible() {
            root.add_child(self.launcher.build_scene(screen));
        }

        root
    }

    /// Handle a platform event and return any resulting shell action.
    pub fn handle_platform_event(&mut self, event: &PlatformEvent) -> Option<ShellAction> {
        use liquide_input::keyboard::KeyState;
        use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};

        match event {
            PlatformEvent::KeyInput { event: ke, .. } => {
                if ke.state == KeyState::Pressed {
                    self.shortcuts.handle_key_event(ke).cloned()
                } else {
                    None
                }
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
                        if *button == MouseButton::Left && *state == ButtonState::Pressed {
                            // Focus window under cursor
                            let mut clicked = None;
                            for window in self.visible_windows().into_iter().rev() {
                                if window.bounds.contains(Point::new(*x, *y)) {
                                    clicked = Some(window.id);
                                    break;
                                }
                            }
                            if let Some(wid) = clicked {
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
    pub fn tick(&mut self, now_us: u64) {
        self.status_bar.update_clock(now_us);
        self.status_bar
            .update_notification_count(self.notifications.unread_count() as u32);
        self.notifications.tick(now_us);
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
                self.workspaces.create_workspace(format!("Workspace {}", n + 1));
                true
            }
            _ => false,
        }
    }
}
