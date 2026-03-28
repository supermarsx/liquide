//! File operations (copy, move, delete, rename, create, compress, extract).

use serde::{Deserialize, Serialize};

/// Archive format for compress/extract operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
    TarBz2,
    TarXz,
    SevenZip,
}

impl std::fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip => write!(f, "zip"),
            Self::TarGz => write!(f, "tar.gz"),
            Self::TarBz2 => write!(f, "tar.bz2"),
            Self::TarXz => write!(f, "tar.xz"),
            Self::SevenZip => write!(f, "7z"),
        }
    }
}

impl ArchiveFormat {
    /// Guess archive format from file extension.
    #[must_use]
    pub fn from_extension(path: &str) -> Option<Self> {
        let lower = path.to_lowercase();
        if lower.ends_with(".zip") {
            Some(Self::Zip)
        } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Some(Self::TarGz)
        } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
            Some(Self::TarBz2)
        } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
            Some(Self::TarXz)
        } else if lower.ends_with(".7z") {
            Some(Self::SevenZip)
        } else {
            None
        }
    }
}

/// An async-style file operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOp {
    /// Copy files to a destination.
    Copy { sources: Vec<String>, destination: String },
    /// Move files to a destination.
    Move { sources: Vec<String>, destination: String },
    /// Delete files (optionally to trash).
    Delete { paths: Vec<String>, trash: bool },
    /// Rename a file or directory.
    Rename { path: String, new_name: String },
    /// Create a new folder.
    CreateFolder { parent: String, name: String },
    /// Create a new empty file.
    CreateFile { parent: String, name: String },
    /// Compress files into an archive.
    Compress { sources: Vec<String>, archive_path: String, format: ArchiveFormat },
    /// Extract an archive to a destination.
    Extract { archive_path: String, destination: String },
}

impl std::fmt::Display for FileOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Copy { sources, destination } => {
                write!(f, "copy {} item(s) to {}", sources.len(), destination)
            }
            Self::Move { sources, destination } => {
                write!(f, "move {} item(s) to {}", sources.len(), destination)
            }
            Self::Delete { paths, trash } => {
                if *trash {
                    write!(f, "trash {} item(s)", paths.len())
                } else {
                    write!(f, "delete {} item(s)", paths.len())
                }
            }
            Self::Rename { path, new_name } => write!(f, "rename {} to {}", path, new_name),
            Self::CreateFolder { parent, name } => write!(f, "create folder {}/{}", parent, name),
            Self::CreateFile { parent, name } => write!(f, "create file {}/{}", parent, name),
            Self::Compress { sources, archive_path, format } => {
                write!(f, "compress {} item(s) to {} ({})", sources.len(), archive_path, format)
            }
            Self::Extract { archive_path, destination } => {
                write!(f, "extract {} to {}", archive_path, destination)
            }
        }
    }
}

/// Progress tracking for file operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationProgress {
    /// Total bytes to process.
    pub total_bytes: u64,
    /// Bytes completed so far.
    pub completed_bytes: u64,
    /// Total number of items to process.
    pub total_items: u32,
    /// Number of items completed.
    pub completed_items: u32,
    /// Path of the file currently being processed.
    pub current_file: String,
    /// Transfer speed in bytes per second.
    pub speed_bytes_per_sec: u64,
    /// Estimated time remaining in seconds.
    pub eta_seconds: u32,
}

impl OperationProgress {
    /// Create a new progress tracker.
    #[must_use]
    pub fn new(total_bytes: u64, total_items: u32) -> Self {
        Self {
            total_bytes,
            completed_bytes: 0,
            total_items,
            completed_items: 0,
            current_file: String::new(),
            speed_bytes_per_sec: 0,
            eta_seconds: 0,
        }
    }

    /// Calculate progress as a percentage (0.0 to 100.0).
    #[must_use]
    pub fn progress_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            if self.total_items == 0 {
                return 0.0;
            }
            return self.completed_items as f32 / self.total_items as f32 * 100.0;
        }
        self.completed_bytes as f32 / self.total_bytes as f32 * 100.0
    }

    /// Update progress with current state.
    pub fn update(
        &mut self,
        completed_bytes: u64,
        completed_items: u32,
        current_file: String,
        speed: u64,
    ) {
        self.completed_bytes = completed_bytes;
        self.completed_items = completed_items;
        self.current_file = current_file;
        self.speed_bytes_per_sec = speed;
        if speed > 0 {
            let remaining = self.total_bytes.saturating_sub(completed_bytes);
            self.eta_seconds = (remaining / speed) as u32;
        } else {
            self.eta_seconds = 0;
        }
    }

    /// Whether the operation is complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.completed_bytes >= self.total_bytes && self.completed_items >= self.total_items
    }
}

impl Default for OperationProgress {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// A pending file operation (legacy struct, kept for backward compat).
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
    CreateFile,
    Compress,
    Extract,
}

impl std::fmt::Display for OperationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Copy => write!(f, "copy"),
            Self::Move => write!(f, "move"),
            Self::Delete => write!(f, "delete"),
            Self::Rename => write!(f, "rename"),
            Self::CreateDirectory => write!(f, "create directory"),
            Self::CreateFile => write!(f, "create file"),
            Self::Compress => write!(f, "compress"),
            Self::Extract => write!(f, "extract"),
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

    /// Create a file creation operation.
    #[must_use]
    pub fn create_file(parent: String, name: String) -> Self {
        Self {
            kind: OperationKind::CreateFile,
            sources: vec![name], destination: parent,
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total: 1,
        }
    }

    /// Create a compress operation.
    #[must_use]
    pub fn compress(sources: Vec<String>, archive_path: String) -> Self {
        let files_total = sources.len() as u32;
        Self {
            kind: OperationKind::Compress,
            sources, destination: archive_path,
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total,
        }
    }

    /// Create an extract operation.
    #[must_use]
    pub fn extract(archive_path: String, destination: String) -> Self {
        Self {
            kind: OperationKind::Extract,
            sources: vec![archive_path], destination,
            state: OperationState::Pending,
            progress: 0.0, bytes_processed: 0, bytes_total: 0,
            files_processed: 0, files_total: 1,
        }
    }

    /// Create a `FileOp` enum from this operation (for the new async-style API).
    #[must_use]
    pub fn to_file_op(&self) -> Option<FileOp> {
        match self.kind {
            OperationKind::Copy => Some(FileOp::Copy {
                sources: self.sources.clone(),
                destination: self.destination.clone(),
            }),
            OperationKind::Move => Some(FileOp::Move {
                sources: self.sources.clone(),
                destination: self.destination.clone(),
            }),
            OperationKind::Delete => Some(FileOp::Delete {
                paths: self.sources.clone(),
                trash: false,
            }),
            OperationKind::Rename => {
                let path = self.sources.first()?.clone();
                Some(FileOp::Rename {
                    path,
                    new_name: self.destination.clone(),
                })
            }
            OperationKind::CreateDirectory => {
                // Destination is the full path; split into parent + name.
                let dest = &self.destination;
                if let Some((parent, name)) = dest.rsplit_once('/') {
                    Some(FileOp::CreateFolder {
                        parent: if parent.is_empty() { "/".into() } else { parent.into() },
                        name: name.into(),
                    })
                } else {
                    Some(FileOp::CreateFolder {
                        parent: ".".into(),
                        name: dest.clone(),
                    })
                }
            }
            OperationKind::CreateFile => Some(FileOp::CreateFile {
                parent: self.destination.clone(),
                name: self.sources.first().cloned().unwrap_or_default(),
            }),
            OperationKind::Compress => Some(FileOp::Compress {
                sources: self.sources.clone(),
                archive_path: self.destination.clone(),
                format: ArchiveFormat::from_extension(&self.destination).unwrap_or(ArchiveFormat::Zip),
            }),
            OperationKind::Extract => Some(FileOp::Extract {
                archive_path: self.sources.first().cloned().unwrap_or_default(),
                destination: self.destination.clone(),
            }),
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
