//! Data transfer payloads for drag-and-drop.
//!
//! Data is typed by MIME type and carried as raw bytes. Common helper
//! constructors for text and URI data are provided.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Well-known MIME types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MimeType(pub String);

impl MimeType {
    pub const TEXT_PLAIN: &str = "text/plain";
    pub const TEXT_HTML: &str = "text/html";
    pub const TEXT_URI_LIST: &str = "text/uri-list";
    pub const IMAGE_PNG: &str = "image/png";
    pub const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";

    #[must_use]
    pub fn new(mime: impl Into<String>) -> Self {
        Self(mime.into())
    }

    #[must_use]
    pub fn text_plain() -> Self {
        Self(Self::TEXT_PLAIN.to_string())
    }

    #[must_use]
    pub fn text_html() -> Self {
        Self(Self::TEXT_HTML.to_string())
    }

    #[must_use]
    pub fn text_uri_list() -> Self {
        Self(Self::TEXT_URI_LIST.to_string())
    }
}

/// A single data payload.
#[derive(Debug, Clone)]
pub struct DataPayload {
    pub mime_type: MimeType,
    pub data: Vec<u8>,
}

impl DataPayload {
    #[must_use]
    pub fn new(mime_type: MimeType, data: Vec<u8>) -> Self {
        Self { mime_type, data }
    }

    /// Create a text/plain payload.
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self {
            mime_type: MimeType::text_plain(),
            data: text.as_bytes().to_vec(),
        }
    }

    /// Create a text/uri-list payload.
    #[must_use]
    pub fn uris(uris: &[&str]) -> Self {
        let text = uris.join("\r\n");
        Self {
            mime_type: MimeType::text_uri_list(),
            data: text.into_bytes(),
        }
    }

    /// Attempt to decode as UTF-8 text.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }
}

/// A collection of data payloads offered during a drag or clipboard operation.
///
/// Multiple representations of the same content can be offered (e.g., plain
/// text + HTML), letting the drop target pick the best format.
#[derive(Debug, Clone)]
pub struct DataTransfer {
    payloads: HashMap<String, DataPayload>,
}

impl DataTransfer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            payloads: HashMap::new(),
        }
    }

    /// Add a payload.
    pub fn add(&mut self, payload: DataPayload) {
        self.payloads
            .insert(payload.mime_type.0.clone(), payload);
    }

    /// Get a payload by MIME type.
    #[must_use]
    pub fn get(&self, mime: &str) -> Option<&DataPayload> {
        self.payloads.get(mime)
    }

    /// Get the text/plain content if available.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.get(MimeType::TEXT_PLAIN).and_then(|p| p.as_text())
    }

    /// Check if a MIME type is available.
    #[must_use]
    pub fn has(&self, mime: &str) -> bool {
        self.payloads.contains_key(mime)
    }

    /// List available MIME types.
    #[must_use]
    pub fn available_types(&self) -> Vec<&str> {
        self.payloads.keys().map(|s| s.as_str()).collect()
    }

    /// Number of payloads.
    #[must_use]
    pub fn len(&self) -> usize {
        self.payloads.len()
    }

    /// Whether empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }
}

impl Default for DataTransfer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_payload_text() {
        let p = DataPayload::text("hello");
        assert_eq!(p.as_text(), Some("hello"));
        assert_eq!(p.mime_type.0, MimeType::TEXT_PLAIN);
    }

    #[test]
    fn test_data_transfer() {
        let mut dt = DataTransfer::new();
        dt.add(DataPayload::text("hello"));
        dt.add(DataPayload::new(
            MimeType::text_html(),
            b"<b>hello</b>".to_vec(),
        ));

        assert_eq!(dt.len(), 2);
        assert_eq!(dt.text(), Some("hello"));
        assert!(dt.has(MimeType::TEXT_HTML));
        assert!(!dt.has(MimeType::IMAGE_PNG));
    }

    #[test]
    fn test_uri_payload() {
        let p = DataPayload::uris(&["file:///a.txt", "file:///b.txt"]);
        assert_eq!(p.as_text(), Some("file:///a.txt\r\nfile:///b.txt"));
    }
}
