//! LiquiDE native desktop client library.
//!
//! Provides connection management, display handling, input capture,
//! cursor prediction, frame decoding, audio, clipboard, and the
//! main client runtime coordinator.

pub mod config;
pub mod connection;
pub mod display;
pub mod input;
pub mod cursor;
pub mod decoder;
pub mod overlay;
pub mod machine;
pub mod clipboard;
pub mod audio;
pub mod crash_screen;
pub mod color;
pub mod credential;
pub mod audit;
pub mod runtime;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the client library.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("connection to {server} failed: {reason}")]
    ConnectionFailed { server: String, reason: String },

    #[error("authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("transport negotiation failed: {reason}")]
    TransportNegotiationFailed { reason: String },

    #[error("decoder error ({codec}): {detail}")]
    DecoderError { codec: String, detail: String },

    #[error("display error: {detail}")]
    DisplayError { detail: String },

    #[error("input capture error: {detail}")]
    InputCaptureError { detail: String },

    #[error("clipboard error: {detail}")]
    ClipboardError { detail: String },

    #[error("audio error: {detail}")]
    AudioError { detail: String },

    #[error("profile not found: {name}")]
    ProfileNotFound { name: String },

    #[error("credential storage error: {detail}")]
    CredentialStorageError { detail: String },

    #[error("reconnect failed after {attempts} attempts")]
    ReconnectFailed { attempts: u32 },

    #[error("server unreachable: {server}")]
    ServerUnreachable { server: String },

    #[error("session crashed (code {error_code})")]
    SessionCrashed { error_code: u32 },

    #[error("config error: {detail}")]
    ConfigError { detail: String },

    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for client operations.
pub type Result<T> = std::result::Result<T, ClientError>;

// Re-exports of key types.
pub use config::ClientConfig;
pub use connection::{ConnectionState, ConnectionQuality, ConnectionProfile, ConnectionManager};
pub use display::{DisplayMode, MonitorStrategy, MonitorInfo, DisplayManager, SeamlessWindow};
pub use input::{CaptureScope, ImeMode, InputManager};
pub use cursor::{CursorMode, SmoothingStrategy, CursorPredictor};
pub use decoder::{DecoderBackend, PixelFormat, FrameQueue, DecoderStats};
pub use overlay::{OverlayMetrics, StreamOverlay};
pub use machine::{MachineEntry, MachineGroup, MachineManager};
pub use clipboard::{ClipboardMode, ClipboardSync};
pub use audio::{AudioState, MicrophoneState, AudioManager};
pub use crash_screen::{CrashScreenType, CrashData, CrashScreen};
pub use color::{ColorMode, ToneMapper, ColorPipeline};
pub use credential::{StorageMode, CredentialStore};
pub use audit::{AuditLevel, ClientAuditEvent};
pub use runtime::ClientRuntime;
