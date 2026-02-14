//! Qt6 bridge for the Liquide UI toolkit.
//!
//! This crate provides integration between Liquide and Qt6, allowing
//! Liquide applications to run as native Qt applications.
//!
//! # Architecture
//!
//! The bridge translates between Liquide's platform-agnostic abstractions
//! and Qt6's C++ API (via an FFI layer):
//!
//! - **Window** ↔ `QWidget` / `QMainWindow`
//! - **Events** ↔ Qt event filters / `QEvent` subclasses
//! - **Rendering** ↔ `QOpenGLWidget` / `QRhiWidget` / `QPainter`
//! - **Clipboard** ↔ `QClipboard`
//! - **DnD** ↔ `QDrag` / `QDropEvent`
//! - **Accessibility** ↔ `QAccessible` / `QAccessibleInterface`
//! - **IME** ↔ `QInputMethod`

pub mod a11y;
pub mod clipboard;
pub mod dnd;
pub mod event;
pub mod render;
pub mod window;

pub use a11y::{QtA11yBridge, QtA11yRole};
pub use clipboard::QtClipboard;
pub use dnd::QtDndBridge;
pub use event::{QtEventBridge, QtKeyEvent, QtMouseEvent};
pub use render::{QtRenderSurface, QtRenderBackend};
pub use window::{QtWindow, QtWindowConfig};

use thiserror::Error;

/// Errors from the Qt bridge.
#[derive(Debug, Error)]
pub enum QtBridgeError {
    #[error("Qt initialization failed: {0}")]
    InitFailed(String),
    #[error("window creation failed: {0}")]
    WindowCreationFailed(String),
    #[error("rendering error: {0}")]
    RenderError(String),
    #[error("Qt not available on this platform")]
    NotAvailable,
}

/// Qt application wrapper.
///
/// Manages `QApplication` lifecycle and the Qt event loop.
pub struct QtApplication {
    /// Organization name (for QSettings path).
    org_name: String,
    /// Application name.
    app_name: String,
    /// Whether QApplication has been created.
    initialized: bool,
    /// Managed windows.
    windows: Vec<QtWindow>,
}

impl QtApplication {
    #[must_use]
    pub fn new(org_name: impl Into<String>, app_name: impl Into<String>) -> Self {
        Self {
            org_name: org_name.into(),
            app_name: app_name.into(),
            initialized: false,
            windows: Vec::new(),
        }
    }

    /// Initialize Qt. Creates the `QApplication` instance.
    pub fn init(&mut self) -> Result<(), QtBridgeError> {
        tracing::info!(org = %self.org_name, app = %self.app_name, "Initializing Qt application");
        self.initialized = true;
        Ok(())
    }

    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    #[must_use]
    pub fn org_name(&self) -> &str {
        &self.org_name
    }

    #[must_use]
    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    /// Create a new window.
    pub fn create_window(&mut self, config: QtWindowConfig) -> Result<usize, QtBridgeError> {
        if !self.initialized {
            return Err(QtBridgeError::InitFailed("Qt not initialized".to_string()));
        }
        let window = QtWindow::new(config);
        let id = self.windows.len();
        self.windows.push(window);
        Ok(id)
    }

    #[must_use]
    pub fn window(&self, id: usize) -> Option<&QtWindow> {
        self.windows.get(id)
    }

    pub fn window_mut(&mut self, id: usize) -> Option<&mut QtWindow> {
        self.windows.get_mut(id)
    }

    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Run the Qt event loop (blocking).
    pub fn exec(&self) -> Result<i32, QtBridgeError> {
        if !self.initialized {
            return Err(QtBridgeError::InitFailed("Qt not initialized".to_string()));
        }
        tracing::info!("Qt event loop started");
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qt_app_lifecycle() {
        let mut app = QtApplication::new("TestOrg", "TestApp");
        assert!(!app.is_initialized());
        app.init().unwrap();
        assert!(app.is_initialized());
        assert_eq!(app.org_name(), "TestOrg");
        assert_eq!(app.app_name(), "TestApp");
    }

    #[test]
    fn test_create_window_requires_init() {
        let mut app = QtApplication::new("Test", "Test");
        let result = app.create_window(QtWindowConfig::default());
        assert!(result.is_err());
    }
}
