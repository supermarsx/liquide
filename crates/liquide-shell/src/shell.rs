//! Top-level shell — orchestrates windows, workspaces, focus, layout.

use std::collections::HashMap;

use liquide_compositor::geometry::Rect;

use crate::decoration::DecorationStyle;
use crate::focus::{FocusManager, FocusPolicy};
use crate::layout::{FloatingLayout, LayoutPolicy};
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
}

impl Shell {
    /// Create a new shell for the given screen dimensions.
    #[must_use]
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            windows: HashMap::new(),
            workspaces: WorkspaceManager::new(),
            focus: FocusManager::new(FocusPolicy::ClickToFocus),
            layout: Box::new(FloatingLayout),
            decoration_style: DecorationStyle::default(),
            next_window_id: 1,
            screen_rect: Rect::new(0.0, 0.0, screen_width, screen_height),
        }
    }

    /// Open a new window. Returns its ID.
    pub fn open_window(&mut self, title: impl Into<String>, bounds: Rect) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let window = Window::new(id, title, bounds);
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);
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
        let win = self.window_mut(id)?;
        win.bounds.x = x;
        win.bounds.y = y;
        Ok(())
    }

    /// Resize a window.
    pub fn resize_window(&mut self, id: WindowId, width: f32, height: f32) -> Result<()> {
        let win = self.window_mut(id)?;
        win.bounds.width = width;
        win.bounds.height = height;
        Ok(())
    }

    /// Minimize a window.
    pub fn minimize(&mut self, id: WindowId) -> Result<()> {
        let win = self.window_mut(id)?;
        win.save_bounds();
        win.state = WindowState::Minimized;
        win.visible = false;
        Ok(())
    }

    /// Maximize a window to fill the screen.
    pub fn maximize(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self.window_mut(id)?;
        win.save_bounds();
        win.state = WindowState::Maximized;
        win.bounds = screen;
        Ok(())
    }

    /// Restore a window from minimized/maximized/fullscreen.
    pub fn restore(&mut self, id: WindowId) -> Result<()> {
        let win = self.window_mut(id)?;
        win.restore_bounds();
        win.state = WindowState::Normal;
        win.visible = true;
        Ok(())
    }

    /// Toggle fullscreen.
    pub fn toggle_fullscreen(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self.window_mut(id)?;
        if win.state == WindowState::Fullscreen {
            win.restore_bounds();
            win.state = WindowState::Normal;
        } else {
            win.save_bounds();
            win.state = WindowState::Fullscreen;
            win.bounds = screen;
        }
        Ok(())
    }

    /// Set focus to a window.
    pub fn set_focus(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        self.focus.set_focus(id);
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
        let win = self.window_mut(id)?;
        win.z_order = max_z + 1;
        Ok(())
    }

    /// Lower a window to the bottom (lowest z_order).
    pub fn lower_window(&mut self, id: WindowId) -> Result<()> {
        let min_z = self.windows.values().map(|w| w.z_order).min().unwrap_or(0);
        let win = self.window_mut(id)?;
        win.z_order = min_z - 1;
        Ok(())
    }

    /// Set the decoration style.
    pub fn set_decoration_style(&mut self, style: DecorationStyle) {
        self.decoration_style = style;
    }
}
