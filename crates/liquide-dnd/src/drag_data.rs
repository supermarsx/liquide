//! Multi-format drag payload types.
//!
//! [`DragData`] holds a list of [`DragFormat`] variants, allowing the same
//! content to be offered in multiple representations (e.g., a file path as
//! both `FilePaths` and `Text`). Drop targets pick the best available format.

use serde::{Deserialize, Serialize};

fn is_windows_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'A' + (value - 10)) as char,
        _ => unreachable!(),
    }
}

fn percent_encode_uri_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                encoded.push(*byte as char)
            }
            _ => {
                encoded.push('%');
                encoded.push(hex_digit(byte >> 4));
                encoded.push(hex_digit(byte & 0x0F));
            }
        }
    }
    encoded
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let hi = hex_value(bytes[index + 1])?;
            let lo = hex_value(bytes[index + 2])?;
            decoded.push((hi << 4) | lo);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn strip_file_scheme(uri: &str) -> Option<&str> {
    if uri.len() < 7 || !uri[..7].eq_ignore_ascii_case("file://") {
        return None;
    }
    Some(&uri[7..])
}

fn file_path_to_uri(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with("//") {
        let host_path = normalized.trim_start_matches('/');
        format!("file://{}", percent_encode_uri_path(host_path))
    } else if is_windows_drive_path(&normalized) {
        format!("file:///{}", percent_encode_uri_path(&normalized))
    } else {
        format!("file://{}", percent_encode_uri_path(&normalized))
    }
}

fn file_uri_to_path(uri: &str) -> Option<String> {
    let remainder = strip_file_scheme(uri)?;
    if remainder.is_empty() {
        return None;
    }

    if is_windows_drive_path(remainder) {
        return Some(percent_decode(remainder)?.replace('/', "\\"));
    }

    if remainder.starts_with('/') {
        let decoded = percent_decode(remainder)?;
        if decoded.len() > 2 && decoded.starts_with('/') && is_windows_drive_path(&decoded[1..]) {
            return Some(decoded[1..].replace('/', "\\"));
        }
        return Some(decoded);
    }

    let (host, path_part) = remainder.split_once('/').unwrap_or((remainder, ""));
    if host.eq_ignore_ascii_case("localhost") {
        let decoded = percent_decode(&format!("/{path_part}"))?;
        if decoded.len() > 2 && decoded.starts_with('/') && is_windows_drive_path(&decoded[1..]) {
            return Some(decoded[1..].replace('/', "\\"));
        }
        return Some(decoded);
    }
    if path_part.is_empty() {
        return None;
    }

    let decoded_path = percent_decode(path_part)?;
    Some(format!(r"\\{}\{}", host, decoded_path.replace('/', "\\")))
}

fn parse_file_uri_list(text: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    if lines.is_empty() {
        return None;
    }

    let mut paths = Vec::with_capacity(lines.len());
    for line in lines {
        paths.push(file_uri_to_path(line)?);
    }
    Some(paths)
}

/// A single typed format for drag data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DragFormat {
    /// Plain text content.
    Text(String),
    /// One or more file paths (platform native paths).
    FilePaths(Vec<String>),
    /// A URI (e.g., `https://...`, `file:///...`).
    Uri(String),
    /// Raw image data in RGBA format.
    Image {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    /// Arbitrary data identified by MIME type.
    Custom { mime_type: String, data: Vec<u8> },
}

impl DragFormat {
    /// Returns a human-readable name for this format variant.
    #[must_use]
    pub fn format_name(&self) -> &str {
        match self {
            DragFormat::Text(_) => "text",
            DragFormat::FilePaths(_) => "file-paths",
            DragFormat::Uri(_) => "uri",
            DragFormat::Image { .. } => "image",
            DragFormat::Custom { mime_type, .. } => mime_type.as_str(),
        }
    }

    /// Returns `true` if this is a `Text` variant.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, DragFormat::Text(_))
    }

    /// Returns `true` if this is a `FilePaths` variant.
    #[must_use]
    pub fn is_file_paths(&self) -> bool {
        matches!(self, DragFormat::FilePaths(_))
    }

    /// Returns `true` if this is a `Uri` variant.
    #[must_use]
    pub fn is_uri(&self) -> bool {
        matches!(self, DragFormat::Uri(_))
    }

    /// Returns `true` if this is an `Image` variant.
    #[must_use]
    pub fn is_image(&self) -> bool {
        matches!(self, DragFormat::Image { .. })
    }

    /// Returns `true` if this is a `Custom` variant.
    #[must_use]
    pub fn is_custom(&self) -> bool {
        matches!(self, DragFormat::Custom { .. })
    }
}

/// Multi-format drag payload.
///
/// Holds one or more [`DragFormat`] representations of the same logical
/// content. For example, dragging a file might offer both `FilePaths` and
/// `Text` (the file path as a string), letting the drop target choose the
/// most appropriate format.
#[derive(Debug, Clone)]
pub struct DragData {
    formats: Vec<DragFormat>,
}

impl DragData {
    /// Create an empty `DragData`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            formats: Vec::new(),
        }
    }

    /// Create `DragData` from a single format.
    #[must_use]
    pub fn single(format: DragFormat) -> Self {
        Self {
            formats: vec![format],
        }
    }

    /// Create `DragData` containing plain text.
    #[must_use]
    pub fn text(s: impl Into<String>) -> Self {
        Self::single(DragFormat::Text(s.into()))
    }

    /// Create `DragData` containing file paths.
    #[must_use]
    pub fn file_paths(paths: Vec<String>) -> Self {
        Self::single(DragFormat::FilePaths(paths))
    }

    /// Create `DragData` containing a URI.
    #[must_use]
    pub fn uri(uri: impl Into<String>) -> Self {
        Self::single(DragFormat::Uri(uri.into()))
    }

    /// Add a format to this payload.
    pub fn add_format(&mut self, format: DragFormat) {
        self.formats.push(format);
    }

    /// Returns the first (preferred) format, if any.
    #[must_use]
    pub fn preferred_format(&self) -> Option<&DragFormat> {
        self.formats.first()
    }

    /// Returns all formats.
    #[must_use]
    pub fn formats(&self) -> &[DragFormat] {
        &self.formats
    }

    /// Returns the number of formats offered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.formats.len()
    }

    /// Returns `true` if no formats are offered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.formats.is_empty()
    }

    /// Check whether any format matches the predicate.
    #[must_use]
    pub fn has_format<F: Fn(&DragFormat) -> bool>(&self, predicate: F) -> bool {
        self.formats.iter().any(predicate)
    }

    /// Check whether text content is available.
    #[must_use]
    pub fn has_text(&self) -> bool {
        self.has_format(DragFormat::is_text)
    }

    /// Check whether file paths are available.
    #[must_use]
    pub fn has_file_paths(&self) -> bool {
        self.has_format(DragFormat::is_file_paths)
    }

    /// Check whether a URI is available.
    #[must_use]
    pub fn has_uri(&self) -> bool {
        self.has_format(DragFormat::is_uri)
    }

    /// Check whether image data is available.
    #[must_use]
    pub fn has_image(&self) -> bool {
        self.has_format(DragFormat::is_image)
    }

    /// Get the first text format, if available.
    #[must_use]
    pub fn get_text(&self) -> Option<&str> {
        self.formats.iter().find_map(|f| match f {
            DragFormat::Text(s) => Some(s.as_str()),
            _ => None,
        })
    }

    /// Get the first file paths format, if available.
    #[must_use]
    pub fn get_file_paths(&self) -> Option<&[String]> {
        self.formats.iter().find_map(|f| match f {
            DragFormat::FilePaths(paths) => Some(paths.as_slice()),
            _ => None,
        })
    }

    /// Get the first URI format, if available.
    #[must_use]
    pub fn get_uri(&self) -> Option<&str> {
        self.formats.iter().find_map(|f| match f {
            DragFormat::Uri(u) => Some(u.as_str()),
            _ => None,
        })
    }

    /// Find the first format matching a predicate.
    #[must_use]
    pub fn find_format<F: Fn(&DragFormat) -> bool>(&self, predicate: F) -> Option<&DragFormat> {
        self.formats.iter().find(|f| predicate(f))
    }
}

impl Default for DragData {
    fn default() -> Self {
        Self::new()
    }
}

impl DragFormat {
    /// Convert this format to a MIME type string suitable for protocol negotiation.
    #[must_use]
    pub fn to_mime_type(&self) -> &str {
        match self {
            DragFormat::Text(_) => "text/plain",
            DragFormat::FilePaths(_) => "text/uri-list",
            DragFormat::Uri(_) => "text/uri-list",
            DragFormat::Image { .. } => "image/png",
            DragFormat::Custom { mime_type, .. } => mime_type.as_str(),
        }
    }

    /// Attempt to convert this format to raw bytes suitable for protocol transfer.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            DragFormat::Text(s) => s.as_bytes().to_vec(),
            DragFormat::FilePaths(paths) => {
                let uris: Vec<String> = paths.iter().map(|path| file_path_to_uri(path)).collect();
                uris.join("\r\n").into_bytes()
            }
            DragFormat::Uri(u) => u.as_bytes().to_vec(),
            DragFormat::Image { data, .. } => data.clone(),
            DragFormat::Custom { data, .. } => data.clone(),
        }
    }
}

/// A store for drag data keyed by MIME type, supporting multi-format
/// data offering for protocol-level DnD negotiation.
///
/// This is the bridge between the high-level [`DragData`] API and
/// protocol-level MIME type negotiation (freedesktop XDND, Wayland
/// `wl_data_offer`, etc.).
#[derive(Debug, Clone)]
pub struct DragDataStore {
    /// MIME-type -> raw bytes mapping (insertion-ordered).
    entries: Vec<(String, Vec<u8>)>,
}

impl DragDataStore {
    /// Create an empty data store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Create a data store from a [`DragData`] payload.
    ///
    /// Each format in the payload is converted to its protocol MIME type
    /// and serialized to bytes.
    #[must_use]
    pub fn from_drag_data(data: &DragData) -> Self {
        let mut store = Self::new();
        for fmt in data.formats() {
            let mime = fmt.to_mime_type().to_string();
            let bytes = fmt.to_bytes();
            // Avoid duplicate MIME entries — first wins
            if !store.entries.iter().any(|(m, _)| *m == mime) {
                store.entries.push((mime, bytes));
            }
        }
        store
    }

    /// Add a MIME-type entry with raw data.
    pub fn set(&mut self, mime_type: impl Into<String>, data: Vec<u8>) {
        let mime = mime_type.into();
        // Replace existing or append
        if let Some(entry) = self.entries.iter_mut().find(|(m, _)| *m == mime) {
            entry.1 = data;
        } else {
            self.entries.push((mime, data));
        }
    }

    /// List all offered MIME types (in insertion order = preference order).
    #[must_use]
    pub fn offer_types(&self) -> Vec<String> {
        self.entries.iter().map(|(m, _)| m.clone()).collect()
    }

    /// Get the raw data for a specific MIME type.
    #[must_use]
    pub fn get_data(&self, mime_type: &str) -> Option<DragData> {
        let (_, bytes) = self.entries.iter().find(|(m, _)| m == mime_type)?;

        // Reconstruct a DragData from MIME type + bytes
        let format = match mime_type {
            "text/plain" | "text/plain;charset=utf-8" | "UTF8_STRING" | "STRING" => {
                let text = String::from_utf8_lossy(bytes).into_owned();
                DragFormat::Text(text)
            }
            "text/uri-list" => {
                let text = String::from_utf8_lossy(bytes).into_owned();
                if let Some(paths) = parse_file_uri_list(&text) {
                    DragFormat::FilePaths(paths)
                } else {
                    DragFormat::Uri(text.trim().to_string())
                }
            }
            mime if mime.starts_with("image/") => DragFormat::Image {
                width: 0,
                height: 0,
                data: bytes.clone(),
            },
            _ => DragFormat::Custom {
                mime_type: mime_type.to_string(),
                data: bytes.clone(),
            },
        };

        Some(DragData::single(format))
    }

    /// Given a list of MIME types that a drop target accepts, return the
    /// best matching type from this store's offerings.
    ///
    /// Preference order:
    /// 1. Exact match in store insertion order (source preference)
    /// 2. `None` if no match
    #[must_use]
    pub fn preferred_type(&self, accepted: &[String]) -> Option<String> {
        // Walk our entries in order (source preference), return first match
        for (offered, _) in &self.entries {
            if accepted.iter().any(|a| a == offered) {
                return Some(offered.clone());
            }
        }
        None
    }

    /// Number of MIME type entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove an entry by MIME type. Returns `true` if it existed.
    pub fn remove(&mut self, mime_type: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(m, _)| m != mime_type);
        self.entries.len() < before
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for DragDataStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drag_data_text() {
        let data = DragData::text("hello world");
        assert!(data.has_text());
        assert!(!data.has_file_paths());
        assert_eq!(data.get_text(), Some("hello world"));
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_drag_data_file_paths() {
        let data = DragData::file_paths(vec![
            "/home/user/doc.txt".to_string(),
            "/home/user/pic.png".to_string(),
        ]);
        assert!(data.has_file_paths());
        assert!(!data.has_text());
        let paths = data.get_file_paths().unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "/home/user/doc.txt");
    }

    #[test]
    fn test_drag_data_uri() {
        let data = DragData::uri("https://example.com");
        assert!(data.has_uri());
        assert_eq!(data.get_uri(), Some("https://example.com"));
    }

    #[test]
    fn test_drag_data_multi_format() {
        let mut data = DragData::text("file:///home/user/doc.txt");
        data.add_format(DragFormat::FilePaths(vec![
            "/home/user/doc.txt".to_string(),
        ]));
        data.add_format(DragFormat::Uri("file:///home/user/doc.txt".to_string()));

        assert_eq!(data.len(), 3);
        assert!(data.has_text());
        assert!(data.has_file_paths());
        assert!(data.has_uri());

        // Preferred is the first added (text)
        let preferred = data.preferred_format().unwrap();
        assert!(preferred.is_text());
    }

    #[test]
    fn test_drag_data_image() {
        let data = DragData::single(DragFormat::Image {
            width: 2,
            height: 2,
            data: vec![255; 16], // 2x2 RGBA
        });
        assert!(data.has_image());
        assert!(!data.has_text());
    }

    #[test]
    fn test_drag_data_custom() {
        let data = DragData::single(DragFormat::Custom {
            mime_type: "application/x-liquide-widget".to_string(),
            data: vec![1, 2, 3, 4],
        });
        assert!(!data.has_text());
        let fmt = data.preferred_format().unwrap();
        assert!(fmt.is_custom());
        assert_eq!(fmt.format_name(), "application/x-liquide-widget");
    }

    #[test]
    fn test_drag_data_empty() {
        let data = DragData::new();
        assert!(data.is_empty());
        assert_eq!(data.preferred_format(), None);
        assert!(!data.has_text());
    }

    #[test]
    fn test_drag_format_names() {
        assert_eq!(DragFormat::Text(String::new()).format_name(), "text");
        assert_eq!(DragFormat::FilePaths(vec![]).format_name(), "file-paths");
        assert_eq!(DragFormat::Uri(String::new()).format_name(), "uri");
        assert_eq!(
            DragFormat::Image {
                width: 0,
                height: 0,
                data: vec![]
            }
            .format_name(),
            "image"
        );
    }

    #[test]
    fn test_find_format() {
        let mut data = DragData::text("hello");
        data.add_format(DragFormat::Uri("https://example.com".to_string()));

        let uri = data.find_format(DragFormat::is_uri).unwrap();
        match uri {
            DragFormat::Uri(u) => assert_eq!(u, "https://example.com"),
            _ => panic!("expected Uri"),
        }
    }

    // ---- DragFormat MIME/bytes conversion tests ----

    #[test]
    fn test_format_to_mime_type() {
        assert_eq!(DragFormat::Text("hi".into()).to_mime_type(), "text/plain");
        assert_eq!(
            DragFormat::FilePaths(vec!["/a".into()]).to_mime_type(),
            "text/uri-list"
        );
        assert_eq!(
            DragFormat::Uri("https://x".into()).to_mime_type(),
            "text/uri-list"
        );
        assert_eq!(
            DragFormat::Image {
                width: 1,
                height: 1,
                data: vec![0; 4]
            }
            .to_mime_type(),
            "image/png"
        );
        assert_eq!(
            DragFormat::Custom {
                mime_type: "application/json".into(),
                data: vec![]
            }
            .to_mime_type(),
            "application/json"
        );
    }

    #[test]
    fn test_format_to_bytes_text() {
        let bytes = DragFormat::Text("hello".into()).to_bytes();
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn test_format_to_bytes_file_paths() {
        let bytes =
            DragFormat::FilePaths(vec!["/home/My File #1.txt".into(), "/home/b.txt".into()])
                .to_bytes();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("file:///home/My%20File%20%231.txt"));
        assert!(text.contains("file:///home/b.txt"));
    }

    #[test]
    fn test_format_to_bytes_file_paths_windows_drive_format() {
        let bytes =
            DragFormat::FilePaths(vec![r"C:\Users\Alice Smith\notes #1.txt".into()]).to_bytes();
        let text = String::from_utf8(bytes).unwrap();

        assert_eq!(text, "file:///C:/Users/Alice%20Smith/notes%20%231.txt");
    }

    // ---- DragDataStore tests ----

    #[test]
    fn test_store_from_text_drag_data() {
        let data = DragData::text("hello world");
        let store = DragDataStore::from_drag_data(&data);
        assert_eq!(store.len(), 1);
        let types = store.offer_types();
        assert_eq!(types, vec!["text/plain"]);
    }

    #[test]
    fn test_store_from_multi_format() {
        let mut data = DragData::text("/home/user/doc.txt");
        data.add_format(DragFormat::FilePaths(vec!["/home/user/doc.txt".into()]));
        let store = DragDataStore::from_drag_data(&data);
        assert_eq!(store.len(), 2);
        let types = store.offer_types();
        assert!(types.contains(&"text/plain".to_string()));
        assert!(types.contains(&"text/uri-list".to_string()));
    }

    #[test]
    fn test_store_dedup_mime_types() {
        // Uri and FilePaths both map to text/uri-list — first wins
        let mut data = DragData::uri("file:///a.txt");
        data.add_format(DragFormat::FilePaths(vec!["/a.txt".into()]));
        let store = DragDataStore::from_drag_data(&data);
        // Should have only 1 text/uri-list entry (the Uri one, added first)
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_store_get_data_text() {
        let store = DragDataStore::from_drag_data(&DragData::text("hello"));
        let result = store.get_data("text/plain").unwrap();
        assert_eq!(result.get_text(), Some("hello"));
    }

    #[test]
    fn test_store_get_data_file_paths() {
        let data = DragData::file_paths(vec!["/home/doc.txt".into()]);
        let store = DragDataStore::from_drag_data(&data);
        let result = store.get_data("text/uri-list").unwrap();
        assert!(result.has_file_paths());
        let paths = result.get_file_paths().unwrap();
        assert_eq!(paths[0], "/home/doc.txt");
    }

    #[test]
    fn test_store_get_data_missing() {
        let store = DragDataStore::from_drag_data(&DragData::text("hi"));
        assert!(store.get_data("image/png").is_none());
    }

    #[test]
    fn test_store_get_data_custom() {
        let mut store = DragDataStore::new();
        store.set("application/x-test", vec![1, 2, 3]);
        let result = store.get_data("application/x-test").unwrap();
        let fmt = result.preferred_format().unwrap();
        assert!(fmt.is_custom());
    }

    #[test]
    fn test_store_preferred_type() {
        let data = DragData::text("hello");
        let store = DragDataStore::from_drag_data(&data);
        let pref = store.preferred_type(&["image/png".to_string(), "text/plain".to_string()]);
        assert_eq!(pref, Some("text/plain".to_string()));
    }

    #[test]
    fn test_store_preferred_type_no_match() {
        let store = DragDataStore::from_drag_data(&DragData::text("hi"));
        let pref = store.preferred_type(&["image/png".to_string()]);
        assert!(pref.is_none());
    }

    #[test]
    fn test_store_preferred_type_source_order() {
        // Store has text/plain first, then text/uri-list
        let mut data = DragData::text("hello");
        data.add_format(DragFormat::FilePaths(vec!["/a".into()]));
        let store = DragDataStore::from_drag_data(&data);

        // Both accepted — source preference wins (text/plain first)
        let pref = store.preferred_type(&["text/uri-list".to_string(), "text/plain".to_string()]);
        assert_eq!(pref, Some("text/plain".to_string()));
    }

    #[test]
    fn test_store_set_and_remove() {
        let mut store = DragDataStore::new();
        store.set("text/plain", b"hello".to_vec());
        assert_eq!(store.len(), 1);
        assert!(store.remove("text/plain"));
        assert_eq!(store.len(), 0);
        assert!(!store.remove("text/plain")); // already removed
    }

    #[test]
    fn test_store_set_replaces() {
        let mut store = DragDataStore::new();
        store.set("text/plain", b"v1".to_vec());
        store.set("text/plain", b"v2".to_vec());
        assert_eq!(store.len(), 1);
        let result = store.get_data("text/plain").unwrap();
        assert_eq!(result.get_text(), Some("v2"));
    }

    #[test]
    fn test_store_clear() {
        let mut store = DragDataStore::from_drag_data(&DragData::text("hi"));
        assert!(!store.is_empty());
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_store_get_data_image_mime() {
        let mut store = DragDataStore::new();
        store.set("image/jpeg", vec![0xFF, 0xD8]);
        let result = store.get_data("image/jpeg").unwrap();
        assert!(result.has_image());
    }

    #[test]
    fn test_store_uri_list_with_comments() {
        let mut store = DragDataStore::new();
        let uri_list = "# comment\r\nfile:///home/a.txt\r\nfile:///home/b.txt\r\n";
        store.set("text/uri-list", uri_list.as_bytes().to_vec());
        let result = store.get_data("text/uri-list").unwrap();
        let paths = result.get_file_paths().unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "/home/a.txt");
        assert_eq!(paths[1], "/home/b.txt");
    }

    #[test]
    fn test_store_uri_list_decodes_percent_encoded_paths() {
        let mut store = DragDataStore::new();
        let uri_list = "file:///home/My%20Folder/%E2%9C%93.txt\r\n";
        store.set("text/uri-list", uri_list.as_bytes().to_vec());

        let result = store.get_data("text/uri-list").unwrap();
        let paths = result.get_file_paths().unwrap();

        assert_eq!(paths, &["/home/My Folder/✓.txt".to_string()]);
    }

    #[test]
    fn test_store_uri_list_parses_windows_drive_paths() {
        let mut store = DragDataStore::new();
        let uri_list = "file:///C:/Users/Alice%20Smith/notes%20%231.txt\r\n";
        store.set("text/uri-list", uri_list.as_bytes().to_vec());

        let result = store.get_data("text/uri-list").unwrap();
        let paths = result.get_file_paths().unwrap();

        assert_eq!(paths, &[r"C:\Users\Alice Smith\notes #1.txt".to_string()]);
    }

    #[test]
    fn test_store_uri_list_parses_windows_unc_paths() {
        let mut store = DragDataStore::new();
        let uri_list = "file://server/share/My%20File.txt\r\n";
        store.set("text/uri-list", uri_list.as_bytes().to_vec());

        let result = store.get_data("text/uri-list").unwrap();
        let paths = result.get_file_paths().unwrap();

        assert_eq!(paths, &[r"\\server\share\My File.txt".to_string()]);
    }
}
