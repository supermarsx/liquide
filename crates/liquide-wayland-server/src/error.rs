use thiserror::Error;

#[derive(Debug, Error)]
pub enum WaylandServerError {
    #[error("failed to bind socket: {path}: {reason}")]
    SocketBind { path: String, reason: String },
    #[error("client connection error: client={client_id}: {reason}")]
    ClientError { client_id: u32, reason: String },
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("invalid object ID: {0}")]
    InvalidObject(u32),
    #[error("buffer import failed: {0}")]
    BufferImport(String),
    #[error("SHM pool error: {0}")]
    ShmPool(String),
    #[error("surface error: {0}")]
    Surface(String),
    #[error("not supported on this platform")]
    NotSupported,
    #[error("server not running")]
    NotRunning,
    #[error("I/O error: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, WaylandServerError>;
