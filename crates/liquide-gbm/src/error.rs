use thiserror::Error;

#[derive(Debug, Error)]
pub enum GbmError {
    #[error("GBM device creation failed: {0}")]
    DeviceCreation(String),
    #[error("buffer allocation failed: {width}x{height} format={format}: {reason}")]
    BufferAlloc {
        width: u32,
        height: u32,
        format: String,
        reason: String,
    },
    #[error("DMA-BUF export failed: {0}")]
    DmaBufExport(String),
    #[error("surface creation failed: {0}")]
    SurfaceCreation(String),
    #[error("surface lock failed: {0}")]
    SurfaceLock(String),
    #[error("not supported on this platform")]
    NotSupported,
}

pub type Result<T> = std::result::Result<T, GbmError>;
