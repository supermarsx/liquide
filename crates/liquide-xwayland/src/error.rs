use thiserror::Error;

#[derive(Debug, Error)]
pub enum XWaylandError {
    #[error("XWayland binary not found at: {0}")]
    BinaryNotFound(String),
    #[error("XWayland failed to start: {0}")]
    StartFailed(String),
    #[error("XWayland process crashed: exit code {0}")]
    Crashed(i32),
    #[error("X11 display allocation failed: {0}")]
    DisplayAlloc(String),
    #[error("socket pair creation failed: {0}")]
    SocketPair(String),
    #[error("window mapping failed: window={window_id}: {reason}")]
    WindowMapping { window_id: u32, reason: String },
    #[error("atom lookup failed: {0}")]
    AtomLookup(String),
    #[error("clipboard bridge error: {0}")]
    Clipboard(String),
    #[error("not supported on this platform")]
    NotSupported,
}

pub type Result<T> = std::result::Result<T, XWaylandError>;
