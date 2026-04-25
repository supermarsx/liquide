//! MIME type handling.
//!
//! Provides a `MimeType` value object, a `MimeDatabase` with built-in
//! mappings for common file extensions, and magic-byte-based content
//! sniffing following the freedesktop.org Shared MIME-info specification
//! concepts.

use std::collections::HashMap;
use std::fmt;

/// A MIME media type consisting of a top-level type and a subtype.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MimeType {
    /// The top-level type (e.g. "text", "image", "application").
    pub type_: String,
    /// The subtype (e.g. "plain", "png", "pdf").
    pub subtype: String,
}

impl MimeType {
    /// Create a new `MimeType`.
    pub fn new(type_: &str, subtype: &str) -> Self {
        MimeType {
            type_: type_.to_string(),
            subtype: subtype.to_string(),
        }
    }

    /// Return the full MIME type string (e.g. `"text/plain"`).
    pub fn essence(&self) -> String {
        format!("{}/{}", self.type_, self.subtype)
    }

    /// Parse a MIME string of the form `"type/subtype"`.
    pub fn parse(s: &str) -> Option<Self> {
        let (type_, subtype) = s.split_once('/')?;
        if type_.is_empty() || subtype.is_empty() {
            return None;
        }
        Some(MimeType::new(type_.trim(), subtype.trim()))
    }

    /// Check whether this is a text type.
    pub fn is_text(&self) -> bool {
        self.type_ == "text"
    }

    /// Check whether this is an image type.
    pub fn is_image(&self) -> bool {
        self.type_ == "image"
    }

    /// Check whether this is an audio type.
    pub fn is_audio(&self) -> bool {
        self.type_ == "audio"
    }

    /// Check whether this is a video type.
    pub fn is_video(&self) -> bool {
        self.type_ == "video"
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.type_, self.subtype)
    }
}

/// An association between a MIME type and a default application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MimeAssociation {
    /// The MIME type.
    pub mime_type: MimeType,
    /// Desktop entry ID of the associated application (e.g. `"firefox.desktop"`).
    pub desktop_entry_id: String,
}

/// Database of MIME type mappings.
///
/// Includes built-in mappings for ~50 common file extensions and supports
/// magic-byte-based content detection.
pub struct MimeDatabase {
    /// Extension -> MimeType mappings (extension without leading dot, lowercase).
    ext_map: HashMap<String, MimeType>,
    /// Default application associations keyed by MIME essence.
    associations: HashMap<String, String>,
}

// We use a function to build the magic byte table since const String is not
// available in static context.
fn magic_table() -> Vec<(usize, &'static [u8], MimeType)> {
    vec![
        // Images
        (0, b"\x89PNG\r\n\x1a\n", MimeType::new("image", "png")),
        (0, b"\xff\xd8\xff", MimeType::new("image", "jpeg")),
        (0, b"GIF87a", MimeType::new("image", "gif")),
        (0, b"GIF89a", MimeType::new("image", "gif")),
        (0, b"BM", MimeType::new("image", "bmp")),
        (0, b"RIFF", MimeType::new("image", "webp")), // RIFF....WEBP (checked further below)
        (0, b"\x00\x00\x01\x00", MimeType::new("image", "x-icon")),
        // Documents
        (0, b"%PDF", MimeType::new("application", "pdf")),
        (0, b"PK\x03\x04", MimeType::new("application", "zip")),
        (0, b"\x1f\x8b", MimeType::new("application", "gzip")),
        (
            0,
            b"Rar!\x1a\x07",
            MimeType::new("application", "x-rar-compressed"),
        ),
        (0, b"\xfd7zXZ\x00", MimeType::new("application", "x-xz")),
        (
            0,
            b"7z\xbc\xaf\x27\x1c",
            MimeType::new("application", "x-7z-compressed"),
        ),
        (0, b"\x7fELF", MimeType::new("application", "x-executable")),
        // Audio
        (0, b"ID3", MimeType::new("audio", "mpeg")),
        (0, b"fLaC", MimeType::new("audio", "flac")),
        (0, b"OggS", MimeType::new("audio", "ogg")),
        // Video
        (4, b"ftyp", MimeType::new("video", "mp4")),
        // Text (BOM markers)
        (0, b"\xef\xbb\xbf", MimeType::new("text", "plain")),
        (0, b"\xff\xfe", MimeType::new("text", "plain")),
        (0, b"\xfe\xff", MimeType::new("text", "plain")),
        // XML
        (0, b"<?xml", MimeType::new("application", "xml")),
        // WASM
        (0, b"\x00asm", MimeType::new("application", "wasm")),
    ]
}

impl MimeDatabase {
    /// Create a new database populated with built-in extension mappings.
    pub fn new() -> Self {
        let mut ext_map = HashMap::with_capacity(64);

        // Text
        ins(&mut ext_map, "txt", "text", "plain");
        ins(&mut ext_map, "html", "text", "html");
        ins(&mut ext_map, "htm", "text", "html");
        ins(&mut ext_map, "css", "text", "css");
        ins(&mut ext_map, "js", "text", "javascript");
        ins(&mut ext_map, "mjs", "text", "javascript");
        ins(&mut ext_map, "json", "application", "json");
        ins(&mut ext_map, "xml", "application", "xml");
        ins(&mut ext_map, "csv", "text", "csv");
        ins(&mut ext_map, "tsv", "text", "tab-separated-values");
        ins(&mut ext_map, "md", "text", "markdown");
        ins(&mut ext_map, "yaml", "text", "yaml");
        ins(&mut ext_map, "yml", "text", "yaml");
        ins(&mut ext_map, "toml", "text", "toml");
        ins(&mut ext_map, "ini", "text", "plain");
        ins(&mut ext_map, "log", "text", "plain");
        ins(&mut ext_map, "conf", "text", "plain");

        // Programming languages
        ins(&mut ext_map, "py", "text", "x-python");
        ins(&mut ext_map, "rs", "text", "x-rust");
        ins(&mut ext_map, "c", "text", "x-csrc");
        ins(&mut ext_map, "h", "text", "x-chdr");
        ins(&mut ext_map, "cpp", "text", "x-c++src");
        ins(&mut ext_map, "hpp", "text", "x-c++hdr");
        ins(&mut ext_map, "java", "text", "x-java");
        ins(&mut ext_map, "go", "text", "x-go");
        ins(&mut ext_map, "rb", "text", "x-ruby");
        ins(&mut ext_map, "sh", "text", "x-shellscript");
        ins(&mut ext_map, "pl", "text", "x-perl");
        ins(&mut ext_map, "ts", "text", "typescript");
        ins(&mut ext_map, "tsx", "text", "typescript");
        ins(&mut ext_map, "jsx", "text", "javascript");
        ins(&mut ext_map, "lua", "text", "x-lua");

        // Images
        ins(&mut ext_map, "png", "image", "png");
        ins(&mut ext_map, "jpg", "image", "jpeg");
        ins(&mut ext_map, "jpeg", "image", "jpeg");
        ins(&mut ext_map, "gif", "image", "gif");
        ins(&mut ext_map, "bmp", "image", "bmp");
        ins(&mut ext_map, "svg", "image", "svg+xml");
        ins(&mut ext_map, "webp", "image", "webp");
        ins(&mut ext_map, "ico", "image", "x-icon");
        ins(&mut ext_map, "tiff", "image", "tiff");
        ins(&mut ext_map, "tif", "image", "tiff");

        // Audio
        ins(&mut ext_map, "mp3", "audio", "mpeg");
        ins(&mut ext_map, "wav", "audio", "wav");
        ins(&mut ext_map, "ogg", "audio", "ogg");
        ins(&mut ext_map, "flac", "audio", "flac");
        ins(&mut ext_map, "m4a", "audio", "mp4");
        ins(&mut ext_map, "aac", "audio", "aac");
        ins(&mut ext_map, "opus", "audio", "opus");
        ins(&mut ext_map, "wma", "audio", "x-ms-wma");

        // Video
        ins(&mut ext_map, "mp4", "video", "mp4");
        ins(&mut ext_map, "mkv", "video", "x-matroska");
        ins(&mut ext_map, "avi", "video", "x-msvideo");
        ins(&mut ext_map, "webm", "video", "webm");
        ins(&mut ext_map, "mov", "video", "quicktime");
        ins(&mut ext_map, "wmv", "video", "x-ms-wmv");
        ins(&mut ext_map, "flv", "video", "x-flv");

        // Archives & documents
        ins(&mut ext_map, "pdf", "application", "pdf");
        ins(&mut ext_map, "zip", "application", "zip");
        ins(&mut ext_map, "gz", "application", "gzip");
        ins(&mut ext_map, "tar", "application", "x-tar");
        ins(&mut ext_map, "xz", "application", "x-xz");
        ins(&mut ext_map, "bz2", "application", "x-bzip2");
        ins(&mut ext_map, "7z", "application", "x-7z-compressed");
        ins(&mut ext_map, "rar", "application", "x-rar-compressed");
        ins(&mut ext_map, "deb", "application", "x-deb");
        ins(&mut ext_map, "rpm", "application", "x-rpm");

        // Desktop
        ins(&mut ext_map, "desktop", "application", "x-desktop");
        ins(&mut ext_map, "wasm", "application", "wasm");

        // Fonts
        ins(&mut ext_map, "ttf", "font", "ttf");
        ins(&mut ext_map, "otf", "font", "otf");
        ins(&mut ext_map, "woff", "font", "woff");
        ins(&mut ext_map, "woff2", "font", "woff2");

        MimeDatabase {
            ext_map,
            associations: HashMap::new(),
        }
    }

    /// Guess MIME type from a file extension (without leading dot).
    ///
    /// The extension is matched case-insensitively.
    pub fn guess_from_extension(&self, ext: &str) -> Option<MimeType> {
        self.ext_map.get(&ext.to_ascii_lowercase()).cloned()
    }

    /// Guess MIME type by inspecting magic bytes at the start of `data`.
    ///
    /// Returns the first matching signature, or `None` if no signature matches.
    pub fn guess_from_content(&self, data: &[u8]) -> Option<MimeType> {
        for (offset, signature, mime) in magic_table() {
            if data.len() >= offset + signature.len() {
                if &data[offset..offset + signature.len()] == signature {
                    // Disambiguate RIFF-based formats.
                    if signature == b"RIFF" {
                        if data.len() >= 12 && &data[8..12] == b"WEBP" {
                            return Some(MimeType::new("image", "webp"));
                        } else if data.len() >= 12 && &data[8..12] == b"WAVE" {
                            return Some(MimeType::new("audio", "wav"));
                        } else if data.len() >= 12 && &data[8..12] == b"AVI " {
                            return Some(MimeType::new("video", "x-msvideo"));
                        }
                        // Unknown RIFF variant, skip.
                        continue;
                    }
                    return Some(mime);
                }
            }
        }
        None
    }

    /// Register a default application for a MIME type.
    pub fn set_association(&mut self, mime: &MimeType, desktop_entry_id: &str) {
        self.associations
            .insert(mime.essence(), desktop_entry_id.to_string());
    }

    /// Get the default application for a MIME type.
    pub fn get_association(&self, mime: &MimeType) -> Option<MimeAssociation> {
        self.associations
            .get(&mime.essence())
            .map(|id| MimeAssociation {
                mime_type: mime.clone(),
                desktop_entry_id: id.clone(),
            })
    }

    /// Register a custom extension mapping.
    pub fn add_extension(&mut self, ext: &str, mime: MimeType) {
        self.ext_map.insert(ext.to_ascii_lowercase(), mime);
    }

    /// Return the number of built-in extension mappings.
    pub fn extension_count(&self) -> usize {
        self.ext_map.len()
    }
}

impl Default for MimeDatabase {
    fn default() -> Self {
        Self::new()
    }
}

fn ins(map: &mut HashMap<String, MimeType>, ext: &str, type_: &str, subtype: &str) {
    map.insert(ext.to_string(), MimeType::new(type_, subtype));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_new_and_essence() {
        let m = MimeType::new("text", "plain");
        assert_eq!(m.essence(), "text/plain");
    }

    #[test]
    fn mime_type_parse_valid() {
        let m = MimeType::parse("image/png").unwrap();
        assert_eq!(m.type_, "image");
        assert_eq!(m.subtype, "png");
    }

    #[test]
    fn mime_type_parse_invalid() {
        assert!(MimeType::parse("noslash").is_none());
        assert!(MimeType::parse("/subtype").is_none());
        assert!(MimeType::parse("type/").is_none());
    }

    #[test]
    fn mime_type_display() {
        let m = MimeType::new("application", "json");
        assert_eq!(format!("{m}"), "application/json");
    }

    #[test]
    fn mime_type_classification() {
        assert!(MimeType::new("text", "plain").is_text());
        assert!(MimeType::new("image", "png").is_image());
        assert!(MimeType::new("audio", "mpeg").is_audio());
        assert!(MimeType::new("video", "mp4").is_video());
        assert!(!MimeType::new("application", "pdf").is_text());
    }

    #[test]
    fn guess_from_extension_common() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_extension("txt"),
            Some(MimeType::new("text", "plain"))
        );
        assert_eq!(
            db.guess_from_extension("png"),
            Some(MimeType::new("image", "png"))
        );
        assert_eq!(
            db.guess_from_extension("rs"),
            Some(MimeType::new("text", "x-rust"))
        );
        assert_eq!(
            db.guess_from_extension("pdf"),
            Some(MimeType::new("application", "pdf"))
        );
        assert_eq!(
            db.guess_from_extension("mp4"),
            Some(MimeType::new("video", "mp4"))
        );
    }

    #[test]
    fn guess_from_extension_case_insensitive() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_extension("PNG"),
            Some(MimeType::new("image", "png"))
        );
        assert_eq!(
            db.guess_from_extension("Html"),
            Some(MimeType::new("text", "html"))
        );
    }

    #[test]
    fn guess_from_extension_unknown() {
        let db = MimeDatabase::new();
        assert!(db.guess_from_extension("xyz_unknown").is_none());
    }

    #[test]
    fn guess_from_content_png() {
        let db = MimeDatabase::new();
        let data = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("image", "png"))
        );
    }

    #[test]
    fn guess_from_content_jpeg() {
        let db = MimeDatabase::new();
        let data = b"\xff\xd8\xff\xe0\x00\x10JFIF";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("image", "jpeg"))
        );
    }

    #[test]
    fn guess_from_content_pdf() {
        let db = MimeDatabase::new();
        let data = b"%PDF-1.7\n";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("application", "pdf"))
        );
    }

    #[test]
    fn guess_from_content_zip() {
        let db = MimeDatabase::new();
        let data = b"PK\x03\x04extra_bytes";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("application", "zip"))
        );
    }

    #[test]
    fn guess_from_content_gif87a() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"GIF87a\x01\x00"),
            Some(MimeType::new("image", "gif"))
        );
    }

    #[test]
    fn guess_from_content_gif89a() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"GIF89a\x01\x00"),
            Some(MimeType::new("image", "gif"))
        );
    }

    #[test]
    fn guess_from_content_gzip() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"\x1f\x8b\x08\x00"),
            Some(MimeType::new("application", "gzip"))
        );
    }

    #[test]
    fn guess_from_content_elf() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"\x7fELF\x02\x01\x01"),
            Some(MimeType::new("application", "x-executable"))
        );
    }

    #[test]
    fn guess_from_content_mp4_ftyp() {
        let db = MimeDatabase::new();
        // MP4 files have "ftyp" at offset 4.
        let data = b"\x00\x00\x00\x18ftypmp42";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("video", "mp4"))
        );
    }

    #[test]
    fn guess_from_content_flac() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"fLaC\x00\x00"),
            Some(MimeType::new("audio", "flac"))
        );
    }

    #[test]
    fn guess_from_content_xml() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"<?xml version=\"1.0\"?>"),
            Some(MimeType::new("application", "xml"))
        );
    }

    #[test]
    fn guess_from_content_wasm() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"\x00asm\x01\x00\x00\x00"),
            Some(MimeType::new("application", "wasm"))
        );
    }

    #[test]
    fn guess_from_content_unknown() {
        let db = MimeDatabase::new();
        assert!(db.guess_from_content(b"random bytes here").is_none());
    }

    #[test]
    fn guess_from_content_empty() {
        let db = MimeDatabase::new();
        assert!(db.guess_from_content(b"").is_none());
    }

    #[test]
    fn guess_from_content_riff_webp() {
        let db = MimeDatabase::new();
        let data = b"RIFF\x00\x00\x00\x00WEBP";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("image", "webp"))
        );
    }

    #[test]
    fn guess_from_content_riff_wave() {
        let db = MimeDatabase::new();
        let data = b"RIFF\x00\x00\x00\x00WAVE";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("audio", "wav"))
        );
    }

    #[test]
    fn guess_from_content_riff_avi() {
        let db = MimeDatabase::new();
        let data = b"RIFF\x00\x00\x00\x00AVI ";
        assert_eq!(
            db.guess_from_content(data),
            Some(MimeType::new("video", "x-msvideo"))
        );
    }

    #[test]
    fn association_set_and_get() {
        let mut db = MimeDatabase::new();
        let mime = MimeType::new("text", "html");
        db.set_association(&mime, "firefox.desktop");
        let assoc = db.get_association(&mime).unwrap();
        assert_eq!(assoc.desktop_entry_id, "firefox.desktop");
        assert_eq!(assoc.mime_type, mime);
    }

    #[test]
    fn association_missing() {
        let db = MimeDatabase::new();
        assert!(
            db.get_association(&MimeType::new("text", "plain"))
                .is_none()
        );
    }

    #[test]
    fn add_custom_extension() {
        let mut db = MimeDatabase::new();
        db.add_extension("myext", MimeType::new("application", "x-myformat"));
        assert_eq!(
            db.guess_from_extension("myext"),
            Some(MimeType::new("application", "x-myformat"))
        );
    }

    #[test]
    fn extension_count_at_least_50() {
        let db = MimeDatabase::new();
        assert!(
            db.extension_count() >= 50,
            "expected >= 50, got {}",
            db.extension_count()
        );
    }

    #[test]
    fn guess_from_content_id3_mp3() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"ID3\x04\x00\x00"),
            Some(MimeType::new("audio", "mpeg"))
        );
    }

    #[test]
    fn guess_from_content_ogg() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"OggS\x00\x02"),
            Some(MimeType::new("audio", "ogg"))
        );
    }

    #[test]
    fn guess_from_content_bmp() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"BM\x00\x00\x00\x00"),
            Some(MimeType::new("image", "bmp"))
        );
    }

    #[test]
    fn guess_from_content_rar() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"Rar!\x1a\x07\x01\x00"),
            Some(MimeType::new("application", "x-rar-compressed"))
        );
    }

    #[test]
    fn guess_from_content_7z() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"7z\xbc\xaf\x27\x1c\x00\x04"),
            Some(MimeType::new("application", "x-7z-compressed"))
        );
    }

    #[test]
    fn guess_from_content_xz() {
        let db = MimeDatabase::new();
        assert_eq!(
            db.guess_from_content(b"\xfd7zXZ\x00\x00"),
            Some(MimeType::new("application", "x-xz"))
        );
    }
}
