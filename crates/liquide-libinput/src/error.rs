use thiserror::Error;

#[derive(Debug, Error)]
pub enum LibinputError {
    #[error("device enumeration failed: {0}")]
    Enumeration(String),
    #[error("device open failed: {path}: {reason}")]
    DeviceOpen { path: String, reason: String },
    #[error("ioctl failed on {device}: {name}: {reason}")]
    Ioctl {
        device: String,
        name: String,
        reason: String,
    },
    #[error("hotplug monitoring failed: {0}")]
    Hotplug(String),
    #[error("not supported on this platform")]
    NotSupported,
    #[error("permission denied: {path}: requires input group or root")]
    PermissionDenied { path: String },
}

pub type Result<T> = std::result::Result<T, LibinputError>;
