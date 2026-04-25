//! File properties and metadata panel.

use serde::{Deserialize, Serialize};

/// Detailed file properties for the metadata/info panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProperties {
    /// Full path.
    pub path: String,
    /// File name.
    pub name: String,
    /// File extension (empty for directories).
    pub extension: String,
    /// Size in bytes.
    pub size: u64,
    /// Creation timestamp (epoch seconds), 0 if unavailable.
    pub created: u64,
    /// Last modified timestamp (epoch seconds).
    pub modified: u64,
    /// Last accessed timestamp (epoch seconds), 0 if unavailable.
    pub accessed: u64,
    /// Whether the file is hidden.
    pub is_hidden: bool,
    /// Whether the file is read-only.
    pub is_readonly: bool,
    /// Unix permission string (e.g. "rwxr-xr-x").
    pub permissions: String,
    /// MIME type.
    pub mime_type: String,
}

impl FileProperties {
    /// Create properties from a `FileEntry`.
    #[must_use]
    pub fn from_entry(entry: &crate::entry::FileEntry) -> Self {
        Self {
            path: entry.path.clone(),
            name: entry.name.clone(),
            extension: entry.extension.clone(),
            size: entry.size,
            created: 0,
            modified: entry.modified,
            accessed: 0,
            is_hidden: entry.hidden,
            is_readonly: !entry.permissions.writable,
            permissions: format_permissions(entry.permissions.mode),
            mime_type: entry.mime_type.clone(),
        }
    }

    /// Human-readable size string.
    #[must_use]
    pub fn formatted_size(&self) -> String {
        format_size(self.size)
    }
}

/// Format a byte count into a human-readable string (KB, MB, GB, TB).
#[must_use]
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{:.1} KB", kb);
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{:.1} MB", mb);
    }
    let gb = mb / 1024.0;
    if gb < 1024.0 {
        return format!("{:.1} GB", gb);
    }
    let tb = gb / 1024.0;
    format!("{:.1} TB", tb)
}

/// Format unix permission mode bits into an "rwxrwxrwx" string.
#[must_use]
pub fn format_permissions(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    for (bit, ch) in bits {
        if mode & bit != 0 {
            s.push(ch);
        } else {
            s.push('-');
        }
    }
    s
}

/// Detect MIME type from a file path extension.
///
/// Supports ~50 common file types with extension-based detection.
#[must_use]
pub fn detect_mime_type(path: &str) -> String {
    let ext = path.rsplit('.').next().filter(|e| *e != path).unwrap_or("");
    match ext.to_lowercase().as_str() {
        // Text
        "txt" | "text" => "text/plain".into(),
        "log" => "text/plain".into(),
        "md" | "markdown" => "text/markdown".into(),
        "rst" => "text/x-rst".into(),
        "csv" => "text/csv".into(),
        "tsv" => "text/tab-separated-values".into(),
        "rtf" => "text/rtf".into(),

        // Source code
        "rs" => "text/x-rust".into(),
        "py" => "text/x-python".into(),
        "js" => "text/javascript".into(),
        "ts" => "text/typescript".into(),
        "jsx" => "text/jsx".into(),
        "tsx" => "text/tsx".into(),
        "c" => "text/x-c".into(),
        "cpp" | "cxx" | "cc" => "text/x-c++".into(),
        "h" | "hpp" => "text/x-c-header".into(),
        "go" => "text/x-go".into(),
        "java" => "text/x-java".into(),
        "rb" => "text/x-ruby".into(),
        "php" => "text/x-php".into(),
        "swift" => "text/x-swift".into(),
        "kt" | "kts" => "text/x-kotlin".into(),
        "sh" | "bash" | "zsh" => "text/x-shellscript".into(),
        "sql" => "text/x-sql".into(),

        // Web
        "html" | "htm" => "text/html".into(),
        "css" => "text/css".into(),
        "xml" => "application/xml".into(),
        "json" => "application/json".into(),
        "yaml" | "yml" => "application/x-yaml".into(),
        "toml" => "application/toml".into(),
        "ini" | "conf" | "cfg" => "text/x-config".into(),

        // Images
        "png" => "image/png".into(),
        "jpg" | "jpeg" => "image/jpeg".into(),
        "gif" => "image/gif".into(),
        "bmp" => "image/bmp".into(),
        "svg" => "image/svg+xml".into(),
        "webp" => "image/webp".into(),
        "ico" => "image/x-icon".into(),
        "tiff" | "tif" => "image/tiff".into(),

        // Audio
        "mp3" => "audio/mpeg".into(),
        "ogg" | "oga" => "audio/ogg".into(),
        "flac" => "audio/flac".into(),
        "wav" => "audio/wav".into(),
        "aac" => "audio/aac".into(),
        "m4a" => "audio/mp4".into(),
        "wma" => "audio/x-ms-wma".into(),

        // Video
        "mp4" | "m4v" => "video/mp4".into(),
        "mkv" => "video/x-matroska".into(),
        "avi" => "video/x-msvideo".into(),
        "mov" => "video/quicktime".into(),
        "wmv" => "video/x-ms-wmv".into(),
        "flv" => "video/x-flv".into(),
        "webm" => "video/webm".into(),

        // Documents
        "pdf" => "application/pdf".into(),
        "doc" => "application/msword".into(),
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
        "xls" => "application/vnd.ms-excel".into(),
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
        "ppt" => "application/vnd.ms-powerpoint".into(),
        "pptx" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".into()
        }
        "odt" => "application/vnd.oasis.opendocument.text".into(),

        // Archives
        "zip" => "application/zip".into(),
        "tar" => "application/x-tar".into(),
        "gz" | "gzip" => "application/gzip".into(),
        "bz2" => "application/x-bzip2".into(),
        "xz" => "application/x-xz".into(),
        "7z" => "application/x-7z-compressed".into(),
        "rar" => "application/x-rar-compressed".into(),

        // Executables / binaries
        "exe" => "application/x-msdownload".into(),
        "dll" | "so" | "dylib" => "application/x-sharedlib".into(),
        "wasm" => "application/wasm".into(),

        // Fonts
        "ttf" => "font/ttf".into(),
        "otf" => "font/otf".into(),
        "woff" => "font/woff".into(),
        "woff2" => "font/woff2".into(),

        "" => "application/octet-stream".into(),
        other => format!("application/x-{}", other),
    }
}
