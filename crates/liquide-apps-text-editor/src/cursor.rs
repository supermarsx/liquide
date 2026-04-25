//! Cursor position, selection, and multi-cursor support.

/// A position in the text buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl Position {
    #[must_use]
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    #[must_use]
    pub fn zero() -> Self {
        Self { line: 0, col: 0 }
    }
}

/// A text selection (from anchor to cursor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub cursor: Position,
}

impl Selection {
    #[must_use]
    pub fn new(anchor: Position, cursor: Position) -> Self {
        Self { anchor, cursor }
    }

    /// Start of the selection (min position).
    #[must_use]
    pub fn start(&self) -> Position {
        if self.anchor <= self.cursor {
            self.anchor
        } else {
            self.cursor
        }
    }

    /// End of the selection (max position).
    #[must_use]
    pub fn end(&self) -> Position {
        if self.anchor >= self.cursor {
            self.anchor
        } else {
            self.cursor
        }
    }

    /// Whether the selection is empty (cursor == anchor).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    /// Whether the selection spans multiple lines.
    #[must_use]
    pub fn is_multiline(&self) -> bool {
        self.start().line != self.end().line
    }
}

/// Cursor state with optional selection.
#[derive(Debug, Clone)]
pub struct Cursor {
    pub position: Position,
    pub selection: Option<Selection>,
    /// Desired column when moving vertically.
    pub sticky_col: Option<usize>,
}

impl Cursor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            position: Position::zero(),
            selection: None,
            sticky_col: None,
        }
    }

    /// Move to a specific position, clearing selection.
    pub fn move_to(&mut self, pos: Position) {
        self.position = pos;
        self.selection = None;
        self.sticky_col = None;
    }

    /// Move to a position while extending the selection.
    pub fn select_to(&mut self, pos: Position) {
        let anchor = match &self.selection {
            Some(sel) => sel.anchor,
            None => self.position,
        };
        self.position = pos;
        self.selection = Some(Selection::new(anchor, pos));
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Select an entire line.
    pub fn select_line(&mut self, line: usize, line_len: usize) {
        let start = Position::new(line, 0);
        let end = Position::new(line, line_len);
        self.position = end;
        self.selection = Some(Selection::new(start, end));
    }

    /// Whether there is an active selection.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selection.as_ref().is_some_and(|s| !s.is_empty())
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-cursor state.
#[derive(Debug, Clone)]
pub struct MultiCursor {
    cursors: Vec<Cursor>,
}

impl MultiCursor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cursors: vec![Cursor::new()],
        }
    }

    /// Get the primary cursor.
    #[must_use]
    pub fn primary(&self) -> &Cursor {
        &self.cursors[0]
    }

    /// Get mutable primary cursor.
    pub fn primary_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[0]
    }

    /// Add an additional cursor.
    pub fn add_cursor(&mut self, pos: Position) {
        let mut c = Cursor::new();
        c.move_to(pos);
        self.cursors.push(c);
    }

    /// Remove all extra cursors, keeping only the primary.
    pub fn collapse(&mut self) {
        self.cursors.truncate(1);
    }

    /// Number of active cursors.
    #[must_use]
    pub fn count(&self) -> usize {
        self.cursors.len()
    }

    /// Get all cursors.
    #[must_use]
    pub fn all(&self) -> &[Cursor] {
        &self.cursors
    }
}

impl Default for MultiCursor {
    fn default() -> Self {
        Self::new()
    }
}
