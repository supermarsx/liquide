//! Built-in text editor for the LiquiDE desktop environment.
//!
//! This crate provides a lightweight text editor with syntax highlighting,
//! search/replace, undo/redo, and multiple document support.
//!
//! # Modules
//!
//! - [`config`] — Editor configuration (tab width, word wrap, etc.).
//! - [`buffer`] — Text buffer with line-oriented storage.
//! - [`cursor`] — Cursor position, selection, and multi-cursor.
//! - [`syntax`] — Syntax highlighting with language definitions.
//! - [`indent`] — Auto-indent logic.
//! - [`search`] — Search and replace.
//! - [`undo`] — Undo/redo history.
//! - [`gutter`] — Line number gutter and diagnostics.
//! - [`document`] — Document (file buffer + metadata).
//! - [`runtime`] — Editor runtime coordinator.

pub mod buffer;
pub mod config;
pub mod cursor;
pub mod document;
pub mod gutter;
pub mod indent;
pub mod runtime;
pub mod search;
pub mod syntax;
pub mod undo;

#[cfg(test)]
mod tests;

use liquide_app_harness::{AppBootstrap, Size};
use liquide_ui_widgets::Label;
use thiserror::Error;
use tracing::info;

/// Reverse-DNS application identifier for the text editor.
pub const APP_ID: &str = "com.liquide.apps.text-editor";

/// Display name used for the default text editor window.
pub const DISPLAY_NAME: &str = "Text Editor";

/// Initial window size for the default GUI launch path.
pub const DEFAULT_WINDOW_SIZE: Size = Size::new(1024, 768);

/// Runtime summary produced by the default GUI launch path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorLaunchState {
    pub document_count: usize,
    pub summary: String,
}

/// Errors produced by the text editor.
#[derive(Debug, Error)]
pub enum EditorError {
    #[error("buffer is empty")]
    EmptyBuffer,

    #[error("line {line} out of range (buffer has {total} lines)")]
    LineOutOfRange { line: usize, total: usize },

    #[error("column {col} out of range (line has {len} chars)")]
    ColumnOutOfRange { col: usize, len: usize },

    #[error("no active document")]
    NoActiveDocument,

    #[error("document not found: {id}")]
    DocumentNotFound { id: usize },

    #[error("nothing to undo")]
    NothingToUndo,

    #[error("nothing to redo")]
    NothingToRedo,

    #[error("no search matches")]
    NoMatches,

    #[error("no file path set for this document")]
    NoPath,

    #[error("I/O error: {0}")]
    Io(String),
}

impl From<std::io::Error> for EditorError {
    fn from(e: std::io::Error) -> Self {
        EditorError::Io(e.to_string())
    }
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, EditorError>;

// Re-exports for convenience.
pub use config::EditorConfig;
pub use document::LineEnding;
pub use runtime::{EditorLine, EditorRuntime};
pub use syntax::TokenKind;

/// Build the default application bootstrap used by the production binary.
#[must_use]
pub fn default_bootstrap() -> AppBootstrap {
    AppBootstrap::new(APP_ID, DISPLAY_NAME)
        .with_initial_size(DEFAULT_WINDOW_SIZE)
        .with_ime(true)
}

/// Build the runtime state surfaced by the default GUI launch path.
#[must_use]
pub fn default_launch_state(config: EditorConfig) -> EditorLaunchState {
    let mut runtime = EditorRuntime::new(config);
    let _ = runtime.new_document();
    let document_count = runtime.document_count();

    EditorLaunchState {
        document_count,
        summary: format!("liquid-text-editor — {document_count} document(s) open"),
    }
}

/// Build the default placeholder root widget.
#[must_use]
pub fn build_default_root(config: EditorConfig) -> Label {
    let state = default_launch_state(config);
    build_root_from_state(&state)
}

/// Build the placeholder root widget from a previously computed launch state.
#[must_use]
pub fn build_root_from_state(state: &EditorLaunchState) -> Label {
    Label::new(state.summary.clone())
}

/// Run the default text editor GUI path.
pub fn run_default_app() -> anyhow::Result<()> {
    let config = EditorConfig::default();
    let state = default_launch_state(config.clone());

    info!(font = %config.font_family, size = config.font_size, "Starting liquid-text-editor");

    default_bootstrap().run(move |_cx| Box::new(build_root_from_state(&state)))
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    #[test]
    fn default_launch_state_opens_a_document() {
        let state = default_launch_state(EditorConfig::default());

        assert_eq!(state.document_count, 1);
        assert_eq!(state.summary, "liquid-text-editor — 1 document(s) open");
    }
}
