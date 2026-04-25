//! Input Method Editor (IME) support for the LiquiDE desktop environment.
//!
//! Provides a cross-platform abstraction over platform IME APIs:
//! - **IBus** / **fcitx** on Linux  
//! - **Text Services Framework (TSF)** / **IMM32** on Windows
//! - **Input Sources** on macOS
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────────────┐
//! │  Application  ├────►│ ImeContext   ├────►│  Platform Backend    │
//! │  (text field) │◄────┤              │◄────┤  (IBus/TSF/InputSrc) │
//! └──────────────┘     └──────────────┘     └──────────────────────┘
//! ```
//!
//! The application creates an `ImeContext` for each text input widget.
//! Composition events flow through `ImeEvent` callbacks.

pub mod candidate;
pub mod composition;
pub mod context;

use thiserror::Error;

pub use candidate::{CandidateItem, CandidateList, CandidatePageInfo};
pub use composition::{ClauseStyle, CompositionClause, CompositionState, CompositionUpdate};
pub use context::{CursorRect, ImeConfig, ImeContext, ImeEvent};

/// IME errors.
#[derive(Debug, Error)]
pub enum ImeError {
    #[error("IME not available on this platform")]
    NotAvailable,
    #[error("IME context creation failed: {0}")]
    ContextCreationFailed(String),
    #[error("composition error: {0}")]
    CompositionError(String),
    #[error("platform error: {0}")]
    PlatformError(String),
}

pub type Result<T> = std::result::Result<T, ImeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ImeError::NotAvailable;
        assert!(err.to_string().contains("not available"));
    }
}
