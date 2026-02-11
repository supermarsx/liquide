//! File and directory entry types.

use serde::{Deserialize, Serialize};

/// Type of filesystem entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Device,
    Socket,
    Pipe,
    Unknown,
}

impl std::fmt::Display for EntryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "file"),
            Self::Directory => write!(f, "directory"),
            Self::Symlink => write!(f, "symlink"),
            Self::Device => write!(f, "device"),
            Self::Socket => write!(f, "socket"),
            Self::Pipe => write!(f, "pipe"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Permission mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Permissions {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    /// Unix mode bits (e.g. 0o755).
    pub mode: u32,
}

impl Permissions {
    /// Create from unix mode bits.
    #[must_use]
    pub fn from_mode(mode: u32) -> Self {
        Self {
            readable: mode & 0o400 != 0,
            writable: mode & 0o200 != 0,
            executable: mode & 0o100 != 0,
            mode,
        }
    }
}

/// A filesystem entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// File name (not full path).
    pub name: String,
    /// Full path.
    pub path: String,
    /// Entry kind.
    pub kind: EntryKind,
    /// Size in bytes.
    pub size: u64,
    /// Last modified timestamp (epoch seconds).
    pub modified: u64,
    /// File extension (empty for dirs).
    pub extension: String,
    /// Whether the file/dir is hidden (starts with dot).
    pub hidden: bool,
    /// Permissions.
    pub permissions: Permissions,
    /// Symlink target, if applicable.
    pub symlink_target: Option<String>,
    /// MIME type (guessed from extension).
    pub mime_type: String,
}

impl FileEntry {
    /// Create a directory entry.
    #[must_use]
    pub fn directory(name: String, path: String, modified: u64) -> Self {
        let hidden = name.starts_with('.');
        Self {
            name, path, kind: EntryKind::Directory, size: 0,
            modified, extension: String::new(), hidden,
            permissions: Permissions::from_mode(0o755),
            symlink_target: None, mime_type: "inode/directory".into(),
        }
    }

    /// Create a file entry.
    #[must_use]
    pub fn file(name: String, path: String, size: u64, modified: u64) -> Self {
        let hidden = name.starts_with('.');
        let extension = name.rsplit('.').next()
            .filter(|e| *e != name.as_str())
            .unwrap_or("")
            .to_string();
        let mime_type = guess_mime(&extension);
        Self {
            name, path, kind: EntryKind::File, size,
            modified, extension, hidden,
            permissions: Permissions::from_mode(0o644),
            symlink_target: None, mime_type,
        }
    }

    /// Whether this entry is a directory.
    #[must_use]
    pub fn is_dir(&self) -> bool { self.kind == EntryKind::Directory }

    /// Human-readable size string.
    #[must_use]
    pub fn human_size(&self) -> String {
        if self.kind == EntryKind::Directory {
            return "--".to_string();
        }
        let size = self.size;
        if size < 1024 { return format!("{size} B"); }
        let kb = size as f64 / 1024.0;
        if kb < 1024.0 { return format!("{kb:.1} KB"); }
        let mb = kb / 1024.0;
        if mb < 1024.0 { return format!("{mb:.1} MB"); }
        let gb = mb / 1024.0;
        format!("{gb:.1} GB")
    }
}

/// Guess MIME type from file extension.
#[must_use]
pub fn guess_mime(ext: &str) -> String {
    match ext.to_lowercase().as_str() {
        "txt" | "log" | "md" | "rst" => "text/plain".into(),
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "go" | "java" => "text/x-source".into(),
        "html" | "htm" => "text/html".into(),
        "css" => "text/css".into(),
        "json" => "application/json".into(),
        "xml" => "application/xml".into(),
        "toml" | "yaml" | "yml" | "ini" | "conf" => "text/x-config".into(),
        "png" => "image/png".into(),
        "jpg" | "jpeg" => "image/jpeg".into(),
        "gif" => "image/gif".into(),
        "svg" => "image/svg+xml".into(),
        "mp4" | "mkv" | "avi" => "video/mp4".into(),
        "mp3" | "ogg" | "flac" | "wav" => "audio/mpeg".into(),
        "pdf" => "application/pdf".into(),
        "zip" | "tar" | "gz" | "bz2" | "xz" => "application/x-archive".into(),
        "" => "application/octet-stream".into(),
        _ => format!("application/x-{ext}"),
    }
}
