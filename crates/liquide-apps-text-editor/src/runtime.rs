//! Editor runtime coordinator.

use std::path::Path;

use crate::config::EditorConfig;
use crate::cursor::Position;
use crate::document::Document;
use crate::syntax::Token;

/// A rendered line for display, including syntax highlighting and metadata.
#[derive(Debug, Clone)]
pub struct EditorLine {
    /// 1-based line number.
    pub number: usize,
    /// The raw text content of the line.
    pub text: String,
    /// Syntax highlighting spans.
    pub highlights: Vec<Token>,
    /// Whether this is the line the cursor is on.
    pub is_current: bool,
}

/// The editor runtime managing multiple open documents.
pub struct EditorRuntime {
    config: EditorConfig,
    documents: Vec<Document>,
    active_id: Option<usize>,
    next_id: usize,
}

impl EditorRuntime {
    /// Create a new editor runtime.
    #[must_use]
    pub fn new(config: EditorConfig) -> Self {
        Self {
            config,
            documents: Vec::new(),
            active_id: None,
            next_id: 1,
        }
    }

    /// Open a new empty document.
    pub fn new_document(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let doc = Document::new(id, self.config.undo_limit);
        self.documents.push(doc);
        self.active_id = Some(id);
        id
    }

    /// Open a document from file contents (in-memory, no disk I/O).
    pub fn open_file(&mut self, path: &str, content: &str) -> usize {
        // Check if already open.
        if let Some(doc) = self
            .documents
            .iter()
            .find(|d| d.path.as_deref() == Some(path))
        {
            let id = doc.id;
            self.active_id = Some(id);
            return id;
        }

        let id = self.next_id;
        self.next_id += 1;
        let doc = Document::from_file(id, path, content, self.config.undo_limit);
        self.documents.push(doc);
        self.active_id = Some(id);
        id
    }

    /// Open a document from a file path on disk.
    pub fn open_path(&mut self, path: &Path) -> crate::Result<usize> {
        let path_str = path.to_string_lossy().to_string();

        // Check if already open.
        if let Some(doc) = self
            .documents
            .iter()
            .find(|d| d.path.as_deref() == Some(&path_str))
        {
            let id = doc.id;
            self.active_id = Some(id);
            return Ok(id);
        }

        let id = self.next_id;
        self.next_id += 1;
        let doc = Document::open(id, path, self.config.undo_limit)?;
        self.documents.push(doc);
        self.active_id = Some(id);
        Ok(id)
    }

    /// Close a document by ID.
    pub fn close_document(&mut self, id: usize) -> crate::Result<()> {
        let pos = self
            .documents
            .iter()
            .position(|d| d.id == id)
            .ok_or(crate::EditorError::DocumentNotFound { id })?;
        self.documents.remove(pos);

        if self.active_id == Some(id) {
            self.active_id = self.documents.last().map(|d| d.id);
        }
        Ok(())
    }

    /// Get the active document.
    #[must_use]
    pub fn active_document(&self) -> Option<&Document> {
        let id = self.active_id?;
        self.documents.iter().find(|d| d.id == id)
    }

    /// Get the active document mutably.
    pub fn active_document_mut(&mut self) -> Option<&mut Document> {
        let id = self.active_id?;
        self.documents.iter_mut().find(|d| d.id == id)
    }

    /// Set the active document.
    pub fn set_active(&mut self, id: usize) -> crate::Result<()> {
        if !self.documents.iter().any(|d| d.id == id) {
            return Err(crate::EditorError::DocumentNotFound { id });
        }
        self.active_id = Some(id);
        Ok(())
    }

    /// Get all document IDs and titles.
    #[must_use]
    pub fn document_list(&self) -> Vec<(usize, String)> {
        self.documents
            .iter()
            .map(|d| (d.id, d.display_title()))
            .collect()
    }

    /// Number of open documents.
    #[must_use]
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Get a document by ID.
    #[must_use]
    pub fn document(&self, id: usize) -> Option<&Document> {
        self.documents.iter().find(|d| d.id == id)
    }

    /// Get a mutable document by ID.
    pub fn document_mut(&mut self, id: usize) -> Option<&mut Document> {
        self.documents.iter_mut().find(|d| d.id == id)
    }

    /// Whether any document has unsaved changes.
    #[must_use]
    pub fn has_unsaved_changes(&self) -> bool {
        self.documents.iter().any(|d| d.is_modified())
    }

    /// Get the config.
    #[must_use]
    pub fn config(&self) -> &EditorConfig {
        &self.config
    }

    // =======================================================================
    // Keyboard event handling
    // =======================================================================

    /// Handle a keyboard event. Returns true if the document content was modified.
    ///
    /// `key` uses the web KeyboardEvent.key naming convention:
    /// `"ArrowUp"`, `"ArrowDown"`, `"ArrowLeft"`, `"ArrowRight"`,
    /// `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`,
    /// `"Enter"`, `"Backspace"`, `"Delete"`, `"Tab"`,
    /// single-character keys like `"s"`, `"z"`, `"y"`, `"a"`.
    pub fn handle_key(&mut self, key: &str, ctrl: bool, shift: bool) -> bool {
        // Capture config values before mutable borrow of documents.
        let tab_width = self.config.tab_width;
        let use_spaces = self.config.use_spaces;

        let doc = match self.active_document_mut() {
            Some(d) => d,
            None => return false,
        };

        match (key, ctrl, shift) {
            // ---- File operations ----
            ("s", true, false) => {
                let _ = doc.save();
                false
            }

            // ---- Undo / Redo ----
            ("z", true, false) => doc.undo(),
            ("y", true, false) | ("z", true, true) => doc.redo(),

            // ---- Cursor movement (no shift = clear selection) ----
            ("ArrowUp", false, false) => {
                let pos = doc.cursors.primary().position;
                let new_line = pos.line.saturating_sub(1);
                let col = doc.cursors.primary().sticky_col.unwrap_or(pos.col);
                let clamped_col = col.min(doc.buffer.line_len(new_line));
                let cursor = doc.cursors.primary_mut();
                cursor.position = Position::new(new_line, clamped_col);
                cursor.selection = None;
                if cursor.sticky_col.is_none() {
                    cursor.sticky_col = Some(pos.col);
                }
                false
            }
            ("ArrowDown", false, false) => {
                let pos = doc.cursors.primary().position;
                let max_line = doc.buffer.line_count().saturating_sub(1);
                let new_line = (pos.line + 1).min(max_line);
                let col = doc.cursors.primary().sticky_col.unwrap_or(pos.col);
                let clamped_col = col.min(doc.buffer.line_len(new_line));
                let cursor = doc.cursors.primary_mut();
                cursor.position = Position::new(new_line, clamped_col);
                cursor.selection = None;
                if cursor.sticky_col.is_none() {
                    cursor.sticky_col = Some(pos.col);
                }
                false
            }
            ("ArrowLeft", false, false) => {
                let pos = doc.cursors.primary().position;
                let new_pos = if pos.col > 0 {
                    Position::new(pos.line, pos.col - 1)
                } else if pos.line > 0 {
                    let prev_len = doc.buffer.line_len(pos.line - 1);
                    Position::new(pos.line - 1, prev_len)
                } else {
                    pos
                };
                doc.cursors.primary_mut().move_to(new_pos);
                false
            }
            ("ArrowRight", false, false) => {
                let pos = doc.cursors.primary().position;
                let line_len = doc.buffer.line_len(pos.line);
                let new_pos = if pos.col < line_len {
                    Position::new(pos.line, pos.col + 1)
                } else if pos.line + 1 < doc.buffer.line_count() {
                    Position::new(pos.line + 1, 0)
                } else {
                    pos
                };
                doc.cursors.primary_mut().move_to(new_pos);
                false
            }
            ("Home", false, false) => {
                let line = doc.cursors.primary().position.line;
                doc.cursors.primary_mut().move_to(Position::new(line, 0));
                false
            }
            ("End", false, false) => {
                let line = doc.cursors.primary().position.line;
                let len = doc.buffer.line_len(line);
                doc.cursors.primary_mut().move_to(Position::new(line, len));
                false
            }
            ("Home", true, false) => {
                doc.cursors.primary_mut().move_to(Position::new(0, 0));
                false
            }
            ("End", true, false) => {
                let last = doc.buffer.line_count().saturating_sub(1);
                let len = doc.buffer.line_len(last);
                doc.cursors.primary_mut().move_to(Position::new(last, len));
                false
            }
            ("PageUp", false, false) => {
                let pos = doc.cursors.primary().position;
                let new_line = pos.line.saturating_sub(30);
                let clamped_col = pos.col.min(doc.buffer.line_len(new_line));
                doc.cursors
                    .primary_mut()
                    .move_to(Position::new(new_line, clamped_col));
                false
            }
            ("PageDown", false, false) => {
                let pos = doc.cursors.primary().position;
                let max_line = doc.buffer.line_count().saturating_sub(1);
                let new_line = (pos.line + 30).min(max_line);
                let clamped_col = pos.col.min(doc.buffer.line_len(new_line));
                doc.cursors
                    .primary_mut()
                    .move_to(Position::new(new_line, clamped_col));
                false
            }

            // ---- Selection (shift + movement) ----
            ("ArrowUp", false, true) => {
                let pos = doc.cursors.primary().position;
                let new_line = pos.line.saturating_sub(1);
                let col = doc.cursors.primary().sticky_col.unwrap_or(pos.col);
                let clamped_col = col.min(doc.buffer.line_len(new_line));
                let new_pos = Position::new(new_line, clamped_col);
                let cursor = doc.cursors.primary_mut();
                if cursor.sticky_col.is_none() {
                    cursor.sticky_col = Some(pos.col);
                }
                cursor.select_to(new_pos);
                false
            }
            ("ArrowDown", false, true) => {
                let pos = doc.cursors.primary().position;
                let max_line = doc.buffer.line_count().saturating_sub(1);
                let new_line = (pos.line + 1).min(max_line);
                let col = doc.cursors.primary().sticky_col.unwrap_or(pos.col);
                let clamped_col = col.min(doc.buffer.line_len(new_line));
                let new_pos = Position::new(new_line, clamped_col);
                let cursor = doc.cursors.primary_mut();
                if cursor.sticky_col.is_none() {
                    cursor.sticky_col = Some(pos.col);
                }
                cursor.select_to(new_pos);
                false
            }
            ("ArrowLeft", false, true) => {
                let pos = doc.cursors.primary().position;
                let new_pos = if pos.col > 0 {
                    Position::new(pos.line, pos.col - 1)
                } else if pos.line > 0 {
                    let prev_len = doc.buffer.line_len(pos.line - 1);
                    Position::new(pos.line - 1, prev_len)
                } else {
                    pos
                };
                doc.cursors.primary_mut().select_to(new_pos);
                false
            }
            ("ArrowRight", false, true) => {
                let pos = doc.cursors.primary().position;
                let line_len = doc.buffer.line_len(pos.line);
                let new_pos = if pos.col < line_len {
                    Position::new(pos.line, pos.col + 1)
                } else if pos.line + 1 < doc.buffer.line_count() {
                    Position::new(pos.line + 1, 0)
                } else {
                    pos
                };
                doc.cursors.primary_mut().select_to(new_pos);
                false
            }
            ("Home", false, true) => {
                let line = doc.cursors.primary().position.line;
                doc.cursors.primary_mut().select_to(Position::new(line, 0));
                false
            }
            ("End", false, true) => {
                let line = doc.cursors.primary().position.line;
                let len = doc.buffer.line_len(line);
                doc.cursors
                    .primary_mut()
                    .select_to(Position::new(line, len));
                false
            }
            ("Home", true, true) => {
                doc.cursors.primary_mut().select_to(Position::new(0, 0));
                false
            }
            ("End", true, true) => {
                let last = doc.buffer.line_count().saturating_sub(1);
                let len = doc.buffer.line_len(last);
                doc.cursors
                    .primary_mut()
                    .select_to(Position::new(last, len));
                false
            }

            // ---- Editing ----
            ("Enter", false, false) => {
                let pos = doc.cursors.primary().position;
                doc.record_insert_newline(pos.line, pos.col);
                let _ = doc.buffer.insert_newline(pos.line, pos.col);
                doc.cursors
                    .primary_mut()
                    .move_to(Position::new(pos.line + 1, 0));
                doc.gutter.update_width(doc.buffer.line_count());
                true
            }
            ("Backspace", false, false) => {
                let pos = doc.cursors.primary().position;
                if pos.col > 0 {
                    let ch = doc
                        .buffer
                        .line(pos.line)
                        .and_then(|l| l.chars().nth(pos.col - 1))
                        .unwrap_or(' ');
                    doc.record_delete(pos.line, pos.col - 1, ch.to_string());
                    let _ = doc.buffer.delete_char(pos.line, pos.col - 1);
                    doc.cursors
                        .primary_mut()
                        .move_to(Position::new(pos.line, pos.col - 1));
                    true
                } else if pos.line > 0 {
                    let prev_len = doc.buffer.line_len(pos.line - 1);
                    doc.record_join_line(pos.line - 1, prev_len);
                    let _ = doc.buffer.join_line_up(pos.line);
                    doc.cursors
                        .primary_mut()
                        .move_to(Position::new(pos.line - 1, prev_len));
                    doc.gutter.update_width(doc.buffer.line_count());
                    true
                } else {
                    false
                }
            }
            ("Delete", false, false) => {
                let pos = doc.cursors.primary().position;
                let line_len = doc.buffer.line_len(pos.line);
                if pos.col < line_len {
                    let ch = doc
                        .buffer
                        .line(pos.line)
                        .and_then(|l| l.chars().nth(pos.col))
                        .unwrap_or(' ');
                    doc.record_delete(pos.line, pos.col, ch.to_string());
                    let _ = doc.buffer.delete_char(pos.line, pos.col);
                    true
                } else if pos.line + 1 < doc.buffer.line_count() {
                    doc.record_join_line(pos.line, line_len);
                    let _ = doc.buffer.join_line_up(pos.line + 1);
                    doc.gutter.update_width(doc.buffer.line_count());
                    true
                } else {
                    false
                }
            }
            ("Tab", false, false) => {
                let pos = doc.cursors.primary().position;
                let indent_str = if use_spaces {
                    " ".repeat(tab_width)
                } else {
                    "\t".to_string()
                };
                doc.record_insert(pos.line, pos.col, indent_str.clone());
                for (i, ch) in indent_str.chars().enumerate() {
                    let _ = doc.buffer.insert_char(pos.line, pos.col + i, ch);
                }
                doc.cursors
                    .primary_mut()
                    .move_to(Position::new(pos.line, pos.col + indent_str.len()));
                true
            }

            // ---- Select all ----
            ("a", true, false) => {
                let last = doc.buffer.line_count().saturating_sub(1);
                let len = doc.buffer.line_len(last);
                let cursor = doc.cursors.primary_mut();
                cursor.position = Position::new(last, len);
                cursor.selection = Some(crate::cursor::Selection::new(
                    Position::new(0, 0),
                    Position::new(last, len),
                ));
                false
            }

            // ---- Clipboard placeholders (no system clipboard access in this crate) ----
            ("c", true, false) | ("x", true, false) | ("v", true, false) => {
                // Copy/cut/paste require system clipboard integration,
                // which is provided by the shell/platform layer.
                false
            }

            _ => false,
        }
    }

    /// Handle a typed character. Returns true if the document was modified.
    pub fn handle_char(&mut self, ch: char) -> bool {
        let doc = match self.active_document_mut() {
            Some(d) => d,
            None => return false,
        };

        let pos = doc.cursors.primary().position;
        doc.record_insert(pos.line, pos.col, ch.to_string());
        let _ = doc.buffer.insert_char(pos.line, pos.col, ch);
        doc.cursors
            .primary_mut()
            .move_to(Position::new(pos.line, pos.col + 1));
        true
    }

    /// Get visible lines for rendering (with syntax highlighting).
    #[must_use]
    pub fn visible_lines(&self, scroll_offset: usize, visible_rows: usize) -> Vec<EditorLine> {
        let doc = match self.active_document() {
            Some(d) => d,
            None => return Vec::new(),
        };

        let start = scroll_offset.min(doc.buffer.line_count());
        let end = (start + visible_rows).min(doc.buffer.line_count());
        let cursor_line = doc.cursors.primary().position.line;

        let mut lines = Vec::with_capacity(end - start);
        for i in start..end {
            let text = doc.buffer.line(i).unwrap_or("").to_string();
            let highlights = doc.highlighter.tokenize_line(&text);
            lines.push(EditorLine {
                number: i + 1,
                text,
                highlights,
                is_current: i == cursor_line,
            });
        }
        lines
    }

    /// Save the active document. Returns an error if no document is active or no path is set.
    pub fn save_active(&mut self) -> crate::Result<()> {
        let doc = self
            .active_document_mut()
            .ok_or(crate::EditorError::NoActiveDocument)?;
        doc.save()
    }

    /// Save the active document to a new path.
    pub fn save_active_as(&mut self, path: &Path) -> crate::Result<()> {
        let doc = self
            .active_document_mut()
            .ok_or(crate::EditorError::NoActiveDocument)?;
        doc.save_as(path)
    }
}
