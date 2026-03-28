//! Subsystem accessor methods — dock, status bar, launcher, tiling,
//! shortcuts, notifications, seamless, config, theme, and related getters.

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::CursorShape;
use liquide_input::KeyEvent;

use crate::app_history::AppHistory;
use crate::config::ShellConfig;
use crate::decoration::DecorationStyle;
use crate::focus::FocusManager;
use crate::history::WindowHistory;
use crate::launcher::Launcher;
use crate::notification::NotificationManager;
use crate::screen_time::ScreenTimeTracker;
use crate::seamless::SeamlessManager;
use crate::shortcuts::{ShellAction, ShortcutManager};
use crate::stats::StatsCollector;
use crate::theme::ShellTheme;
use crate::tiling::TilingEngine;
use crate::window::WindowId;
use crate::workspace::WorkspaceManager;
use liquide_dock::Dock;
use liquide_renderer_css::StyleResolver;
use liquide_statusbar::ShellStatusBar;

use super::{DragState, HookManager, Shell};

impl Shell {
    /// Get the screen rect.
    #[must_use]
    pub fn screen_rect(&self) -> Rect {
        self.screen_rect
    }

    /// Compute the work area (screen minus statusbar and dock).
    ///
    /// The work area is the usable rectangle where windows can be maximized
    /// without overlapping the statusbar or dock.
    #[must_use]
    pub fn work_area(&self) -> Rect {
        let bar_h = self.status_bar.config().height;
        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
        let dock_h = dock_bounds.height;
        Rect::new(
            self.screen_rect.x,
            self.screen_rect.y + bar_h,
            self.screen_rect.width,
            (self.screen_rect.height - bar_h - dock_h).max(0.0),
        )
    }

    /// Get the current cursor shape.
    #[must_use]
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Whether the user is currently dragging a window (move or resize).
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.drag_state.is_some()
    }

    /// Get the ID of the window currently being dragged, if any.
    #[must_use]
    pub fn dragged_window(&self) -> Option<WindowId> {
        match self.drag_state {
            Some(DragState::Moving { window_id, .. }) => Some(window_id),
            Some(DragState::Resizing { window_id, .. }) => Some(window_id),
            None => None,
        }
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

    /// Get the dock.
    #[must_use]
    pub fn dock(&self) -> &Dock {
        &self.dock
    }

    /// Get the dock mutably.
    pub fn dock_mut(&mut self) -> &mut Dock {
        &mut self.dock
    }

    /// Poll Win32 windows and update the dock with running desktop apps.
    #[cfg(windows)]
    pub fn poll_win32_apps(&mut self) {
        self.win32_dock.poll(&mut self.dock);
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

    /// Get the CSS style resolver, if available.
    #[must_use]
    pub fn style_resolver(&self) -> Option<&StyleResolver> {
        self.style_resolver.as_ref()
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

    /// Set the layout policy.
    pub fn set_layout(&mut self, layout: Box<dyn crate::layout::LayoutPolicy>) {
        self.layout = layout;
    }

    /// Get the hook manager.
    #[must_use]
    pub fn hook_manager(&self) -> &HookManager {
        &self.hook_manager
    }

    /// Get the hook manager mutably.
    pub fn hook_manager_mut(&mut self) -> &mut HookManager {
        &mut self.hook_manager
    }
}
