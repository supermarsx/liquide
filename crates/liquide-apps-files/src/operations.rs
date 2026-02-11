//! File operations (copy, move, delete, rename, create).

use serde::{Deserialize, Serialize};

/// A pending file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperation {
    /// Operation kind.
    pub kind: OperationKind,
    /// Source paths.
    pub sources: Vec<String>,
    /// Destination path.
    pub destination: String,
    /// Operation state.
    pub state: OperationState,
    /// Progress (0.0-1.0).
    pub progress: f32,
    /// Bytes processed.
    pub bytes_processed: u64,
    /// Total bytes.
    pub bytes_total: u64,
    /// Files processed.
    pub files_processed: u32,
    /// Total files.
    pub files_total: u32,
}

/// Kind of file operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationKind {
    Copy,
    Move,
    Delete,
    Rename,
    CreateDirectory,
}

impl std::fmt::Display for OperationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Copy => write!(f, "copy"),
            Self::Move => write!(f, "move"),
            Self::Delete => write!(f, "delete"),
            Self::Rename => write!(f, "rename"),
            Self::CreateDirectory => write!(f, "create directory"),
        }
    }
}

/// State of a file operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationState {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

impl FileOperation {
    /// Create a new copy operation.
    #[must_use]
    pub fn copy(sources: Vec<String>, destination: String) -> Self {
        let files_total = sources.len() as u32;
        Self {
            kind: OperationKind::Copy,
            sources, destination,
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total,
        }
    }

    /// Create a new move operation.
    #[must_use]
    pub fn r#move(sources: Vec<String>, destination: String) -> Self {
        let files_total = sources.len() as u32;
        Self {
            kind: OperationKind::Move,
            sources, destination,
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total,
        }
    }

    /// Create a delete operation.
    #[must_use]
    pub fn delete(sources: Vec<String>) -> Self {
        let files_total = sources.len() as u32;
        Self {
            kind: OperationKind::Delete,
            sources, destination: String::new(),
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total,
        }
    }

    /// Create a rename operation.
    #[must_use]
    pub fn rename(old_path: String, new_path: String) -> Self {
        Self {
            kind: OperationKind::Rename,
            sources: vec![old_path], destination: new_path,
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total: 1,
        }
    }

    /// Create a mkdir operation.
    #[must_use]
    pub fn mkdir(path: String) -> Self {
        Self {
            kind: OperationKind::CreateDirectory,
            sources: Vec::new(), destination: path,
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total: 1,
        }
    }

    /// Update progress.
    pub fn update_progress(&mut self, bytes_done: u64, bytes_total: u64, files_done: u32) {
        self.bytes_processed = bytes_done;
        self.bytes_total = bytes_total;
        self.files_processed = files_done;
        self.progress = if bytes_total > 0 { bytes_done as f32 / bytes_total as f32 } else { 0.0 };
    }

    /// Mark as completed.
    pub fn complete(&mut self) {
        self.state = OperationState::Completed;
        self.progress = 1.0;
    }

    /// Mark as failed.
    pub fn fail(&mut self) {
        self.state = OperationState::Failed;
    }

    /// Cancel the operation.
    pub fn cancel(&mut self) {
        self.state = OperationState::Cancelled;
    }

    /// Whether the operation is finished (completed, failed, or cancelled).
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self.state, OperationState::Completed | OperationState::Failed | OperationState::Cancelled)
    }
}

/// Operation queue.
pub struct OperationQueue {
    operations: Vec<FileOperation>,
}

impl OperationQueue {
    #[must_use]
    pub fn new() -> Self { Self { operations: Vec::new() } }

    pub fn enqueue(&mut self, op: FileOperation) { self.operations.push(op); }

    #[must_use]
    pub fn pending(&self) -> Vec<&FileOperation> {
        self.operations.iter().filter(|o| !o.is_done()).collect()
    }

    #[must_use]
    pub fn all(&self) -> &[FileOperation] { &self.operations }

    pub fn clear_completed(&mut self) {
        self.operations.retain(|o| !o.is_done());
    }

    #[must_use]
    pub fn count(&self) -> usize { self.operations.len() }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.operations.iter().filter(|o| !o.is_done()).count()
    }
}

impl Default for OperationQueue {
    fn default() -> Self { Self::new() }
}
