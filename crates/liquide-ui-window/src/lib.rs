//! Window frame, title bar, decorations and management for the LiquiDE UI toolkit.
//!
//! This crate provides:
//! - `Window` — the top-level container widget with title bar, frame, and client area
//! - `WindowBuilder` — fluent API for constructing windows
//! - `TitleBar` — macOS/Qt-style title bar with close/minimize/maximize buttons
//! - `WindowFrame` — border decorations, resize handles, glass effect
//! - `WindowManager` — stacking order, focus tracking, minimise/maximize state
//!
//! Inspired by Qt's `QMainWindow` / `QDialog` and GTK's `GtkWindow` / `GtkDialog`.

pub mod builder;
pub mod frame;
pub mod manager;
pub mod title_bar;
pub mod window;

pub use builder::WindowBuilder;
pub use frame::{FrameStyle, WindowFrame};
pub use manager::WindowManager;
pub use title_bar::{TitleBar, TitleBarButton, TitleBarButtonKind};
pub use window::{Window, WindowFlags, WindowKind, WindowState};
