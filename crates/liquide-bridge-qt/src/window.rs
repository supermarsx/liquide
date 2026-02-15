//! Qt window management.
//!
//! Maps Liquide windows to `QWidget` / `QMainWindow`.

use serde::{Deserialize, Serialize};

/// Window configuration for Qt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QtWindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub resizable: bool,
    pub frameless: bool,
    pub always_on_top: bool,
    pub opacity: f64,
    /// Qt window flags (Qt::WindowFlags).
    pub window_type: QtWindowType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtWindowType {
    /// Normal top-level window.
    Widget,
    /// Main application window with menu bar, toolbars, status bar.
    MainWindow,
    /// Modal or modeless dialog.
    Dialog,
    /// Popup window (tooltip, dropdown, etc.).
    Popup,
    /// Tool window (floating palette).
    Tool,
}

impl Default for QtWindowConfig {
    fn default() -> Self {
        Self {
            title: "Liquide".to_string(),
            width: 800,
            height: 600,
            x: None,
            y: None,
            resizable: true,
            frameless: false,
            always_on_top: false,
            opacity: 1.0,
            window_type: QtWindowType::Widget,
        }
    }
}

/// Window state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtWindowState {
    Normal,
    Minimized,
    Maximized,
    FullScreen,
}

/// Represents a QWidget-based window.
#[derive(Debug)]
pub struct QtWindow {
    config: QtWindowConfig,
    state: QtWindowState,
    visible: bool,
    /// Unique window ID (maps to `QWidget::winId()`).
    #[allow(dead_code)]
    win_id: u64,
    /// Actual geometry (may differ from config after WM adjustments).
    actual_x: i32,
    actual_y: i32,
    actual_width: u32,
    actual_height: u32,
}

impl QtWindow {
    #[must_use]
    pub fn new(config: QtWindowConfig) -> Self {
        let w = config.width;
        let h = config.height;
        Self {
            config,
            state: QtWindowState::Normal,
            visible: false,
            win_id: 0,
            actual_x: 0,
            actual_y: 0,
            actual_width: w,
            actual_height: h,
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.config.title = title.into();
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.config.title
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.actual_width = width;
        self.actual_height = height;
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.actual_width, self.actual_height)
    }

    pub fn move_to(&mut self, x: i32, y: i32) {
        self.actual_x = x;
        self.actual_y = y;
    }

    #[must_use]
    pub fn position(&self) -> (i32, i32) {
        (self.actual_x, self.actual_y)
    }

    pub fn set_state(&mut self, state: QtWindowState) {
        self.state = state;
    }

    #[must_use]
    pub fn state(&self) -> QtWindowState {
        self.state
    }

    pub fn close(&mut self) {
        self.visible = false;
        tracing::debug!(title = %self.config.title, "Qt window closed");
    }

    #[must_use]
    pub fn config(&self) -> &QtWindowConfig {
        &self.config
    }

    #[must_use]
    pub fn window_type(&self) -> QtWindowType {
        self.config.window_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qt_window() {
        let mut win = QtWindow::new(QtWindowConfig {
            title: "Test".into(),
            width: 1024,
            height: 768,
            ..Default::default()
        });
        assert_eq!(win.title(), "Test");
        assert_eq!(win.size(), (1024, 768));
        win.show();
        assert!(win.is_visible());
        win.move_to(100, 200);
        assert_eq!(win.position(), (100, 200));
    }
}
