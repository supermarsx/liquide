//! Persistent storage for clipboard history.
//!
//! Entries are serialised to a simple binary format that can be read back on
//! restart.  Sensitive entries are never persisted.  Image data larger than
//! `MAX_PERSISTED_IMAGE_BYTES` is skipped to keep the file reasonable.

use std::io::{self, Read, Write};

use crate::entry::{ClipboardContent, ClipboardEntry, ImageFormat};

/// Maximum image payload that will be persisted (2 MB).
const MAX_PERSISTED_IMAGE_BYTES: usize = 2 * 1024 * 1024;

/// Magic header bytes for the clipboard history file.
const MAGIC: &[u8; 4] = b"CLHX";

/// Current format version.
const FORMAT_VERSION: u8 = 1;

// Tag bytes for content variants.
const TAG_TEXT: u8 = 1;
const TAG_RICH_TEXT: u8 = 2;
const TAG_IMAGE: u8 = 3;
const TAG_FILE_PATHS: u8 = 4;
const TAG_COLOR: u8 = 5;
const TAG_CUSTOM: u8 = 6;

// Tag bytes for image formats.
const IMG_PNG: u8 = 1;
const IMG_BMP: u8 = 2;
const IMG_RGBA32: u8 = 3;

/// Errors that can occur during persistence operations.
#[derive(Debug)]
pub enum PersistError {
    Io(io::Error),
    /// The file header or version is not recognised.
    InvalidFormat(String),
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::InvalidFormat(msg) => write!(f, "invalid format: {msg}"),
        }
    }
}

impl std::error::Error for PersistError {}

impl From<io::Error> for PersistError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Result type for persistence operations.
pub type PersistResult<T> = Result<T, PersistError>;

/// Determine whether an entry should be persisted.
#[must_use]
pub fn should_persist(entry: &ClipboardEntry) -> bool {
    // Never persist sensitive entries.
    if entry.sensitive {
        return false;
    }
    // Skip oversized images.
    if let ClipboardContent::Image { data, .. } = &entry.content {
        if data.len() > MAX_PERSISTED_IMAGE_BYTES {
            return false;
        }
    }
    true
}

/// Serialise a list of entries to a writer.  Only entries passing
/// [`should_persist`] are written.
pub fn save_entries<W: Write>(entries: &[ClipboardEntry], writer: &mut W) -> PersistResult<usize> {
    let persistable: Vec<&ClipboardEntry> = entries.iter().filter(|e| should_persist(e)).collect();

    writer.write_all(MAGIC)?;
    writer.write_all(&[FORMAT_VERSION])?;

    let count = persistable.len() as u32;
    writer.write_all(&count.to_le_bytes())?;

    for entry in &persistable {
        write_entry(writer, entry)?;
    }

    Ok(persistable.len())
}

/// Deserialise entries from a reader, returning them newest-first (the
/// order they were written).
pub fn load_entries<R: Read>(reader: &mut R) -> PersistResult<Vec<ClipboardEntry>> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(PersistError::InvalidFormat("bad magic header".into()));
    }

    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != FORMAT_VERSION {
        return Err(PersistError::InvalidFormat(format!(
            "unsupported version {}",
            version[0]
        )));
    }

    let mut count_buf = [0u8; 4];
    reader.read_exact(&mut count_buf)?;
    let count = u32::from_le_bytes(count_buf) as usize;

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(read_entry(reader)?);
    }

    Ok(entries)
}

// -----------------------------------------------------------------------
// Internal serialisation helpers
// -----------------------------------------------------------------------

fn write_u64<W: Write>(w: &mut W, v: u64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn write_u32<W: Write>(w: &mut W, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn write_bytes<W: Write>(w: &mut W, data: &[u8]) -> io::Result<()> {
    write_u32(w, data.len() as u32)?;
    w.write_all(data)
}

fn read_bytes<R: Read>(r: &mut R) -> io::Result<Vec<u8>> {
    let len = read_u32(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

fn write_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    write_bytes(w, s.as_bytes())
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let bytes = read_bytes(r)?;
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_opt_string<W: Write>(w: &mut W, s: &Option<String>) -> io::Result<()> {
    match s {
        Some(val) => {
            w.write_all(&[1])?;
            write_string(w, val)
        }
        None => w.write_all(&[0]),
    }
}

fn read_opt_string<R: Read>(r: &mut R) -> io::Result<Option<String>> {
    let mut tag = [0u8; 1];
    r.read_exact(&mut tag)?;
    if tag[0] == 0 {
        Ok(None)
    } else {
        Ok(Some(read_string(r)?))
    }
}

fn write_content<W: Write>(w: &mut W, content: &ClipboardContent) -> io::Result<()> {
    match content {
        ClipboardContent::Text(s) => {
            w.write_all(&[TAG_TEXT])?;
            write_string(w, s)
        }
        ClipboardContent::RichText {
            html,
            plain_fallback,
        } => {
            w.write_all(&[TAG_RICH_TEXT])?;
            write_string(w, html)?;
            write_string(w, plain_fallback)
        }
        ClipboardContent::Image {
            width,
            height,
            data,
            format,
        } => {
            w.write_all(&[TAG_IMAGE])?;
            write_u32(w, *width)?;
            write_u32(w, *height)?;
            let fmt_tag = match format {
                ImageFormat::Png => IMG_PNG,
                ImageFormat::Bmp => IMG_BMP,
                ImageFormat::Rgba32 => IMG_RGBA32,
            };
            w.write_all(&[fmt_tag])?;
            write_bytes(w, data)
        }
        ClipboardContent::FilePaths(paths) => {
            w.write_all(&[TAG_FILE_PATHS])?;
            write_u32(w, paths.len() as u32)?;
            for p in paths {
                write_string(w, p)?;
            }
            Ok(())
        }
        ClipboardContent::Color { r, g, b, a } => {
            w.write_all(&[TAG_COLOR])?;
            w.write_all(&[*r, *g, *b, *a])
        }
        ClipboardContent::Custom { mime_type, data } => {
            w.write_all(&[TAG_CUSTOM])?;
            write_string(w, mime_type)?;
            write_bytes(w, data)
        }
    }
}

fn read_content<R: Read>(r: &mut R) -> PersistResult<ClipboardContent> {
    let mut tag = [0u8; 1];
    r.read_exact(&mut tag)?;

    match tag[0] {
        TAG_TEXT => {
            let s = read_string(r)?;
            Ok(ClipboardContent::Text(s))
        }
        TAG_RICH_TEXT => {
            let html = read_string(r)?;
            let plain = read_string(r)?;
            Ok(ClipboardContent::RichText {
                html,
                plain_fallback: plain,
            })
        }
        TAG_IMAGE => {
            let width = read_u32(r)?;
            let height = read_u32(r)?;
            let mut fmt_tag = [0u8; 1];
            r.read_exact(&mut fmt_tag)?;
            let format = match fmt_tag[0] {
                IMG_PNG => ImageFormat::Png,
                IMG_BMP => ImageFormat::Bmp,
                IMG_RGBA32 => ImageFormat::Rgba32,
                other => {
                    return Err(PersistError::InvalidFormat(format!(
                        "unknown image format tag {other}"
                    )));
                }
            };
            let data = read_bytes(r)?;
            Ok(ClipboardContent::Image {
                width,
                height,
                data,
                format,
            })
        }
        TAG_FILE_PATHS => {
            let count = read_u32(r)? as usize;
            let mut paths = Vec::with_capacity(count);
            for _ in 0..count {
                paths.push(read_string(r)?);
            }
            Ok(ClipboardContent::FilePaths(paths))
        }
        TAG_COLOR => {
            let mut rgba = [0u8; 4];
            r.read_exact(&mut rgba)?;
            Ok(ClipboardContent::Color {
                r: rgba[0],
                g: rgba[1],
                b: rgba[2],
                a: rgba[3],
            })
        }
        TAG_CUSTOM => {
            let mime = read_string(r)?;
            let data = read_bytes(r)?;
            Ok(ClipboardContent::Custom {
                mime_type: mime,
                data,
            })
        }
        other => Err(PersistError::InvalidFormat(format!(
            "unknown content tag {other}"
        ))),
    }
}

fn write_entry<W: Write>(w: &mut W, entry: &ClipboardEntry) -> PersistResult<()> {
    write_u64(w, entry.id)?;
    write_u64(w, entry.timestamp)?;
    write_opt_string(w, &entry.source_app)?;
    w.write_all(&[entry.pinned as u8])?;
    write_u32(w, entry.times_pasted)?;
    write_content(w, &entry.content)?;
    Ok(())
}

fn read_entry<R: Read>(r: &mut R) -> PersistResult<ClipboardEntry> {
    let id = read_u64(r)?;
    let timestamp = read_u64(r)?;
    let source_app = read_opt_string(r)?;
    let mut pinned_buf = [0u8; 1];
    r.read_exact(&mut pinned_buf)?;
    let pinned = pinned_buf[0] != 0;
    let times_pasted = read_u32(r)?;
    let content = read_content(r)?;

    Ok(ClipboardEntry {
        id,
        content,
        timestamp,
        source_app,
        pinned,
        sensitive: false, // sensitive entries are never persisted
        times_pasted,
    })
}
