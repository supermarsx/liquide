//! Threaded rendering architecture for Liquide.
//!
//! Separates window chrome (decorations, menus, status bar) rendering
//! from content rendering, so that if the content thread hangs or crashes,
//! the window chrome remains responsive.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │           UI Thread (main)            │
//! │  Event dispatch, layout, state mgmt  │
//! └──────────┬───────────┬───────────────┘
//!            │           │
//!     ┌──────┴────┐ ┌───┴──────────┐
//!     │  Chrome   │ │   Content    │
//!     │  Thread   │ │   Thread     │
//!     │ (window   │ │ (app content │
//!     │  frame)   │ │  rendering)  │
//!     └───────────┘ └──────────────┘
//! ```
//!
//! Communication between threads uses lock-free message passing.

pub mod chrome_thread;
pub mod content_thread;
pub mod coordinator;
pub mod message;

pub use chrome_thread::ChromeThread;
pub use content_thread::ContentThread;
pub use coordinator::RenderCoordinator;
pub use message::{ChromeMessage, ContentMessage, FrameId};

use thiserror::Error;

/// Errors from the render thread system.
#[derive(Debug, Error)]
pub enum RenderThreadError {
    #[error("thread spawn failed: {0}")]
    SpawnFailed(String),
    #[error("channel disconnected")]
    ChannelDisconnected,
    #[error("frame timeout after {0}ms")]
    FrameTimeout(u64),
    #[error("thread panicked: {0}")]
    ThreadPanicked(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let e = RenderThreadError::FrameTimeout(16);
        assert_eq!(format!("{e}"), "frame timeout after 16ms");
    }

    #[test]
    fn test_error_channel_disconnected() {
        let e = RenderThreadError::ChannelDisconnected;
        assert_eq!(format!("{e}"), "channel disconnected");
    }

    #[test]
    fn test_error_spawn_failed() {
        let e = RenderThreadError::SpawnFailed("out of memory".into());
        assert!(format!("{e}").contains("out of memory"));
    }

    #[test]
    fn test_error_thread_panicked() {
        let e = RenderThreadError::ThreadPanicked("assertion failed".into());
        assert!(format!("{e}").contains("assertion failed"));
    }
}
