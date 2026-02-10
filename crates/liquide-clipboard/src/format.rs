//! Clipboard data formats and MIME type mapping.

use serde::{Deserialize, Serialize};

/// Well-known MIME type constants.
pub const MIME_PLAIN_TEXT: &str = "text/plain;charset=utf-8";
pub const MIME_HTML: &str = "text/html";
pub const MIME_RICH_TEXT: &str = "text/richtext";
pub const MIME_PNG: &str = "image/png";
pub const MIME_JPEG: &str = "image/jpeg";
pub const MIME_SVG: &str = "image/svg+xml";
pub const MIME_FILE_URI_LIST: &str = "text/uri-list";

/// Clipboard data format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClipboardFormat {
    PlainText,
    Html,
    RichText,
    Png,
    Jpeg,
    Svg,
    FileUriList,
    Custom(String),
}

impl ClipboardFormat {
    /// Get the MIME type string for this format.
    #[must_use]
    pub fn mime_type(&self) -> &str {
        match self {
            Self::PlainText => MIME_PLAIN_TEXT,
            Self::Html => MIME_HTML,
            Self::RichText => MIME_RICH_TEXT,
            Self::Png => MIME_PNG,
            Self::Jpeg => MIME_JPEG,
            Self::Svg => MIME_SVG,
            Self::FileUriList => MIME_FILE_URI_LIST,
            Self::Custom(mime) => mime,
        }
    }

    /// Try to create a format from a MIME type string.
    #[must_use]
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            MIME_PLAIN_TEXT | "text/plain" => Some(Self::PlainText),
            MIME_HTML => Some(Self::Html),
            MIME_RICH_TEXT => Some(Self::RichText),
            MIME_PNG => Some(Self::Png),
            MIME_JPEG => Some(Self::Jpeg),
            MIME_SVG => Some(Self::Svg),
            MIME_FILE_URI_LIST => Some(Self::FileUriList),
            _ => None,
        }
    }

    /// Check if this format is text-based.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, Self::PlainText | Self::Html | Self::RichText | Self::FileUriList)
    }

    /// Check if this format is an image.
    #[must_use]
    pub fn is_image(&self) -> bool {
        matches!(self, Self::Png | Self::Jpeg | Self::Svg)
    }
}

impl std::fmt::Display for ClipboardFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlainText => write!(f, "PlainText"),
            Self::Html => write!(f, "HTML"),
            Self::RichText => write!(f, "RichText"),
            Self::Png => write!(f, "PNG"),
            Self::Jpeg => write!(f, "JPEG"),
            Self::Svg => write!(f, "SVG"),
            Self::FileUriList => write!(f, "FileUriList"),
            Self::Custom(mime) => write!(f, "Custom({mime})"),
        }
    }
}
