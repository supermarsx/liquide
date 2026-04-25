//! LiquiDE native desktop client library.
//!
//! Provides connection management, display handling, input capture,
//! cursor prediction, frame decoding, audio, clipboard, and the
//! main client runtime coordinator.

pub mod audio;
pub mod audit;
pub mod clipboard;
pub mod color;
pub mod config;
pub mod connection;
pub mod crash_screen;
pub mod credential;
pub mod cursor;
pub mod decoder;
pub mod display;
pub mod input;
pub mod machine;
pub mod overlay;
pub mod runtime;

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

    #[error("connection timed out after {timeout_ms}ms")]
    ConnectionTimeout { timeout_ms: u64 },

    #[error("not connected to any server")]
    NotConnected,

    #[error("connection lost: {reason}")]
    ConnectionLost { reason: String },

    #[error("protocol error: {detail}")]
    ProtocolError { detail: String },

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
pub use audio::{AudioManager, AudioState, MicrophoneState};
pub use audit::{AuditLevel, ClientAuditEvent};
pub use clipboard::{ClipboardMode, ClipboardSync};
pub use color::{ColorMode, ColorPipeline, ToneMapper};
pub use config::ClientConfig;
pub use connection::{ConnectionManager, ConnectionProfile, ConnectionQuality, ConnectionState};
pub use crash_screen::{CrashData, CrashScreen, CrashScreenType};
pub use credential::{CredentialStore, StorageMode};
pub use cursor::{CursorMode, CursorPredictor, SmoothingStrategy};
pub use decoder::{DecoderBackend, DecoderStats, FrameQueue, PixelFormat};
pub use display::{DisplayManager, DisplayMode, MonitorInfo, MonitorStrategy, SeamlessWindow};
pub use input::{CaptureScope, ImeMode, InputManager};
pub use machine::{MachineEntry, MachineGroup, MachineManager};
pub use overlay::{OverlayMetrics, StreamOverlay};
pub use runtime::ClientRuntime;

#[cfg(test)]
mod tests;
