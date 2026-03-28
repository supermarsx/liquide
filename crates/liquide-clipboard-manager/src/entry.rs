//! Clipboard entry types — a single clipboard item and its content variants.

/// Pixel format for image clipboard content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// PNG-compressed bytes.
    Png,
    /// BMP-formatted bytes.
    Bmp,
    /// Raw 32-bit RGBA pixel data (width * height * 4 bytes).
    Rgba32,
}

/// The payload of a clipboard entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContent {
    /// Plain UTF-8 text.
    Text(String),
    /// Rich text with an HTML representation and a plain-text fallback.
    RichText {
        html: String,
        plain_fallback: String,
    },
    /// Image pixel data.
    Image {
        width: u32,
        height: u32,
        data: Vec<u8>,
        format: ImageFormat,
    },
    /// One or more file/directory paths (copy or cut).
    FilePaths(Vec<String>),
    /// A colour value (e.g. from a colour-picker).
    Color { r: u8, g: u8, b: u8, a: u8 },
    /// Arbitrary data identified by MIME type.
    Custom { mime_type: String, data: Vec<u8> },
}

impl ClipboardContent {
    /// Approximate size in bytes of the content payload.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        match self {
            Self::Text(s) => s.len(),
            Self::RichText {
                html,
                plain_fallback,
            } => html.len() + plain_fallback.len(),
            Self::Image { data, .. } => data.len(),
            Self::FilePaths(paths) => paths.iter().map(|p| p.len()).sum(),
            Self::Color { .. } => 4,
            Self::Custom { mime_type, data } => mime_type.len() + data.len(),
        }
    }

    /// Which high-level category this content belongs to.
    #[must_use]
    pub fn category(&self) -> ContentCategory {
        match self {
            Self::Text(_) | Self::RichText { .. } => ContentCategory::Text,
            Self::Image { .. } => ContentCategory::Images,
            Self::FilePaths(_) => ContentCategory::Files,
            Self::Color { .. } => ContentCategory::Colors,
            Self::Custom { .. } => ContentCategory::Other,
        }
    }

    /// Return a textual representation if possible (for searching).
    #[must_use]
    pub fn as_searchable_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            Self::RichText { plain_fallback, .. } => Some(plain_fallback.as_str()),
            _ => None,
        }
    }

    /// Return file paths if this is a `FilePaths` variant (for search).
    #[must_use]
    pub fn as_file_paths(&self) -> Option<&[String]> {
        match self {
            Self::FilePaths(paths) => Some(paths.as_slice()),
            _ => None,
        }
    }

    /// Return true when two contents are semantically equal (used for dedup).
    #[must_use]
    pub fn content_eq(&self, other: &Self) -> bool {
        self == other
    }
}

/// High-level content categories for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentCategory {
    Text,
    Images,
    Files,
    Colors,
    Other,
}

/// A single item stored in clipboard history.
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    /// Unique identifier (monotonically increasing).
    pub id: u64,
    /// The clipboard payload.
    pub content: ClipboardContent,
    /// Unix timestamp (seconds since epoch) when this entry was created or last
    /// refreshed by a duplicate copy.
    pub timestamp: u64,
    /// Name of the application that produced this entry, if known.
    pub source_app: Option<String>,
    /// Whether this entry is pinned (survives clear operations).
    pub pinned: bool,
    /// Whether this entry contains sensitive data (e.g. from a password
    /// manager) and should be auto-cleared after a timeout.
    pub sensitive: bool,
    /// How many times this entry has been pasted.
    pub times_pasted: u32,
}

impl ClipboardEntry {
    /// Create a new entry with reasonable defaults.
    #[must_use]
    pub fn new(
        id: u64,
        content: ClipboardContent,
        timestamp: u64,
        source_app: Option<String>,
    ) -> Self {
        Self {
            id,
            content,
            timestamp,
            source_app,
            pinned: false,
            sensitive: false,
            times_pasted: 0,
        }
    }

    /// Generate a short text preview of the entry's content, truncated to
    /// `max_len` characters.  Non-text content returns a descriptive label.
    #[must_use]
    pub fn text_preview(&self, max_len: usize) -> String {
        match &self.content {
            ClipboardContent::Text(s) => truncate_preview(s, max_len),
            ClipboardContent::RichText { plain_fallback, .. } => {
                truncate_preview(plain_fallback, max_len)
            }
            ClipboardContent::Image {
                width, height, format, ..
            } => {
                let label = match format {
                    ImageFormat::Png => "PNG",
                    ImageFormat::Bmp => "BMP",
                    ImageFormat::Rgba32 => "RGBA",
                };
                format!("[{label} image {width}\u{00d7}{height}]")
            }
            ClipboardContent::FilePaths(paths) => {
                if paths.is_empty() {
                    "[no files]".to_string()
                } else if paths.len() == 1 {
                    truncate_preview(&paths[0], max_len)
                } else {
                    let first = truncate_preview(&paths[0], max_len.saturating_sub(12));
                    format!("{first} (+{} more)", paths.len() - 1)
                }
            }
            ClipboardContent::Color { r, g, b, a } => {
                format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
            }
            ClipboardContent::Custom { mime_type, data } => {
                format!("[{mime_type}, {} bytes]", data.len())
            }
        }
    }

    /// A human-readable label for the content type (e.g. "Text", "Image",
    /// "Rich Text", "Files", "Color", or the MIME type for custom data).
    #[must_use]
    pub fn content_type_label(&self) -> &str {
        match &self.content {
            ClipboardContent::Text(_) => "Text",
            ClipboardContent::RichText { .. } => "Rich Text",
            ClipboardContent::Image { .. } => "Image",
            ClipboardContent::FilePaths(_) => "Files",
            ClipboardContent::Color { .. } => "Color",
            ClipboardContent::Custom { mime_type, .. } => mime_type.as_str(),
        }
    }
}

/// Truncate a string to at most `max_len` characters, appending "\u{2026}"
/// (ellipsis) if truncated.  Newlines are replaced with "\u{21b5}" (return
/// symbol) for single-line display.
fn truncate_preview(s: &str, max_len: usize) -> String {
    let sanitised: String = s.chars().map(|c| if c == '\n' || c == '\r' { '\u{21b5}' } else { c }).collect();
    if sanitised.chars().count() <= max_len {
        sanitised
    } else {
        let mut out: String = sanitised.chars().take(max_len.saturating_sub(1)).collect();
        out.push('\u{2026}');
        out
    }
}
