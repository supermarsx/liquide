//! `tick()` method, status bar updates, notification ticking, auto-hide logic,
//! and `execute_action()`.

use liquide_compositor::geometry::Rect;

use crate::history::WindowEventKind;
use crate::shortcuts::ShellAction;
use crate::window::{WindowFlags, WindowId, WindowState};

use super::Shell;
use super::batch::WindowBatch;
use super::hooks::ShellHookEvent;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShellTickResult {
    pub dirty: bool,
    pub status_bar_dirty: bool,
    pub notifications_dirty: bool,
    pub windows_dirty: bool,
    pub auto_hide_dirty: bool,
}

impl Shell {
    /// Periodic tick — update clock, expire notifications.
    ///
    /// Returns `true` if something visually changed (notification expired,
    /// status bar updated, etc.) and a redraw is needed.
    pub fn tick(&mut self, now_us: u64) -> bool {
        self.tick_detailed(now_us).dirty
    }

    /// Periodic tick with a damage-friendly breakdown of what changed.
    pub fn tick_detailed(&mut self, now_us: u64) -> ShellTickResult {
        self.status_bar.update_clock(now_us);
        self.status_bar
            .update_notification_count(self.notifications.unread_count() as u32);
        // t52-e1 single-sourcing: advance the canonical notification daemon (the
        // single source of the active/history data the center renders) so its
        // expiry/dispatch progresses each frame (t49-e5-F03 — events are no
        // longer silently dropped on tick). The daemon is ticked once here; the
        // renderable mirror is ticked once below, so neither is double-advanced.
        if let Some(server) = self.chrome_notification_server.as_mut() {
            server.tick(now_us / 1000);
        }
        let expired = self.notifications.tick(now_us);
        let bar_dirty = self.status_bar.is_dirty();
        if bar_dirty {
            self.status_bar.mark_clean();
        }

        // Window repatriation: ensure windows stay within screen bounds
        let mut repatriation_dirty = false;
        if self.config.window_management.auto_repatriate {
            repatriation_dirty = self.repatriate_offscreen_windows();
            if repatriation_dirty {
                self.mark_window_scene_dirty();
            }
        }

        // Status bar auto-hide based on cursor position and maximized windows
        let auto_hide_dirty = self.update_status_bar_visibility();

        // Tooltip: the canonical `liquide-tooltip` TooltipManager (t51-e9) is
        // driven from the shell's hover state. t51-e15 moved the single
        // per-frame *drive* into the render path (`sync_tooltip_template`,
        // dom_sync.rs), which advances the show-delay / fade lifecycle in the
        // F07-safe order regardless of tick↔render ordering across the
        // render-thread boundary. Tick only *reads* the manager's current
        // visibility for its redraw hint (driving here too would double-advance
        // the dwell). The render path is the retirement replacement for the old
        // hand-rolled `tooltip_timer_us` 400 ms dwell.
        let tooltip_visible = self.tooltip_manager_visible();

        ShellTickResult {
            dirty: bar_dirty
                || !expired.is_empty()
                || repatriation_dirty
                || auto_hide_dirty
                || tooltip_visible,
            status_bar_dirty: bar_dirty,
            notifications_dirty: !expired.is_empty(),
            windows_dirty: repatriation_dirty,
            auto_hide_dirty,
        }
    }

    /// Check if any windows are off-screen and repatriate them within bounds.
    /// Returns true if any window was repositioned.
    fn repatriate_offscreen_windows(&mut self) -> bool {
        let threshold = self.config.window_management.repatriation_threshold_px;
        let screen = self.screen_rect;
        let mut dirty = false;

        // Collect window IDs and old bounds that need repatriation
        let mut updates: Vec<(WindowId, Rect, Rect)> = Vec::new();

        for window in self.windows.values() {
            let bounds = window.bounds;

            // Calculate visible portions on each edge
            let visible_left = bounds.x + bounds.width;
            let visible_right = screen.width - bounds.x;
            let visible_top = bounds.y + bounds.height;
            let visible_bottom = screen.height - bounds.y;

            // Check if window needs repositioning
            let needs_repatriate = visible_left < threshold
                || visible_right < threshold
                || visible_top < threshold
                || visible_bottom < threshold;

            if needs_repatriate {
                // Reposition to keep at least threshold pixels visible
                let mut new_x = bounds.x;
                let mut new_y = bounds.y;

                // Too far left
                if visible_left < threshold {
                    new_x = threshold - bounds.width;
                }
                // Too far right
                if visible_right < threshold {
                    new_x = screen.width - threshold;
                }
                // Too far up
                if visible_top < threshold {
                    new_y = threshold - bounds.height;
                }
                // Too far down
                if visible_bottom < threshold {
                    new_y = screen.height - threshold;
                }

                let new_bounds = Rect::new(
                    new_x.max(0.0).min(screen.width - 50.0),
                    new_y.max(0.0).min(screen.height - 50.0),
                    bounds.width,
                    bounds.height,
                );

                updates.push((window.id, bounds, new_bounds));
            }
        }

        // Apply updates
        for (window_id, old_bounds, new_bounds) in updates {
            if let Some(window) = self.windows.get_mut(&window_id) {
                window.bounds = new_bounds;

                let ts = self.next_timestamp();
                self.window_history.record_at(
                    window_id,
                    WindowEventKind::Moved {
                        from: old_bounds,
                        to: new_bounds,
                    },
                    ts,
                );

                dirty = true;
            }
        }

        dirty
    }

    /// Update status bar visibility based on maximized windows and cursor position.
    /// Returns true if visibility changed.
    fn update_status_bar_visibility(&mut self) -> bool {
        if !self.config.status_bar.auto_hide_on_maximize {
            // Feature disabled, always show
            if !self.status_bar_visible {
                self.status_bar_visible = true;
                return true;
            }
            return false;
        }

        // Check if any window is maximized
        let has_maximized = self
            .windows
            .values()
            .any(|w| w.state == WindowState::Maximized && w.visible);

        if !has_maximized {
            // No maximized windows, always show bar
            if !self.status_bar_visible {
                self.status_bar_visible = true;
                return true;
            }
            return false;
        }

        // Maximized window present: reveal bar when cursor is within
        // the configured edge-hover distance from the top of the screen.
        let reveal_distance = self.status_bar.config().auto_hide_reveal_distance.max(0.0);
        let at_top_edge = self.last_cursor_y <= self.screen_rect.y + reveal_distance;
        if at_top_edge && !self.status_bar_visible {
            self.status_bar_visible = true;
            true
        } else if !at_top_edge && self.status_bar_visible {
            self.status_bar_visible = false;
            true
        } else {
            false
        }
    }

    /// Execute a shell action, returns true if a redraw is needed.
    pub fn execute_action(&mut self, action: &ShellAction) -> bool {
        match action {
            ShellAction::OpenLauncher => {
                let was_visible = self.launcher.is_visible();
                self.launcher.toggle();
                if was_visible {
                    self.hook_manager.dispatch(&ShellHookEvent::LauncherClosed);
                } else {
                    self.hook_manager.dispatch(&ShellHookEvent::LauncherOpened);
                }
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
            ShellAction::ToggleAlwaysOnTop => {
                if let Some(wid) = self.focus.focused() {
                    if let Some(window) = self.windows.get_mut(&wid) {
                        window.flags.toggle(WindowFlags::ALWAYS_ON_TOP);
                        self.mark_window_scene_dirty();
                    }
                }
                true
            }
            ShellAction::SwitchWindowForward => {
                let previous = self.focus.focused();
                self.focus.focus_next();
                if self.focus.focused() != previous {
                    self.mark_window_scene_dirty();
                }
                true
            }
            ShellAction::SwitchWindowBackward => {
                let previous = self.focus.focused();
                self.focus.focus_prev();
                if self.focus.focused() != previous {
                    self.mark_window_scene_dirty();
                }
                true
            }
            ShellAction::TileLeft => {
                if let Some(wid) = self.focus.focused() {
                    let work = self.work_area();
                    let half_w = work.width / 2.0;
                    let mut batch = WindowBatch::new();
                    batch.move_resize(wid, work.x, work.y, half_w, work.height);
                    self.apply_batch(batch);
                }
                true
            }
            ShellAction::TileRight => {
                if let Some(wid) = self.focus.focused() {
                    let work = self.work_area();
                    let half_w = work.width / 2.0;
                    let mut batch = WindowBatch::new();
                    batch.move_resize(wid, work.x + half_w, work.y, half_w, work.height);
                    self.apply_batch(batch);
                }
                true
            }
            ShellAction::WorkspaceNext => {
                // Drive through the canonical `liquide-workspaces` manager
                // (t51-e12). This is a REAL switch (fixes t49-e5-F01): the new
                // workspace's windows become visible/interactive and the old
                // ones are hidden via a batched `workspace_switch`.
                let from = self.workspaces.active().id.0;
                if self.switch_workspace_next() {
                    let to = self.workspaces.active().id.0;
                    self.mark_window_scene_dirty();
                    self.hook_manager
                        .dispatch(&ShellHookEvent::WorkspaceChanged { from, to });
                }
                true
            }
            ShellAction::WorkspacePrev => {
                let from = self.workspaces.active().id.0;
                if self.switch_workspace_prev() {
                    let to = self.workspaces.active().id.0;
                    self.mark_window_scene_dirty();
                    self.hook_manager
                        .dispatch(&ShellHookEvent::WorkspaceChanged { from, to });
                }
                true
            }
            ShellAction::SwitchToWorkspace(n) => {
                let from = self.workspaces.active().id.0;
                if self.switch_workspace_to_index(*n as usize) {
                    let to = self.workspaces.active().id.0;
                    self.mark_window_scene_dirty();
                    self.hook_manager
                        .dispatch(&ShellHookEvent::WorkspaceChanged { from, to });
                }
                true
            }
            ShellAction::MoveWindowToWorkspace(n) => {
                // Repatriate the focused window to another workspace (fixes the
                // F04 gap where this action had no arm). Routed through the
                // canonical manager; the window stops rendering on the active
                // workspace once moved away.
                if let Some(wid) = self.focus.focused() {
                    self.move_window_to_workspace_index(wid, *n as usize);
                }
                true
            }
            ShellAction::MoveWindowToNextWorkspace => {
                if let Some(wid) = self.focus.focused() {
                    let count = self.workspaces.workspace_count();
                    let active = self.workspaces.active().id.0 as usize;
                    if active + 1 < count {
                        self.move_window_to_workspace_index(wid, active + 1);
                    }
                }
                true
            }
            ShellAction::MoveWindowToPrevWorkspace => {
                if let Some(wid) = self.focus.focused() {
                    let active = self.workspaces.active().id.0 as usize;
                    if active > 0 {
                        self.move_window_to_workspace_index(wid, active - 1);
                    }
                }
                true
            }
            ShellAction::ShowDesktop => {
                let ids: Vec<_> = self.visible_windows().iter().map(|w| w.id).collect();
                let mut batch = WindowBatch::with_capacity(ids.len());
                for wid in ids {
                    batch.minimize(wid);
                }
                self.apply_batch(batch);
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
                // Drive the canonical lockscreen (t51-e12 folding in t51-e10's
                // `Shell::lock_session()`): the session-menu Lock action now
                // transitions the canonical `liquide-lockscreen` state to locked
                // through the real `AuthBackend`, instead of the prior no-op
                // (fixes t49-e5-F02). The emitted lockscreen events
                // (RequestBackgroundCapture / ClearOverview) are produced by the
                // canonical state machine; we ignore them here (no compositor
                // sink in the simulated shell) but the lock state is now real.
                let _events = self.lock_session();
                true
            }
            ShellAction::LogOut => {
                // Record the request only — the shell never terminates the
                // session itself (t57-f9). The host launcher/compositor reads
                // `pending_session_request` and performs the real teardown. The
                // session menu is closed so the gesture has a visible effect.
                self.pending_session_request = Some(crate::shell::SessionRequest::LogOut);
                self.session_menu_visible = false;
                true
            }
            ShellAction::Restart => {
                self.pending_session_request = Some(crate::shell::SessionRequest::Restart);
                self.session_menu_visible = false;
                true
            }
            ShellAction::Shutdown => {
                self.pending_session_request = Some(crate::shell::SessionRequest::Shutdown);
                self.session_menu_visible = false;
                true
            }
            ShellAction::Redraw => {
                // No-op — just triggers a redraw.
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
            ShellAction::OpenNotificationCenter => {
                // Toggle the notification center panel (t57-f4). This action is
                // returned from the status-bar notification-indicator click
                // (events.rs:1019/1026) and the keyboard shortcut, but used to
                // fall through to `_ => false` so neither user gesture toggled
                // the panel — only the test-only `toggle_notification_center`
                // public helper did. Wiring the arm makes the user action live.
                self.toggle_notification_center();
                true
            }
            ShellAction::TaskOverview | ShellAction::WorkspaceOverview => {
                // Toggle the overview overlay (t57-f-overview). Both the task
                // overview (Super+Tab) and workspace overview shortcuts used to
                // fall through to `_ => false`, so the overview never appeared.
                // The arm flips the shell's `overview_visible` state; the scene
                // builder emits the overview overlay (tiles of visible windows)
                // when it is set.
                self.overview_visible = !self.overview_visible;
                self.mark_window_scene_dirty();
                true
            }
            _ => false,
        }
    }
}
