//! `tick()` method, status bar updates, notification ticking, auto-hide logic,
//! and `execute_action()`.

use liquide_compositor::geometry::Rect;

use crate::history::WindowEventKind;
use crate::shortcuts::ShellAction;
use crate::window::{WindowFlags, WindowId, WindowState};

use super::hooks::ShellHookEvent;
use super::Shell;
use super::batch::WindowBatch;

impl Shell {
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

        // Window repatriation: ensure windows stay within screen bounds
        let mut repatriation_dirty = false;
        if self.config.window_management.auto_repatriate {
            repatriation_dirty = self.repatriate_offscreen_windows();
        }

        // Status bar auto-hide based on cursor position and maximized windows
        let auto_hide_dirty = self.update_status_bar_visibility();

        bar_dirty || !expired.is_empty() || repatriation_dirty || auto_hide_dirty
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

        // Maximized window present: always hide for now (will show on mouse hover in future)
        // TODO: Track mouse position to reveal on top-edge hover
        if self.status_bar_visible {
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
                    }
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
                let active = self.workspaces.active().id;
                let count = self.workspaces.workspace_count();
                if (active.0 as usize) < count - 1 {
                    let from = active.0;
                    let next = crate::workspace::WorkspaceId(active.0 + 1);
                    let _ = self.workspaces.switch_to(next);
                    self.hook_manager.dispatch(&ShellHookEvent::WorkspaceChanged {
                        from,
                        to: next.0,
                    });
                }
                true
            }
            ShellAction::WorkspacePrev => {
                let active = self.workspaces.active().id;
                if active.0 > 0 {
                    let from = active.0;
                    let prev = crate::workspace::WorkspaceId(active.0 - 1);
                    let _ = self.workspaces.switch_to(prev);
                    self.hook_manager.dispatch(&ShellHookEvent::WorkspaceChanged {
                        from,
                        to: prev.0,
                    });
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
                // Visual feedback only — no real lock in a simulated shell.
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
            _ => false,
        }
    }
}
