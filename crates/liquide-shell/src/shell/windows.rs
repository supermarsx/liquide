//! Window management — create, close, resize, move, focus, z-order.

use liquide_compositor::geometry::Rect;

use crate::history::WindowEventKind;
use crate::window::{Window, WindowId, WindowState};
use crate::{Result, ShellError};

use super::hooks::ShellHookEvent;
use super::Shell;

impl Shell {
    /// Open a new window. Returns its ID.
    pub fn open_window(&mut self, title: impl Into<String>, bounds: Rect) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let window = Window::new(id, title, bounds);
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::Opened, ts);
        self.hook_manager.dispatch(&ShellHookEvent::WindowCreated { window_id: id.0 });
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
        if !app_id_str.is_empty() {
            self.sandbox_manager.register_app(app_id_str.clone());
        }
        let mut window = Window::new(id, title, bounds);
        window.app_id = app_id_str.clone();
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::Opened, ts);
        if !app_id_str.is_empty() {
            self.app_history.record_open(&app_id_str, id, bounds, ts);
            self.screen_time.feed_open(&app_id_str, id, ts);
        }
        self.hook_manager.dispatch(&ShellHookEvent::WindowCreated { window_id: id.0 });
        id
    }

    /// Close a window. Returns the removed window.
    pub fn close_window(&mut self, id: WindowId) -> Result<Window> {
        let window = self.windows.remove(&id).ok_or(ShellError::WindowNotFound { id })?;
        self.workspaces.active_mut().remove_window(id);
        self.focus.remove_window(id);
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::Closed, ts);
        if !window.app_id.is_empty() {
            self.app_history.record_close(&window.app_id, id, window.bounds, ts);
            self.screen_time.feed_close(&window.app_id, id, ts);
            self.dock.remove_running(&window.app_id);
            let has_other_windows = self.windows.values().any(|w| w.app_id == window.app_id);
            if !has_other_windows {
                self.sandbox_manager.unregister_app(&window.app_id);
            }
        }
        self.hook_manager.dispatch(&ShellHookEvent::WindowClosed { window_id: id.0 });
        Ok(window)
    }

    /// Get a window by ID.
    pub fn window(&self, id: WindowId) -> Result<&Window> {
        self.windows.get(&id).ok_or(ShellError::WindowNotFound { id })
    }

    /// Get a window mutably by ID.
    pub fn window_mut(&mut self, id: WindowId) -> Result<&mut Window> {
        self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })
    }

    /// Move a window to a new position.
    pub fn move_window(&mut self, id: WindowId, x: f32, y: f32) -> Result<()> {
        let win = self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.x = x;
        win.bounds.y = y;
        let to = win.bounds;
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::Moved { from, to }, ts);
        self.hook_manager.dispatch(&ShellHookEvent::WindowMoved {
            window_id: id.0,
            x: x.round() as i32,
            y: y.round() as i32,
        });
        Ok(())
    }

    /// Resize a window.
    pub fn resize_window(&mut self, id: WindowId, width: f32, height: f32) -> Result<()> {
        let win = self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.width = width;
        win.bounds.height = height;
        let to = win.bounds;
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::Resized { from, to }, ts);
        self.hook_manager.dispatch(&ShellHookEvent::WindowResized {
            window_id: id.0,
            width: width.round() as u32,
            height: height.round() as u32,
        });
        Ok(())
    }

    /// Minimize a window.
    pub fn minimize(&mut self, id: WindowId) -> Result<()> {
        let win = self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        win.save_bounds();
        win.state = WindowState::Minimized;
        win.visible = false;
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::StateChanged { from: from_state, to: WindowState::Minimized }, ts);
        if from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(id, WindowEventKind::VisibilityChanged { from: true, to: false }, ts2);
        }
        self.hook_manager.dispatch(&ShellHookEvent::WindowMinimized { window_id: id.0 });
        Ok(())
    }

    /// Maximize a window to fill the work area (screen minus statusbar and dock).
    pub fn maximize(&mut self, id: WindowId) -> Result<()> {
        let work = self.work_area();
        let win = self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_bounds = win.bounds;
        win.save_bounds();
        win.state = WindowState::Maximized;
        win.bounds = work;
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::StateChanged { from: from_state, to: WindowState::Maximized }, ts);
        let ts2 = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::Resized { from: from_bounds, to: work }, ts2);
        self.hook_manager.dispatch(&ShellHookEvent::WindowMaximized { window_id: id.0 });
        Ok(())
    }

    /// Restore a window from minimized/maximized/fullscreen.
    pub fn restore(&mut self, id: WindowId) -> Result<()> {
        let win = self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        let from_bounds = win.bounds;
        win.restore_bounds();
        win.state = WindowState::Normal;
        win.visible = true;
        let to_bounds = win.bounds;
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::StateChanged { from: from_state, to: WindowState::Normal }, ts);
        if !from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(id, WindowEventKind::VisibilityChanged { from: false, to: true }, ts2);
        }
        if from_bounds != to_bounds {
            let ts3 = self.next_timestamp();
            self.window_history.record_at(id, WindowEventKind::Resized { from: from_bounds, to: to_bounds }, ts3);
        }
        self.hook_manager.dispatch(&ShellHookEvent::WindowRestored { window_id: id.0 });
        Ok(())
    }

    /// Toggle fullscreen.
    pub fn toggle_fullscreen(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })?;
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
        self.window_history.record_at(id, WindowEventKind::StateChanged { from: from_state, to: to_state }, ts);
        if from_bounds != to_bounds {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(id, WindowEventKind::Resized { from: from_bounds, to: to_bounds }, ts2);
        }
        Ok(())
    }

    /// Set focus to a window.
    ///
    /// NOTE: This updates the internal focus manager but does NOT sync the
    /// `class="focused"` attribute in the DOM because windows are rendered
    /// manually via `scene.rs` rather than through the CSS pipeline.  The
    /// `DesktopDocument::set_focused_window()` helper exists for future use
    /// when/if windows are migrated to DOM-based rendering.  The scene
    /// builder already reads `self.focus.focused()` directly to determine
    /// the focused-window visual state (title-bar colour, border colour).
    pub fn set_focus(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        let prev_focused = self.focus.focused();
        self.focus.set_focus(id);
        if let Some(prev_id) = prev_focused {
            if prev_id != id {
                let ts = self.next_timestamp();
                self.window_history.record_at(prev_id, WindowEventKind::Unfocused, ts);
                self.screen_time.feed_unfocus(ts);
                self.hook_manager.dispatch(&ShellHookEvent::WindowDeactivated { window_id: prev_id.0 });
            }
        }
        let ts2 = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::Focused, ts2);
        let app_id = self.windows.get(&id).map(|w| w.app_id.clone()).unwrap_or_default();
        if !app_id.is_empty() {
            self.screen_time.feed_focus(&app_id, id, ts2);
        }
        self.hook_manager.dispatch(&ShellHookEvent::WindowActivated { window_id: id.0 });
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

    /// Apply the current layout to visible windows.
    pub fn arrange_windows(&mut self) {
        let screen = self.screen_rect;
        let mut visible_ids: Vec<WindowId> = self.windows.values().filter(|w| w.visible).map(|w| w.id).collect();
        visible_ids.sort_by_key(|id| id.0);
        let mut window_vec: Vec<Window> = visible_ids.iter().filter_map(|id| self.windows.get(id).cloned()).collect();
        self.layout.arrange(&mut window_vec, screen);
        for win in window_vec {
            if let Some(existing) = self.windows.get_mut(&win.id) {
                existing.bounds = win.bounds;
            }
        }
    }

    /// Raise a window to the top (highest z_order).
    pub fn raise_window(&mut self, id: WindowId) -> Result<()> {
        let max_z = self.windows.values().map(|w| w.z_order).max().unwrap_or(0);
        let win = self.windows.get_mut(&id).ok_or(ShellError::WindowNotFound { id })?;
        let from_z = win.z_order;
        win.z_order = max_z + 1;
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::ZOrderChanged { from: from_z, to: max_z + 1 }, ts);
        self.normalize_z_orders();
        Ok(())
    }

    /// Lower a window to the bottom (lowest z_order).
    pub fn lower_window(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        // Temporarily set to -1 so it sorts below everything, then normalize.
        if let Some(win) = self.windows.get_mut(&id) {
            win.z_order = -1;
        }
        self.normalize_z_orders();
        let new_z = self.windows.get(&id).map(|w| w.z_order).unwrap_or(0);
        let ts = self.next_timestamp();
        self.window_history.record_at(id, WindowEventKind::ZOrderChanged { from: new_z, to: new_z }, ts);
        Ok(())
    }

    /// Compact z_order values to sequential non-negative integers,
    /// preserving relative order. Prevents unbounded growth from
    /// repeated raise/lower operations.
    pub(crate) fn normalize_z_orders(&mut self) {
        let mut sorted: Vec<(WindowId, i32)> = self.windows.iter()
            .map(|(id, w)| (*id, w.z_order))
            .collect();
        sorted.sort_by_key(|(_, z)| *z);
        for (i, (id, _)) in sorted.iter().enumerate() {
            if let Some(w) = self.windows.get_mut(id) {
                w.z_order = i as i32;
            }
        }
    }

    /// Open a new window for the given application, or focus an existing one.
    pub fn open_app_window(&mut self, app_id: &str) -> WindowId {
        self.sandbox_manager.register_app(app_id.to_string());
        let can_create_windows = self.sandbox_manager.with_sandbox(app_id, |sandbox| sandbox.can_create_windows()).unwrap_or(false);
        if !can_create_windows {
            tracing::warn!("Sandbox denied window creation for app: {}", app_id);
        }
        if let Some(existing) = self.windows.values().find(|w| w.app_id == app_id && w.visible) {
            let wid = existing.id;
            let _ = self.set_focus(wid);
            let _ = self.raise_window(wid);
            return wid;
        }
        let work = self.work_area();
        let (title, w, h): (&str, f32, f32) = match app_id {
            "com.liquide.settings" => ("Settings", 700.0, 500.0),
            "com.liquide.terminal" => ("Terminal", 720.0, 480.0),
            "com.liquide.files" => ("Files", 800.0, 550.0),
            "com.liquide.browser" => ("Browser", 900.0, 600.0),
            "com.liquide.calculator" => ("Calculator", 360.0, 420.0),
            _ => ("Application", 640.0, 480.0),
        };
        let x = work.x + (work.width - w) / 2.0;
        let y = work.y + (work.height - h) / 2.0;
        let id = self.open_window_with_app(title, Rect::new(x, y, w, h), app_id);
        // Set sensible minimum sizes for known applications.
        let min = match app_id {
            "com.liquide.calculator" => Some((280.0, 320.0)),
            _ => Some((200.0, 150.0)),
        };
        if let Some(win) = self.windows.get_mut(&id) {
            win.min_size = min;
        }
        self.dock.add_running(app_id);
        let _ = self.set_focus(id);
        let _ = self.raise_window(id);
        id
    }
}
