//! Document (file buffer + metadata).

use crate::buffer::TextBuffer;
use crate::cursor::MultiCursor;
use crate::gutter::Gutter;
use crate::search::SearchReplace;
use crate::syntax::{Highlighter, Language};
use crate::undo::UndoHistory;

/// A document represents an open file with its associated state.
pub struct Document {
    /// Unique document ID.
    pub id: usize,
    /// File path (if saved).
    pub path: Option<String>,
    /// Display title.
    pub title: String,
    /// Text buffer.
    pub buffer: TextBuffer,
    /// Cursor state.
    pub cursors: MultiCursor,
    /// Undo history.
    pub history: UndoHistory,
    /// Syntax highlighter.
    pub highlighter: Highlighter,
    /// Gutter state.
    pub gutter: Gutter,
    /// Search/replace state.
    pub search: SearchReplace,
    /// Scroll offset (first visible line).
    pub scroll_top: usize,
    /// Horizontal scroll offset.
    pub scroll_left: usize,
}

impl Document {
    /// Create a new empty document.
    #[must_use]
    pub fn new(id: usize, undo_limit: usize) -> Self {
        Self {
            id,
            path: None,
            title: "Untitled".into(),
            buffer: TextBuffer::new(),
            cursors: MultiCursor::new(),
            history: UndoHistory::new(undo_limit),
            highlighter: Highlighter::new(None),
            gutter: Gutter::new(),
            search: SearchReplace::new(),
            scroll_top: 0,
            scroll_left: 0,
        }
    }

    /// Create a document from file contents.
    #[must_use]
    pub fn from_file(id: usize, path: &str, content: &str, undo_limit: usize) -> Self {
        let ext = path.rsplit('.').next().unwrap_or("");
        let lang = Language::from_extension(ext);
        let title = path.rsplit('/').next()
            .or_else(|| path.rsplit('\\').next())
            .unwrap_or(path)
            .to_string();

        let mut doc = Self {
            id,
            path: Some(path.into()),
            title,
            buffer: TextBuffer::from_text(content),
            cursors: MultiCursor::new(),
            history: UndoHistory::new(undo_limit),
            highlighter: Highlighter::new(lang),
            gutter: Gutter::new(),
            search: SearchReplace::new(),
            scroll_top: 0,
            scroll_left: 0,
        };
        doc.gutter.update_width(doc.buffer.line_count());
        doc
    }

    /// Whether the document has unsaved changes.
    #[must_use]
    pub fn is_modified(&self) -> bool { self.buffer.is_modified() }

    /// Mark as saved.
    pub fn mark_saved(&mut self) { self.buffer.mark_saved(); }

    /// The language name for this document.
    #[must_use]
    pub fn language_name(&self) -> &str { self.highlighter.language_name() }

    /// Display title with modification indicator.
    #[must_use]
    pub fn display_title(&self) -> String {
        if self.is_modified() {
            format!("{} *", self.title)
        } else {
            self.title.clone()
        }
    }

    /// Status bar info: line, column, language.
    #[must_use]
    pub fn status_info(&self) -> (usize, usize, &str) {
        let pos = self.cursors.primary().position;
        (pos.line + 1, pos.col + 1, self.language_name())
    }
}
