//! Error types for render coordinator

use thiserror::Error;

/// Result type alias for render coordinator operations
pub type Result<T> = std::result::Result<T, RenderError>;

/// Errors that can occur during rendering coordination
#[derive(Debug, Error)]
pub enum RenderError {
    /// Thread pool initialization failed
    #[error("Failed to initialize thread pool: {0}")]
    ThreadPoolInit(String),

    /// Render task failed
    #[error("Render task failed: {0}")]
    RenderTaskFailed(String),

    /// Channel send error
    #[error("Failed to send task to render thread: {0}")]
    ChannelSend(String),

    /// Channel receive error
    #[error("Failed to receive from render thread: {0}")]
    ChannelRecv(String),

    /// Thread join error
    #[error("Failed to join render thread: {0}")]
    ThreadJoin(String),

    /// Timeout waiting for render
    #[error("Render task timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Invalid configuration
    #[error("Invalid render configuration: {0}")]
    InvalidConfig(String),

    /// Thread panic
    #[error("Render thread panicked: {0}")]
    ThreadPanic(String),

    /// Thread creation error
    #[error("Failed to create thread: {0}")]
    ThreadCreation(String),

    /// Resource exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl RenderError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RenderError::Timeout(_)
                | RenderError::ChannelSend(_)
                | RenderError::ResourceExhausted(_)
        )
    }

    /// Check if error indicates thread panic
    pub fn is_panic(&self) -> bool {
        matches!(self, RenderError::ThreadPanic(_))
    }
}
