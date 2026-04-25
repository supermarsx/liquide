//! Window manager — stacking, focus, minimize/maximize state tracking.

use super::window::WindowState;
use liquide_ui_core::WidgetId;
use std::collections::HashMap;

/// Window stacking layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum WindowLayer {
    /// Background / desktop layer.
    Background = 0,
    /// Normal application windows.
    Normal = 100,
    /// Dialogs and transient windows.
    Dialog = 200,
    /// Always-on-top windows.
    AlwaysOnTop = 300,
    /// Notifications and toasts.
    Notification = 400,
    /// System overlays (lock screen, etc.).
    SystemOverlay = 500,
}

/// Tracks managed windows, their stacking order, and focus state.
pub struct WindowManager {
    /// Windows indexed by their widget id.
    windows: HashMap<WidgetId, ManagedWindow>,
    /// Stacking order (back to front).
    stack: Vec<WidgetId>,
    /// Currently focused window.
    focused: Option<WidgetId>,
}

struct ManagedWindow {
    layer: WindowLayer,
    state: WindowState,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            stack: Vec::new(),
            focused: None,
        }
    }

    /// Register a window with the manager.
    pub fn add_window(&mut self, id: WidgetId, layer: WindowLayer) {
        self.windows.insert(
            id,
            ManagedWindow {
                layer,
                state: WindowState::Normal,
            },
        );
        self.stack.push(id);
        self.sort_stack();
    }

    /// Remove a window from the manager.
    pub fn remove_window(&mut self, id: WidgetId) {
        self.windows.remove(&id);
        self.stack.retain(|w| *w != id);
        if self.focused == Some(id) {
            self.focused = self.stack.last().copied();
        }
    }

    /// Bring a window to the front within its layer.
    pub fn raise_window(&mut self, id: WidgetId) {
        if self.windows.contains_key(&id) {
            self.stack.retain(|w| *w != id);
            self.stack.push(id);
            self.sort_stack();
        }
    }

    /// Set the focused window.
    pub fn focus_window(&mut self, id: WidgetId) {
        if self.windows.contains_key(&id) {
            self.focused = Some(id);
            self.raise_window(id);
        }
    }

    /// Get the currently focused window.
    pub fn focused_window(&self) -> Option<WidgetId> {
        self.focused
    }

    /// Get the stacking order (back to front).
    pub fn stacking_order(&self) -> &[WidgetId] {
        &self.stack
    }

    /// Set window state.
    pub fn set_window_state(&mut self, id: WidgetId, state: WindowState) {
        if let Some(mw) = self.windows.get_mut(&id) {
            mw.state = state;
        }
    }

    /// Get window state.
    pub fn window_state(&self, id: WidgetId) -> Option<WindowState> {
        self.windows.get(&id).map(|mw| mw.state)
    }

    /// Number of managed windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    fn sort_stack(&mut self) {
        let windows = &self.windows;
        self.stack.sort_by(|a, b| {
            let la = windows
                .get(a)
                .map(|mw| mw.layer)
                .unwrap_or(WindowLayer::Normal);
            let lb = windows
                .get(b)
                .map(|mw| mw.layer)
                .unwrap_or(WindowLayer::Normal);
            la.cmp(&lb)
        });
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_manager_basic() {
        let mut wm = WindowManager::new();
        let id1 = WidgetId::new();
        let id2 = WidgetId::new();

        wm.add_window(id1, WindowLayer::Normal);
        wm.add_window(id2, WindowLayer::Normal);
        assert_eq!(wm.window_count(), 2);

        wm.focus_window(id1);
        assert_eq!(wm.focused_window(), Some(id1));

        wm.remove_window(id1);
        assert_eq!(wm.window_count(), 1);
        // Focus should fall to remaining window
        assert_eq!(wm.focused_window(), Some(id2));
    }

    #[test]
    fn test_layer_sorting() {
        let mut wm = WindowManager::new();
        let normal_id = WidgetId::new();
        let top_id = WidgetId::new();

        wm.add_window(top_id, WindowLayer::AlwaysOnTop);
        wm.add_window(normal_id, WindowLayer::Normal);

        let stack = wm.stacking_order();
        // Normal should be before AlwaysOnTop
        assert_eq!(stack[0], normal_id);
        assert_eq!(stack[1], top_id);
    }

    #[test]
    fn test_raise_window() {
        let mut wm = WindowManager::new();
        let id1 = WidgetId::new();
        let id2 = WidgetId::new();
        let id3 = WidgetId::new();

        wm.add_window(id1, WindowLayer::Normal);
        wm.add_window(id2, WindowLayer::Normal);
        wm.add_window(id3, WindowLayer::Normal);

        wm.raise_window(id1);
        let stack = wm.stacking_order();
        assert_eq!(*stack.last().unwrap(), id1);
    }

    #[test]
    fn test_set_and_get_window_state() {
        let mut wm = WindowManager::new();
        let id = WidgetId::new();
        wm.add_window(id, WindowLayer::Normal);

        assert_eq!(wm.window_state(id), Some(WindowState::Normal));
        wm.set_window_state(id, WindowState::Maximized);
        assert_eq!(wm.window_state(id), Some(WindowState::Maximized));
    }

    #[test]
    fn test_window_state_nonexistent() {
        let wm = WindowManager::new();
        let id = WidgetId::new();
        assert_eq!(wm.window_state(id), None);
    }

    #[test]
    fn test_focus_raises_window() {
        let mut wm = WindowManager::new();
        let id1 = WidgetId::new();
        let id2 = WidgetId::new();

        wm.add_window(id1, WindowLayer::Normal);
        wm.add_window(id2, WindowLayer::Normal);

        wm.focus_window(id1);
        assert_eq!(wm.focused_window(), Some(id1));
        assert_eq!(*wm.stacking_order().last().unwrap(), id1);
    }

    #[test]
    fn test_remove_focused_falls_back() {
        let mut wm = WindowManager::new();
        let id1 = WidgetId::new();
        let id2 = WidgetId::new();

        wm.add_window(id1, WindowLayer::Normal);
        wm.add_window(id2, WindowLayer::Normal);
        wm.focus_window(id2);
        wm.remove_window(id2);
        // Focus falls to the last window in the stack
        assert_eq!(wm.focused_window(), Some(id1));
    }

    #[test]
    fn test_remove_all_windows() {
        let mut wm = WindowManager::new();
        let id = WidgetId::new();
        wm.add_window(id, WindowLayer::Normal);
        wm.focus_window(id);
        wm.remove_window(id);
        assert_eq!(wm.window_count(), 0);
        assert_eq!(wm.focused_window(), None);
    }

    #[test]
    fn test_layer_ordering_mixed() {
        let mut wm = WindowManager::new();
        let bg = WidgetId::new();
        let normal = WidgetId::new();
        let dialog = WidgetId::new();
        let overlay = WidgetId::new();

        wm.add_window(overlay, WindowLayer::SystemOverlay);
        wm.add_window(normal, WindowLayer::Normal);
        wm.add_window(bg, WindowLayer::Background);
        wm.add_window(dialog, WindowLayer::Dialog);

        let stack = wm.stacking_order();
        assert_eq!(stack[0], bg);
        assert_eq!(stack[1], normal);
        assert_eq!(stack[2], dialog);
        assert_eq!(stack[3], overlay);
    }
}
