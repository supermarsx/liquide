//! Editor runtime coordinator.

use crate::config::EditorConfig;
use crate::document::Document;

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

    /// Open a document from file contents.
    pub fn open_file(&mut self, path: &str, content: &str) -> usize {
        // Check if already open.
        if let Some(doc) = self.documents.iter().find(|d| d.path.as_deref() == Some(path)) {
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

    /// Close a document by ID.
    pub fn close_document(&mut self, id: usize) -> crate::Result<()> {
        let pos = self.documents.iter().position(|d| d.id == id)
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
        self.documents.iter()
            .map(|d| (d.id, d.display_title()))
            .collect()
    }

    /// Number of open documents.
    #[must_use]
    pub fn document_count(&self) -> usize { self.documents.len() }

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
    pub fn config(&self) -> &EditorConfig { &self.config }
}
