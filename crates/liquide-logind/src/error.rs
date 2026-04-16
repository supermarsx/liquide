use thiserror::Error;

#[derive(Debug, Error)]
pub enum LogindError {
    #[error("D-Bus connection failed: {0}")]
    DbusConnection(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("take control failed: {0}")]
    TakeControl(String),
    #[error("release control failed: {0}")]
    ReleaseControl(String),
    #[error("VT allocation failed: {0}")]
    VtAllocation(String),
    #[error("VT switch failed: vt={vt}: {reason}")]
    VtSwitch { vt: u32, reason: String },
    #[error("VT ioctl failed: {name}: {reason}")]
    VtIoctl { name: String, reason: String },
    #[error("device access denied: {path}")]
    DeviceAccess { path: String },
    #[error("privilege operation failed: {0}")]
    Privilege(String),
    #[error("not supported on this platform")]
    NotSupported,
    #[error("session already active")]
    AlreadyActive,
    #[error("seatd connection failed: {0}")]
    SeatdConnection(String),
}

pub type Result<T> = std::result::Result<T, LogindError>;
