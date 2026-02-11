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

pub mod config;
pub mod buffer;
pub mod cursor;
pub mod syntax;
pub mod indent;
pub mod search;
pub mod undo;
pub mod gutter;
pub mod document;
pub mod runtime;

#[cfg(test)]
mod tests;

use thiserror::Error;

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

    #[error("I/O error: {0}")]
    Io(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, EditorError>;

// Re-exports for convenience.
pub use config::EditorConfig;
pub use runtime::EditorRuntime;
