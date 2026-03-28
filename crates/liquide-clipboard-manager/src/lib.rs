//! Clipboard manager with history, pinning, multi-format support, sensitive
//! data handling, persistence, sync, and platform-native clipboard bridges.
//!
//! This crate provides a full clipboard history manager for the LiquiDE
//! desktop environment, supporting:
//!
//! - **Multiple content types**: plain text, rich text (HTML), images, file
//!   paths, colours, and arbitrary MIME data.
//! - **Ring-buffer history** with configurable limits, deduplication, and
//!   full-text search.
//! - **Pinning**: pinned entries survive clear operations.
//! - **Sensitive mode**: suppresses storage for password fields; auto-clear
//!   policy with per-app exclusion and screen-lock purge.
//! - **Category filtering**: quickly filter by text, images, files, colours.
//! - **Text merging**: join multiple text entries with a separator.
//! - **Persistence**: save/load history to/from disk (binary format).
//! - **Sync**: trait-based multi-device clipboard synchronisation with a
//!   local stub for testing.
//! - **Platform clipboard bridge**: cfg-gated Win32, Linux (X11/Wayland),
//!   and macOS implementations.

pub mod entry;
pub mod history;
pub mod manager;
pub mod persistence;
pub mod platform;
pub mod sensitive;
pub mod sync;

// Re-exports for convenience.
pub use entry::{ClipboardContent, ClipboardEntry, ContentCategory, ImageFormat};
pub use history::ClipboardHistory;
pub use manager::ClipboardManager;
pub use persistence::{load_entries, save_entries, should_persist, PersistError, PersistResult};
pub use platform::{
    create_platform_clipboard, NullClipboard, PlatformClipboard, PlatformClipboardError,
    PlatformResult,
};
pub use sensitive::SensitiveClipboardPolicy;
pub use sync::{ClipboardSync, ClipboardSyncBackend, LocalSyncStub};

#[cfg(test)]
mod tests;
