//! Multi-threaded render coordinator for Liquide compositor
//!
//! This crate provides a sophisticated rendering architecture that assigns
//! dedicated threads to different UI components for optimal performance:
//!
//! - **Window threads**: Each window can have its own render thread
//! - **Dock thread**: Dedicated thread for the dock/taskbar
//! - **Status bar thread**: Separate thread for status bar rendering
//! - **Background thread**: Handles desktop background rendering
//! - **Wallpaper thread**: Manages animated/dynamic wallpapers
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │           Render Coordinator (Main)                 │
//! └─────────────────┬───────────────────────────────────┘
//!                   │
//!        ┌──────────┼──────────┬──────────┬──────────┐
//!        │          │          │          │          │
//!   ┌────▼────┐ ┌──▼───┐  ┌──▼───┐  ┌───▼────┐ ┌───▼────┐
//!   │ Window  │ │ Dock │  │Status│  │ Back-  │ │ Wall-  │
//!   │ Threads │ │Thread│  │Thread│  │ ground │ │ paper  │
//!   │  Pool   │ │      │  │      │  │ Thread │ │ Thread │
//!   └─────────┘ └──────┘  └──────┘  └────────┘ └────────┘
//! ```
//!
//! # Example
//!
//! ```rust
//! use liquide_render_coordinator::{RenderCoordinator, RenderConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = RenderConfig::builder()
//!     .window_threads(4)
//!     .enable_dock(true)
//!     .enable_statusbar(true)
//!     .enable_wallpaper(true)
//!     .build();
//!
//! let coordinator = RenderCoordinator::new(config).await?;
//!
//! // Submit window render tasks
//! coordinator.render_window(window_id, render_data).await?;
//!
//! // Update dock
//! coordinator.render_dock(dock_data).await?;
//!
//! // Render status bar
//! coordinator.render_statusbar(statusbar_data).await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod coordinator;
pub mod error;
pub mod thread_pool;
pub mod render_task;
pub mod metrics;

pub use config::{RenderConfig, RenderConfigBuilder};
pub use coordinator::RenderCoordinator;
pub use error::{RenderError, Result};
pub use render_task::{RenderTask, RenderTaskKind, RenderPriority, RenderOutput};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::config::{RenderConfig, RenderConfigBuilder};
    pub use crate::coordinator::RenderCoordinator;
    pub use crate::error::{RenderError, Result};
    pub use crate::render_task::{RenderTask, RenderTaskKind, RenderPriority};
}
