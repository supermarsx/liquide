use crate::format::*;

#[test]
fn plain_text_mime() {
    assert_eq!(ClipboardFormat::PlainText.mime_type(), MIME_PLAIN_TEXT);
}

#[test]
fn html_mime() {
    assert_eq!(ClipboardFormat::Html.mime_type(), MIME_HTML);
}

#[test]
fn png_mime() {
    assert_eq!(ClipboardFormat::Png.mime_type(), MIME_PNG);
}

#[test]
fn custom_mime() {
    let fmt = ClipboardFormat::Custom("application/x-custom".to_string());
    assert_eq!(fmt.mime_type(), "application/x-custom");
}

#[test]
fn from_mime_text() {
    assert_eq!(
        ClipboardFormat::from_mime(MIME_PLAIN_TEXT),
        Some(ClipboardFormat::PlainText)
    );
    assert_eq!(
        ClipboardFormat::from_mime("text/plain"),
        Some(ClipboardFormat::PlainText)
    );
}

#[test]
fn from_mime_html() {
    assert_eq!(
        ClipboardFormat::from_mime(MIME_HTML),
        Some(ClipboardFormat::Html)
    );
}

#[test]
fn from_mime_unknown() {
    assert_eq!(ClipboardFormat::from_mime("application/octet-stream"), None);
}

#[test]
fn is_text() {
    assert!(ClipboardFormat::PlainText.is_text());
    assert!(ClipboardFormat::Html.is_text());
    assert!(ClipboardFormat::RichText.is_text());
    assert!(ClipboardFormat::FileUriList.is_text());
    assert!(!ClipboardFormat::Png.is_text());
    assert!(!ClipboardFormat::Jpeg.is_text());
}

#[test]
fn is_image() {
    assert!(ClipboardFormat::Png.is_image());
    assert!(ClipboardFormat::Jpeg.is_image());
    assert!(ClipboardFormat::Svg.is_image());
    assert!(!ClipboardFormat::PlainText.is_image());
    assert!(!ClipboardFormat::Html.is_image());
}

#[test]
fn format_serde_roundtrip() {
    let fmt = ClipboardFormat::Html;
    let json = serde_json::to_string(&fmt).unwrap();
    let back: ClipboardFormat = serde_json::from_str(&json).unwrap();
    assert_eq!(fmt, back);

    let custom = ClipboardFormat::Custom("x/y".to_string());
    let json2 = serde_json::to_string(&custom).unwrap();
    let back2: ClipboardFormat = serde_json::from_str(&json2).unwrap();
    assert_eq!(custom, back2);
}
