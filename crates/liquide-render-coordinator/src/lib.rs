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
//! │     Coordinator Thread (Message Passing)            │
//! │  ┌───────────────────────────────────────────────┐  │
//! │  │        Render Coordinator (Async)             │  │
//! │  └─────────────────┬─────────────────────────────┘  │
//! │                    │                                 │
//! │         ┌──────────┼──────────┬──────────┬──────┐   │
//! │         │          │          │          │      │   │
//! │    ┌────▼────┐ ┌──▼───┐  ┌──▼───┐  ┌───▼────┐ │   │
//! │    │ Window  │ │ Dock │  │Status│  │ Back-  │ │   │
//! │    │ Threads │ │Thread│  │Thread│  │ ground │ │   │
//! │    │  Pool   │ │      │  │      │  │ Thread │ │   │
//! │    └─────────┘ └──────┘  └──────┘  └────────┘ │   │
//! └─────────────────────────────────────────────────┘   │
//! ```
//!
//! # Example
//!
//! ```rust
//! use liquide_render_coordinator::{ThreadedRenderCoordinator, RenderConfig};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = RenderConfig::builder()
//!     .window_threads(4)
//!     .enable_dock(true)
//!     .enable_statusbar(true)
//!     .enable_wallpaper(true)
//!     .build();
//!
//! // Coordinator runs on its own thread
//! let coordinator = ThreadedRenderCoordinator::new(config)?;
//!
//! // Submit render tasks (non-blocking message passing)
//! coordinator.render_window(window_id, is_focused)?;
//! coordinator.render_dock()?;
//! coordinator.render_statusbar()?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod coordinator;
pub mod error;
pub mod metrics;
pub mod render_task;
pub mod thread_pool;
pub mod threaded;

pub use config::{RenderConfig, RenderConfigBuilder};
pub use coordinator::RenderCoordinator;
pub use error::{RenderError, Result};
pub use render_task::{RenderOutput, RenderPriority, RenderTask, RenderTaskKind};
pub use threaded::ThreadedRenderCoordinator;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::config::{RenderConfig, RenderConfigBuilder};
    pub use crate::coordinator::RenderCoordinator;
    pub use crate::error::{RenderError, Result};
    pub use crate::render_task::{RenderPriority, RenderTask, RenderTaskKind};
    pub use crate::threaded::ThreadedRenderCoordinator;
}
