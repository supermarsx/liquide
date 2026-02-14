//! GTK4 bridge for the Liquide UI toolkit.
//!
//! This crate provides integration between Liquide and GTK4, allowing
//! Liquide applications to run as native GTK applications on Linux and
//! other GTK-supported platforms.
//!
//! # Architecture
//!
//! The bridge translates between Liquide's platform-agnostic abstractions
//! and GTK4's API:
//!
//! - **Window** ↔ `GtkWindow` / `GtkApplicationWindow`
//! - **Events** ↔ GTK event controllers (`GestureClick`, `EventControllerKey`, etc.)
//! - **Rendering** ↔ GTK's `GtkGLArea` or Cairo `Snapshot` API
//! - **Clipboard** ↔ `GdkClipboard`
//! - **DnD** ↔ `GtkDragSource` / `GtkDropTarget`
//! - **Accessibility** ↔ ATK / AT-SPI2
//! - **IME** ↔ `GtkIMContext`

pub mod a11y;
pub mod clipboard;
pub mod dnd;
pub mod event;
pub mod render;
pub mod window;

pub use a11y::{AtkBridge, AtkRole};
pub use clipboard::GtkClipboard;
pub use dnd::GtkDndBridge;
pub use event::{GtkEventBridge, GtkKeyEvent, GtkPointerEvent};
pub use render::{GtkRenderSurface, RenderBackend};
pub use window::{GtkWindow, GtkWindowConfig};

use thiserror::Error;

/// Errors from the GTK bridge.
#[derive(Debug, Error)]
pub enum GtkBridgeError {
    #[error("GTK initialization failed: {0}")]
    InitFailed(String),
    #[error("window creation failed: {0}")]
    WindowCreationFailed(String),
    #[error("rendering error: {0}")]
    RenderError(String),
    #[error("clipboard error: {0}")]
    ClipboardError(String),
    #[error("GTK not available on this platform")]
    NotAvailable,
}

/// GTK application wrapper.
///
/// Manages the GTK main loop and application lifecycle.
pub struct GtkApplication {
    /// Application ID (e.g., "com.example.myapp").
    app_id: String,
    /// Whether the application has been initialized.
    initialized: bool,
    /// Registered windows.
    windows: Vec<GtkWindow>,
}

impl GtkApplication {
    /// Create a new GTK application bridge.
    #[must_use]
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            initialized: false,
            windows: Vec::new(),
        }
    }

    /// Initialize GTK. Must be called before creating windows.
    pub fn init(&mut self) -> Result<(), GtkBridgeError> {
        tracing::info!(app_id = %self.app_id, "Initializing GTK application");
        // In a real implementation, this would call gtk_init() and
        // create the GtkApplication object.
        self.initialized = true;
        Ok(())
    }

    /// Get the application ID.
    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Whether GTK has been initialized.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Create a new window.
    pub fn create_window(&mut self, config: GtkWindowConfig) -> Result<usize, GtkBridgeError> {
        if !self.initialized {
            return Err(GtkBridgeError::InitFailed(
                "GTK not initialized".to_string(),
            ));
        }
        let window = GtkWindow::new(config);
        let id = self.windows.len();
        self.windows.push(window);
        Ok(id)
    }

    /// Get a window by index.
    #[must_use]
    pub fn window(&self, id: usize) -> Option<&GtkWindow> {
        self.windows.get(id)
    }

    /// Get a mutable window by index.
    pub fn window_mut(&mut self, id: usize) -> Option<&mut GtkWindow> {
        self.windows.get_mut(id)
    }

    /// Number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Run the GTK main loop (blocking).
    ///
    /// In a real implementation, this would call `g_application_run()`.
    pub fn run(&self) -> Result<i32, GtkBridgeError> {
        if !self.initialized {
            return Err(GtkBridgeError::InitFailed(
                "GTK not initialized".to_string(),
            ));
        }
        tracing::info!("GTK main loop started");
        // Placeholder — real impl calls gtk main loop.
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_lifecycle() {
        let mut app = GtkApplication::new("com.test.app");
        assert!(!app.is_initialized());
        app.init().unwrap();
        assert!(app.is_initialized());
        assert_eq!(app.app_id(), "com.test.app");
    }

    #[test]
    fn test_create_window_requires_init() {
        let mut app = GtkApplication::new("com.test.app");
        let result = app.create_window(GtkWindowConfig::default());
        assert!(result.is_err());
    }
}
