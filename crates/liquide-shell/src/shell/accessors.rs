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

    /// Read-only view of the typed-text buffer for a given window — the shell
    /// side of the shell↔app text-input seam (t57-fG feature 2).
    ///
    /// Returns the characters routed into `window`'s buffer by the keyboard
    /// path (`route_char_to_focused_app`), or `None` if nothing has been typed
    /// into that window. Hosts/apps consume this to deliver text into the app's
    /// own model; tests assert it to prove keyboard text reached the window.
    #[must_use]
    pub fn window_text_input(&self, window: WindowId) -> Option<&str> {
        self.focused_app_text.get(&window).map(String::as_str)
    }

    /// Read-only view of the FOCUSED window's typed-text buffer (t57-fG
    /// feature 2). `None` when nothing is focused or no text has been typed
    /// into the focused window yet.
    #[must_use]
    pub fn focused_app_text(&self) -> Option<&str> {
        self.focus
            .focused()
            .and_then(|wid| self.focused_app_text.get(&wid))
            .map(String::as_str)
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
        let screen_rect = Rect::new(0.0, 0.0, width, height);
        if self.screen_rect != screen_rect {
            self.mark_window_scene_dirty();
        }
        self.screen_rect = screen_rect;
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
        self.mark_window_scene_dirty();
        &mut self.focus
    }

    /// Get the workspace manager.
    #[must_use]
    pub fn workspace_manager(&self) -> &WorkspaceManager {
        &self.workspaces
    }

    /// Get the workspace manager mutably.
    pub fn workspace_manager_mut(&mut self) -> &mut WorkspaceManager {
        self.mark_window_scene_dirty();
        &mut self.workspaces
    }

    /// Set the decoration style.
    pub fn set_decoration_style(&mut self, style: DecorationStyle) {
        self.decoration_style = style;
        self.mark_window_scene_dirty();
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
        self.mark_window_scene_dirty();
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
        self.mark_window_scene_dirty();
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

    /// Whether the task/workspace overview overlay is currently shown (t57).
    #[must_use]
    pub fn overview_visible(&self) -> bool {
        self.overview_visible
    }

    /// Whether the session is currently locked (public read of the canonical
    /// lock-screen state driven by the Lock action) (t57-f9).
    #[must_use]
    pub fn session_locked(&self) -> bool {
        self.is_session_locked()
    }

    /// The pending session-lifecycle request (Log Out / Restart / Shut Down)
    /// recorded by the session menu, if any. The shell never terminates the
    /// process itself; a host launcher/compositor consumes this (t57-f9).
    #[must_use]
    pub fn pending_session_request(&self) -> Option<crate::shell::SessionRequest> {
        self.pending_session_request
    }

    /// Clear a consumed session-lifecycle request (called by the host after it
    /// has acted on it).
    pub fn take_session_request(&mut self) -> Option<crate::shell::SessionRequest> {
        self.pending_session_request.take()
    }

    // ──────────────────────────────────────────────────────────────────────
    // Runtime wiring-audit (t57-e7 / A6)
    // ──────────────────────────────────────────────────────────────────────

    /// Record that a canonical manager / chrome adapter ran its LIVE drive path
    /// this session. Pure audit channel — never feeds back into behavior.
    pub(crate) fn mark_wired(&mut self, bit: WiringBit) {
        self.wiring_touched |= bit.mask();
    }

    /// Read the current wiring report (which audited managers have run their
    /// live drive path this session). Read-only; does not run any sync.
    #[must_use]
    pub fn wiring_report(&self) -> WiringReport {
        WiringReport {
            touched: self.wiring_touched,
        }
    }

    /// Run one idempotent live DOM sync (the same `sync_dom` the compositor runs
    /// each frame) so the render-path wiring bits — status bar, dock, launcher,
    /// context menu, tooltip — are recorded, then return the wiring report.
    ///
    /// This lets the wiring-audit observe the render-path bits without touching
    /// the (peer-owned) capture harness.
    pub fn wiring_report_after_sync(&mut self) -> WiringReport {
        self.sync_dom();
        self.wiring_report()
    }
}

/// A canonical manager / chrome adapter tracked by the runtime wiring audit
/// (t57-e7). Each variant flips a bit in `Shell::wiring_touched` the first time
/// its LIVE drive path runs this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WiringBit {
    /// Status bar render path (`sync_statusbar_template`).
    StatusBar,
    /// Dock render path (`sync_dock_template`).
    Dock,
    /// Launcher overlay render path (when visible).
    Launcher,
    /// Context-menu render path (when visible).
    ContextMenu,
    /// Canonical notification daemon (`chrome_notification_server`).
    NotificationServer,
    /// Canonical window-class chrome (`register_window_chrome`).
    WindowClass,
    /// Canonical window-groups chrome (`register_window_chrome`).
    WindowGroups,
    /// Canonical window-tree topology (`register_window_tree`).
    WindowTree,
    /// Canonical window-effects manager (`register_window_tree`).
    WindowEffects,
    /// Canonical lock-screen state machine (`lock_session`).
    LockScreen,
    /// Canonical workspace switch (`commit_workspace_switch`).
    Workspace,
    /// Canonical tiling engine (`canonical_tiling`).
    Tiling,
    /// Canonical tooltip manager (only when it surfaces a tooltip).
    Tooltip,
}

impl WiringBit {
    /// Every audited manager bit (used by the partition test).
    pub const ALL: [WiringBit; 13] = [
        WiringBit::StatusBar,
        WiringBit::Dock,
        WiringBit::Launcher,
        WiringBit::ContextMenu,
        WiringBit::NotificationServer,
        WiringBit::WindowClass,
        WiringBit::WindowGroups,
        WiringBit::WindowTree,
        WiringBit::WindowEffects,
        WiringBit::LockScreen,
        WiringBit::Workspace,
        WiringBit::Tiling,
        WiringBit::Tooltip,
    ];

    /// The single-bit mask for this manager.
    #[must_use]
    pub const fn mask(self) -> u32 {
        1u32 << (self as u32)
    }

    /// Stable lower-snake-case name for diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            WiringBit::StatusBar => "status_bar",
            WiringBit::Dock => "dock",
            WiringBit::Launcher => "launcher",
            WiringBit::ContextMenu => "context_menu",
            WiringBit::NotificationServer => "chrome_notification_server",
            WiringBit::WindowClass => "chrome_window_class",
            WiringBit::WindowGroups => "chrome_window_groups",
            WiringBit::WindowTree => "chrome_window_tree",
            WiringBit::WindowEffects => "chrome_window_effects",
            WiringBit::LockScreen => "chrome_lockscreen",
            WiringBit::Workspace => "workspace",
            WiringBit::Tiling => "chrome_tiling",
            WiringBit::Tooltip => "chrome_tooltip",
        }
    }
}

/// A read-only snapshot of which audited managers ran their live drive path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WiringReport {
    touched: u32,
}

impl WiringReport {
    /// Whether `bit`'s manager ran its live drive path this session.
    #[must_use]
    pub fn is_driven(&self, bit: WiringBit) -> bool {
        self.touched & bit.mask() != 0
    }

    /// All audited managers that ran their live drive path.
    #[must_use]
    pub fn driven(&self) -> Vec<WiringBit> {
        WiringBit::ALL
            .into_iter()
            .filter(|b| self.is_driven(*b))
            .collect()
    }

    /// All audited managers that did NOT run their live drive path.
    #[must_use]
    pub fn not_driven(&self) -> Vec<WiringBit> {
        WiringBit::ALL
            .into_iter()
            .filter(|b| !self.is_driven(*b))
            .collect()
    }
}
