//! GTK window management.
//!
//! Maps Liquide window concepts to GtkWindow / GtkApplicationWindow.

use serde::{Deserialize, Serialize};

/// Window configuration for GTK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtkWindowConfig {
    /// Window title.
    pub title: String,
    /// Initial width.
    pub width: u32,
    /// Initial height.
    pub height: u32,
    /// Whether the window is resizable.
    pub resizable: bool,
    /// Whether to show decorations (CSD vs SSD).
    pub decorated: bool,
    /// Whether to use client-side decorations (GTK4 default).
    pub client_side_decorations: bool,
    /// Opacity (0.0 – 1.0).
    pub opacity: f64,
    /// Whether the window should be modal.
    pub modal: bool,
}

impl Default for GtkWindowConfig {
    fn default() -> Self {
        Self {
            title: "Liquide".to_string(),
            width: 800,
            height: 600,
            resizable: true,
            decorated: true,
            client_side_decorations: true,
            opacity: 1.0,
            modal: false,
        }
    }
}

/// Window state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

/// Represents a GtkWindow / GtkApplicationWindow.
#[derive(Debug)]
pub struct GtkWindow {
    config: GtkWindowConfig,
    state: WindowState,
    visible: bool,
    /// Opaque handle to the native GtkWindow (would be a pointer in real impl).
    native_handle: u64,
}

impl GtkWindow {
    #[must_use]
    pub fn new(config: GtkWindowConfig) -> Self {
        Self {
            config,
            state: WindowState::Normal,
            visible: false,
            native_handle: 0,
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
        tracing::debug!(title = %self.config.title, "Showing GTK window");
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
        self.config.width = width;
        self.config.height = height;
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn set_state(&mut self, state: WindowState) {
        self.state = state;
    }

    #[must_use]
    pub fn state(&self) -> WindowState {
        self.state
    }

    pub fn maximize(&mut self) {
        self.state = WindowState::Maximized;
    }

    pub fn minimize(&mut self) {
        self.state = WindowState::Minimized;
    }

    pub fn restore(&mut self) {
        self.state = WindowState::Normal;
    }

    pub fn fullscreen(&mut self) {
        self.state = WindowState::Fullscreen;
    }

    pub fn close(&mut self) {
        self.visible = false;
        tracing::debug!(title = %self.config.title, "Closing GTK window");
    }

    #[must_use]
    pub fn config(&self) -> &GtkWindowConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window() {
        let mut win = GtkWindow::new(GtkWindowConfig {
            title: "Test".into(),
            width: 1024,
            height: 768,
            ..Default::default()
        });
        assert_eq!(win.title(), "Test");
        assert_eq!(win.size(), (1024, 768));
        assert!(!win.is_visible());

        win.show();
        assert!(win.is_visible());

        win.maximize();
        assert_eq!(win.state(), WindowState::Maximized);

        win.close();
        assert!(!win.is_visible());
    }
}
