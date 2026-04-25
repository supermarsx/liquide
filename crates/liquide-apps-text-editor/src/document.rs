//! Document (file buffer + metadata).

use std::path::Path;

use crate::buffer::TextBuffer;
use crate::cursor::MultiCursor;
use crate::gutter::Gutter;
use crate::search::SearchReplace;
use crate::syntax::{Highlighter, Language};
use crate::undo::{EditOp, UndoHistory};

/// Line ending style detected from the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    /// The string representation of this line ending.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

impl Default for LineEnding {
    fn default() -> Self {
        Self::Lf
    }
}

/// Detect the line ending style used in a string.
#[must_use]
fn detect_line_ending(content: &str) -> LineEnding {
    if content.contains("\r\n") {
        LineEnding::CrLf
    } else if content.contains('\r') {
        LineEnding::Cr
    } else {
        LineEnding::Lf
    }
}

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
    /// Detected line ending style.
    pub line_ending: LineEnding,
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
            line_ending: LineEnding::default(),
        }
    }

    /// Create a document from file contents (in-memory, no disk I/O).
    #[must_use]
    pub fn from_file(id: usize, path: &str, content: &str, undo_limit: usize) -> Self {
        let ext = path.rsplit('.').next().unwrap_or("");
        let lang = Language::from_extension(ext);
        let title = path
            .rsplit('/')
            .next()
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
            line_ending: detect_line_ending(content),
        };
        doc.gutter.update_width(doc.buffer.line_count());
        doc
    }

    /// Open a document from a file path on disk.
    pub fn open(id: usize, path: &Path, undo_limit: usize) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let line_ending = detect_line_ending(&content);
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = Language::from_extension(ext);
        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let path_str = path.to_string_lossy().to_string();

        let mut doc = Self {
            id,
            path: Some(path_str),
            title,
            buffer: TextBuffer::from_lines(lines),
            cursors: MultiCursor::new(),
            history: UndoHistory::new(undo_limit),
            highlighter: Highlighter::new(lang),
            gutter: Gutter::new(),
            search: SearchReplace::new(),
            scroll_top: 0,
            scroll_left: 0,
            line_ending,
        };
        doc.gutter.update_width(doc.buffer.line_count());
        Ok(doc)
    }

    /// Save the document to its current path.
    pub fn save(&mut self) -> crate::Result<()> {
        let path = self.path.clone().ok_or(crate::EditorError::NoPath)?;
        let eol = self.line_ending.as_str();
        let content = self.buffer.lines().join(eol);
        std::fs::write(&path, &content)?;
        self.buffer.mark_saved();
        Ok(())
    }

    /// Save the document to a new path.
    pub fn save_as(&mut self, path: &Path) -> crate::Result<()> {
        let path_str = path.to_string_lossy().to_string();
        self.path = Some(path_str);
        self.title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        // Re-detect language from new extension.
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.highlighter.set_language(Language::from_extension(ext));
        self.save()
    }

    /// Whether the document has unsaved changes.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Mark as saved.
    pub fn mark_saved(&mut self) {
        self.buffer.mark_saved();
    }

    /// The language name for this document.
    #[must_use]
    pub fn language_name(&self) -> &str {
        self.highlighter.language_name()
    }

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

    // -----------------------------------------------------------------------
    // Undo / Redo integration
    // -----------------------------------------------------------------------

    /// Record a single-character insert for undo.
    pub fn record_insert(&mut self, line: usize, col: usize, text: String) {
        self.history.record(EditOp::Insert { line, col, text });
    }

    /// Record a single-character delete for undo.
    pub fn record_delete(&mut self, line: usize, col: usize, text: String) {
        self.history.record(EditOp::Delete { line, col, text });
    }

    /// Record a newline insertion for undo.
    pub fn record_insert_newline(&mut self, line: usize, col: usize) {
        self.history.record(EditOp::InsertNewline { line, col });
    }

    /// Record a line join for undo.
    pub fn record_join_line(&mut self, line: usize, col: usize) {
        self.history.record(EditOp::JoinLine { line, col });
    }

    /// Undo the last edit, returning true if something was undone.
    pub fn undo(&mut self) -> bool {
        let Some(op) = self.history.undo() else {
            return false;
        };
        self.apply_undo_op(&op);
        true
    }

    /// Redo the last undone edit, returning true if something was redone.
    pub fn redo(&mut self) -> bool {
        let Some(op) = self.history.redo() else {
            return false;
        };
        self.apply_redo_op(&op);
        true
    }

    /// Apply the inverse of an edit operation (for undo).
    fn apply_undo_op(&mut self, op: &EditOp) {
        let cursor = self.cursors.primary_mut();
        match op {
            EditOp::Insert { line, col, text } => {
                // Undo an insert = delete the inserted text.
                for _ in 0..text.len() {
                    let _ = self.buffer.delete_char(*line, *col);
                }
                cursor.move_to(crate::cursor::Position::new(*line, *col));
            }
            EditOp::Delete { line, col, text } => {
                // Undo a delete = re-insert the deleted text.
                for (i, ch) in text.chars().enumerate() {
                    let _ = self.buffer.insert_char(*line, *col + i, ch);
                }
                cursor.move_to(crate::cursor::Position::new(*line, *col + text.len()));
            }
            EditOp::InsertNewline { line, col } => {
                // Undo a newline insert = join the line back up.
                let _ = self.buffer.join_line_up(*line + 1);
                cursor.move_to(crate::cursor::Position::new(*line, *col));
            }
            EditOp::JoinLine { line, col } => {
                // Undo a join = re-insert the newline.
                let _ = self.buffer.insert_newline(*line, *col);
                cursor.move_to(crate::cursor::Position::new(*line + 1, 0));
            }
        }
    }

    /// Apply an edit operation forward (for redo).
    fn apply_redo_op(&mut self, op: &EditOp) {
        let cursor = self.cursors.primary_mut();
        match op {
            EditOp::Insert { line, col, text } => {
                for (i, ch) in text.chars().enumerate() {
                    let _ = self.buffer.insert_char(*line, *col + i, ch);
                }
                cursor.move_to(crate::cursor::Position::new(*line, *col + text.len()));
            }
            EditOp::Delete { line, col, text } => {
                for _ in 0..text.len() {
                    let _ = self.buffer.delete_char(*line, *col);
                }
                cursor.move_to(crate::cursor::Position::new(*line, *col));
            }
            EditOp::InsertNewline { line, col } => {
                let _ = self.buffer.insert_newline(*line, *col);
                cursor.move_to(crate::cursor::Position::new(*line + 1, 0));
            }
            EditOp::JoinLine { line, col } => {
                let _ = self.buffer.join_line_up(*line + 1);
                cursor.move_to(crate::cursor::Position::new(*line, *col));
            }
        }
    }
}
