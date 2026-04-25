//! Tests for FileProperties, MIME detection, and size formatting.

use crate::entry::FileEntry;
use crate::properties::{FileProperties, detect_mime_type, format_permissions, format_size};

#[test]
fn test_format_size_bytes() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1023), "1023 B");
}

#[test]
fn test_format_size_kb() {
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(2048), "2.0 KB");
}

#[test]
fn test_format_size_mb() {
    assert_eq!(format_size(1024 * 1024), "1.0 MB");
    assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
}

#[test]
fn test_format_size_gb() {
    assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
}

#[test]
fn test_format_size_tb() {
    assert_eq!(format_size(2u64 * 1024 * 1024 * 1024 * 1024), "2.0 TB");
}

#[test]
fn test_format_permissions_755() {
    assert_eq!(format_permissions(0o755), "rwxr-xr-x");
}

#[test]
fn test_format_permissions_644() {
    assert_eq!(format_permissions(0o644), "rw-r--r--");
}

#[test]
fn test_format_permissions_000() {
    assert_eq!(format_permissions(0o000), "---------");
}

#[test]
fn test_detect_mime_text() {
    assert_eq!(detect_mime_type("readme.txt"), "text/plain");
    assert_eq!(detect_mime_type("data.csv"), "text/csv");
    assert_eq!(detect_mime_type("NOTES.MD"), "text/markdown");
}

#[test]
fn test_detect_mime_source() {
    assert_eq!(detect_mime_type("main.rs"), "text/x-rust");
    assert_eq!(detect_mime_type("app.py"), "text/x-python");
    assert_eq!(detect_mime_type("index.js"), "text/javascript");
    assert_eq!(detect_mime_type("style.ts"), "text/typescript");
    assert_eq!(detect_mime_type("main.go"), "text/x-go");
}

#[test]
fn test_detect_mime_web() {
    assert_eq!(detect_mime_type("page.html"), "text/html");
    assert_eq!(detect_mime_type("style.css"), "text/css");
    assert_eq!(detect_mime_type("data.json"), "application/json");
    assert_eq!(detect_mime_type("config.yaml"), "application/x-yaml");
    assert_eq!(detect_mime_type("Cargo.toml"), "application/toml");
}

#[test]
fn test_detect_mime_images() {
    assert_eq!(detect_mime_type("photo.png"), "image/png");
    assert_eq!(detect_mime_type("photo.jpg"), "image/jpeg");
    assert_eq!(detect_mime_type("photo.gif"), "image/gif");
    assert_eq!(detect_mime_type("photo.webp"), "image/webp");
    assert_eq!(detect_mime_type("icon.svg"), "image/svg+xml");
    assert_eq!(detect_mime_type("icon.bmp"), "image/bmp");
}

#[test]
fn test_detect_mime_audio_video() {
    assert_eq!(detect_mime_type("song.mp3"), "audio/mpeg");
    assert_eq!(detect_mime_type("song.flac"), "audio/flac");
    assert_eq!(detect_mime_type("movie.mp4"), "video/mp4");
    assert_eq!(detect_mime_type("movie.mkv"), "video/x-matroska");
    assert_eq!(detect_mime_type("movie.webm"), "video/webm");
}

#[test]
fn test_detect_mime_documents() {
    assert_eq!(detect_mime_type("report.pdf"), "application/pdf");
    assert_eq!(
        detect_mime_type("sheet.xlsx"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
}

#[test]
fn test_detect_mime_archives() {
    assert_eq!(detect_mime_type("data.zip"), "application/zip");
    assert_eq!(detect_mime_type("data.tar"), "application/x-tar");
    assert_eq!(detect_mime_type("data.7z"), "application/x-7z-compressed");
}

#[test]
fn test_detect_mime_fonts() {
    assert_eq!(detect_mime_type("font.ttf"), "font/ttf");
    assert_eq!(detect_mime_type("font.woff2"), "font/woff2");
}

#[test]
fn test_detect_mime_unknown() {
    assert_eq!(detect_mime_type("data.xyz"), "application/x-xyz");
}

#[test]
fn test_detect_mime_no_extension() {
    assert_eq!(detect_mime_type("Makefile"), "application/octet-stream");
}

#[test]
fn test_file_properties_from_entry() {
    let entry = FileEntry::file(
        "readme.md".into(),
        "/home/user/readme.md".into(),
        2048,
        1000,
    );
    let props = FileProperties::from_entry(&entry);
    assert_eq!(props.name, "readme.md");
    assert_eq!(props.extension, "md");
    assert_eq!(props.size, 2048);
    assert_eq!(props.modified, 1000);
    assert!(!props.is_hidden);
    assert!(!props.is_readonly);
    assert_eq!(props.formatted_size(), "2.0 KB");
}

#[test]
fn test_file_properties_hidden_readonly() {
    let mut entry = FileEntry::file(".config".into(), "/.config".into(), 100, 0);
    entry.permissions.writable = false;
    let props = FileProperties::from_entry(&entry);
    assert!(props.is_hidden);
    assert!(props.is_readonly);
}
