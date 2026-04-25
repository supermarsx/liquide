//! File clipboard for copy/cut/paste operations.

use crate::entry::FileEntry;

/// Clipboard operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

/// File clipboard state.
pub struct FileClipboard {
    entries: Vec<FileEntry>,
    operation: Option<ClipboardOp>,
}

impl FileClipboard {
    /// Create a new empty clipboard.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            operation: None,
        }
    }

    /// Copy entries to the clipboard.
    pub fn copy(&mut self, entries: Vec<FileEntry>) {
        self.entries = entries;
        self.operation = Some(ClipboardOp::Copy);
    }

    /// Cut entries to the clipboard.
    pub fn cut(&mut self, entries: Vec<FileEntry>) {
        self.entries = entries;
        self.operation = Some(ClipboardOp::Cut);
    }

    /// Get clipboard entries.
    #[must_use]
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Get the clipboard operation.
    #[must_use]
    pub fn operation(&self) -> Option<ClipboardOp> {
        self.operation
    }

    /// Whether the clipboard has entries.
    #[must_use]
    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Number of entries in the clipboard.
    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Clear the clipboard.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.operation = None;
    }

    /// Take entries and operation, consuming the clipboard contents.
    pub fn take(&mut self) -> (Vec<FileEntry>, Option<ClipboardOp>) {
        let entries = std::mem::take(&mut self.entries);
        let op = self.operation.take();
        (entries, op)
    }
}

impl Default for FileClipboard {
    fn default() -> Self {
        Self::new()
    }
}
