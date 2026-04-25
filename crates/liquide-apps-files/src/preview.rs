//! File preview generation.

use crate::entry::{EntryKind, FileEntry};

/// Preview content for a file.
#[derive(Debug, Clone)]
pub enum PreviewContent {
    /// Text preview (first N lines).
    Text {
        lines: Vec<String>,
        truncated: bool,
        total_lines: usize,
    },
    /// Image metadata (dimensions, format).
    Image {
        width: u32,
        height: u32,
        format: String,
    },
    /// Directory summary.
    Directory {
        file_count: usize,
        dir_count: usize,
        total_size: u64,
    },
    /// Binary file (just show size + type).
    Binary { size: u64, mime_type: String },
    /// No preview available.
    None,
}

/// A file preview.
#[derive(Debug, Clone)]
pub struct Preview {
    /// Entry being previewed.
    pub path: String,
    /// Preview content.
    pub content: PreviewContent,
}

impl Preview {
    /// Create a preview for a file entry.
    #[must_use]
    pub fn for_entry(entry: &FileEntry, max_lines: usize) -> Self {
        let content = match entry.kind {
            EntryKind::Directory => PreviewContent::Directory {
                file_count: 0,
                dir_count: 0,
                total_size: 0,
            },
            EntryKind::File if is_text_mime(&entry.mime_type) => PreviewContent::Text {
                lines: Vec::new(),
                truncated: false,
                total_lines: 0,
            },
            EntryKind::File if entry.mime_type.starts_with("image/") => PreviewContent::Image {
                width: 0,
                height: 0,
                format: entry.extension.clone(),
            },
            _ => PreviewContent::Binary {
                size: entry.size,
                mime_type: entry.mime_type.clone(),
            },
        };
        let _ = max_lines;
        Self {
            path: entry.path.clone(),
            content,
        }
    }

    /// Create a text preview with actual content.
    #[must_use]
    pub fn text(path: String, text: &str, max_lines: usize) -> Self {
        let all_lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let total = all_lines.len();
        let truncated = total > max_lines;
        let lines = all_lines.into_iter().take(max_lines).collect();
        Self {
            path,
            content: PreviewContent::Text {
                lines,
                truncated,
                total_lines: total,
            },
        }
    }

    /// Create a directory summary preview.
    #[must_use]
    pub fn directory_summary(
        path: String,
        file_count: usize,
        dir_count: usize,
        total_size: u64,
    ) -> Self {
        Self {
            path,
            content: PreviewContent::Directory {
                file_count,
                dir_count,
                total_size,
            },
        }
    }

    /// Whether a preview is available.
    #[must_use]
    pub fn has_content(&self) -> bool {
        !matches!(self.content, PreviewContent::None)
    }
}

/// Check if a MIME type is text-like.
#[must_use]
pub fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/") || mime == "application/json" || mime == "application/xml"
}
