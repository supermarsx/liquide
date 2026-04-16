use thiserror::Error;

#[derive(Debug, Error)]
pub enum DrmError {
    #[error("failed to open DRM device: {path}: {reason}")]
    DeviceOpen { path: String, reason: String },
    #[error("no suitable DRM device found")]
    NoDevice,
    #[error("modesetting failed on connector {connector}: {reason}")]
    ModeSetting { connector: u32, reason: String },
    #[error("page flip failed: {0}")]
    PageFlip(String),
    #[error("no connected output found")]
    NoConnectedOutput,
    #[error("atomic commit failed: {0}")]
    AtomicCommit(String),
    #[error("buffer allocation failed: {0}")]
    BufferAlloc(String),
    #[error("DRM ioctl failed: {name}: {reason}")]
    Ioctl { name: String, reason: String },
    #[error("VBLANK wait failed: {0}")]
    VblankWait(String),
    #[error("device lost")]
    DeviceLost,
}

pub type Result<T> = std::result::Result<T, DrmError>;
