//! Undo/redo history.

/// An edit operation for undo/redo.
#[derive(Debug, Clone)]
pub enum EditOp {
    /// Characters were inserted.
    Insert {
        line: usize,
        col: usize,
        text: String,
    },
    /// Characters were deleted.
    Delete {
        line: usize,
        col: usize,
        text: String,
    },
    /// A newline was inserted (line split).
    InsertNewline {
        line: usize,
        col: usize,
    },
    /// A line was joined with the one above.
    JoinLine {
        line: usize,
        col: usize,
    },
}

/// Undo/redo history with configurable depth.
pub struct UndoHistory {
    undo_stack: Vec<EditOp>,
    redo_stack: Vec<EditOp>,
    limit: usize,
}

impl UndoHistory {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            limit,
        }
    }

    /// Record an edit operation.
    pub fn record(&mut self, op: EditOp) {
        self.redo_stack.clear();
        self.undo_stack.push(op);
        if self.undo_stack.len() > self.limit {
            self.undo_stack.remove(0);
        }
    }

    /// Pop the last edit for undo.
    pub fn undo(&mut self) -> Option<EditOp> {
        let op = self.undo_stack.pop()?;
        self.redo_stack.push(op.clone());
        Some(op)
    }

    /// Pop the last undone edit for redo.
    pub fn redo(&mut self) -> Option<EditOp> {
        let op = self.redo_stack.pop()?;
        self.undo_stack.push(op.clone());
        Some(op)
    }

    /// Whether undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }

    /// Whether redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    /// Number of items in undo stack.
    #[must_use]
    pub fn undo_depth(&self) -> usize { self.undo_stack.len() }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
