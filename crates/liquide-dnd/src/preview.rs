//! Drag preview visuals.
//!
//! [`DragPreview`] describes the visual feedback shown under the cursor
//! during a drag operation.

use serde::{Deserialize, Serialize};

/// Visual representation of a drag operation, displayed under the cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DragPreview {
    /// A named icon (looked up from the icon theme).
    Icon(String),
    /// A bitmap thumbnail.
    Thumbnail {
        width: u32,
        height: u32,
        /// RGBA pixel data.
        #[serde(with = "serde_bytes_vec")]
        data: Vec<u8>,
    },
    /// A text label rendered as the drag preview.
    TextLabel(String),
    /// A snapshot of the source window (captured at drag start).
    WindowSnapshot,
}

impl DragPreview {
    /// Create an icon preview.
    #[must_use]
    pub fn icon(name: impl Into<String>) -> Self {
        DragPreview::Icon(name.into())
    }

    /// Create a text label preview.
    #[must_use]
    pub fn text_label(text: impl Into<String>) -> Self {
        DragPreview::TextLabel(text.into())
    }

    /// Create a thumbnail preview from RGBA pixel data.
    #[must_use]
    pub fn thumbnail(width: u32, height: u32, data: Vec<u8>) -> Self {
        DragPreview::Thumbnail {
            width,
            height,
            data,
        }
    }

    /// Create a window snapshot preview.
    #[must_use]
    pub fn window_snapshot() -> Self {
        DragPreview::WindowSnapshot
    }

    /// Returns `true` if this is an icon preview.
    #[must_use]
    pub fn is_icon(&self) -> bool {
        matches!(self, DragPreview::Icon(_))
    }

    /// Returns `true` if this is a thumbnail preview.
    #[must_use]
    pub fn is_thumbnail(&self) -> bool {
        matches!(self, DragPreview::Thumbnail { .. })
    }

    /// Returns `true` if this is a text label preview.
    #[must_use]
    pub fn is_text_label(&self) -> bool {
        matches!(self, DragPreview::TextLabel(_))
    }

    /// Returns `true` if this is a window snapshot preview.
    #[must_use]
    pub fn is_window_snapshot(&self) -> bool {
        matches!(self, DragPreview::WindowSnapshot)
    }
}

/// Serde helper for `Vec<u8>` — serialize as raw bytes in binary formats
/// and as a regular vec in JSON.
mod serde_bytes_vec {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(data: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        data.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        Vec::<u8>::deserialize(d)
    }
}

use crate::drag_data::{DragData, DragFormat};

/// Style of drag preview to generate.
#[derive(Debug, Clone, PartialEq)]
pub enum DragPreviewStyle {
    /// Show a named icon (from the icon theme).
    Icon,
    /// Show a bitmap thumbnail of the dragged content.
    Thumbnail,
    /// Ghost: semi-transparent snapshot with the given opacity (0.0..=1.0).
    Ghost(f32),
    /// Custom preview with explicit parameters.
    Custom,
}

impl Default for DragPreviewStyle {
    fn default() -> Self {
        DragPreviewStyle::Icon
    }
}

/// Fully resolved drag preview with all rendering parameters.
///
/// Produced by [`compute_preview`] from drag data and a style hint.
#[derive(Debug, Clone)]
pub struct DragPreviewConfig {
    /// The base visual to show (icon name, thumbnail pixels, etc.).
    pub preview: DragPreview,
    /// Optional text label shown below or beside the icon.
    pub label: Option<String>,
    /// Badge count for multi-item drags (e.g., "3" on a file icon).
    /// 0 means no badge.
    pub badge_count: u32,
    /// Opacity of the entire preview (0.0 = invisible, 1.0 = opaque).
    pub opacity: f32,
    /// Horizontal offset from the cursor hotspot to the preview origin.
    pub offset_x: f32,
    /// Vertical offset from the cursor hotspot to the preview origin.
    pub offset_y: f32,
    /// Preview width in pixels.
    pub width: f32,
    /// Preview height in pixels.
    pub height: f32,
}

impl DragPreviewConfig {
    /// Create a minimal preview config.
    #[must_use]
    pub fn new(preview: DragPreview) -> Self {
        Self {
            preview,
            label: None,
            badge_count: 0,
            opacity: 0.85,
            offset_x: -12.0,
            offset_y: -12.0,
            width: 48.0,
            height: 48.0,
        }
    }

    /// Set the label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the badge count.
    #[must_use]
    pub fn with_badge(mut self, count: u32) -> Self {
        self.badge_count = count;
        self
    }

    /// Set the opacity.
    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set the cursor offset.
    #[must_use]
    pub fn with_offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    /// Set the preview size.
    #[must_use]
    pub fn with_size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    /// Compute the bounding rectangle for this preview at the given cursor
    /// position.
    #[must_use]
    pub fn preview_at(&self, cursor_x: f32, cursor_y: f32) -> PreviewRect {
        PreviewRect {
            x: cursor_x + self.offset_x,
            y: cursor_y + self.offset_y,
            width: self.width,
            height: self.height,
        }
    }
}

/// Axis-aligned rectangle for the drag preview.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PreviewRect {
    /// Whether the point (px, py) is inside this rect.
    #[must_use]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Compute a drag preview from data and a style hint.
///
/// Examines the drag data's formats and the requested style to produce
/// a fully resolved [`DragPreviewConfig`].
#[must_use]
pub fn compute_preview(data: &DragData, style: &DragPreviewStyle) -> DragPreviewConfig {
    match style {
        DragPreviewStyle::Icon => compute_icon_preview(data),
        DragPreviewStyle::Thumbnail => compute_thumbnail_preview(data),
        DragPreviewStyle::Ghost(opacity) => {
            let mut cfg = compute_icon_preview(data);
            cfg.opacity = opacity.clamp(0.0, 1.0);
            cfg
        }
        DragPreviewStyle::Custom => {
            // For custom, return a basic config; caller is expected to modify it.
            DragPreviewConfig::new(DragPreview::icon("application-x-generic"))
        }
    }
}

/// Compute an icon-style preview based on data content.
fn compute_icon_preview(data: &DragData) -> DragPreviewConfig {
    if let Some(paths) = data.get_file_paths() {
        let count = paths.len();
        let icon_name = if count == 1 {
            file_icon_for_path(&paths[0])
        } else {
            "document-multiple"
        };
        let label = if count == 1 {
            file_basename(&paths[0])
        } else {
            format!("{count} items")
        };
        let mut cfg = DragPreviewConfig::new(DragPreview::icon(icon_name)).with_label(label);
        if count > 1 {
            cfg = cfg.with_badge(count as u32);
        }
        cfg
    } else if let Some(text) = data.get_text() {
        let snippet = text_snippet(text, 24);
        DragPreviewConfig::new(DragPreview::text_label(&snippet))
            .with_label(snippet)
            .with_size((text.len().min(24) as f32 * 7.0 + 16.0).max(48.0), 32.0)
    } else if data.has_uri() {
        let uri = data.get_uri().unwrap_or("link");
        let label = text_snippet(uri, 30);
        DragPreviewConfig::new(DragPreview::icon("text-x-uri")).with_label(label)
    } else if data.has_image() {
        DragPreviewConfig::new(DragPreview::icon("image-x-generic")).with_label("Image")
    } else {
        DragPreviewConfig::new(DragPreview::icon("application-x-generic"))
    }
}

/// Compute a thumbnail-style preview. Falls back to icon if no image data.
fn compute_thumbnail_preview(data: &DragData) -> DragPreviewConfig {
    // Try to find image data for a real thumbnail
    if let Some(img) = data.find_format(DragFormat::is_image) {
        if let DragFormat::Image {
            width,
            height,
            data: pixels,
        } = img
        {
            let (tw, th) = thumbnail_size(*width, *height, 96);
            return DragPreviewConfig::new(DragPreview::thumbnail(tw, th, pixels.clone()))
                .with_size(tw as f32, th as f32);
        }
    }
    // Fallback to icon
    compute_icon_preview(data)
}

/// Compute thumbnail dimensions fitting within max_side, preserving aspect ratio.
fn thumbnail_size(w: u32, h: u32, max_side: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (max_side, max_side);
    }
    if w <= max_side && h <= max_side {
        return (w, h);
    }
    if w >= h {
        let tw = max_side;
        let th = (h as f64 * max_side as f64 / w as f64).round() as u32;
        (tw, th.max(1))
    } else {
        let th = max_side;
        let tw = (w as f64 * max_side as f64 / h as f64).round() as u32;
        (tw.max(1), th)
    }
}

/// Extract the file basename from a path.
fn file_basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Choose an icon name based on file extension.
fn file_icon_for_path(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "md" | "log" | "csv" => "text-plain",
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "java" | "go" | "rb" => "text-x-source",
        "html" | "htm" | "xml" | "css" | "json" | "yaml" | "yml" | "toml" => "text-x-markup",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" => "image-x-generic",
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => "audio-x-generic",
        "mp4" | "mkv" | "avi" | "webm" | "mov" | "wmv" => "video-x-generic",
        "pdf" => "application-pdf",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => "package-x-generic",
        _ => "application-x-generic",
    }
}

/// Truncate text to a snippet for preview labels.
fn text_snippet(text: &str, max_len: usize) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    if first_line.len() <= max_len {
        first_line.to_string()
    } else {
        let truncated: String = first_line.chars().take(max_len - 1).collect();
        format!("{truncated}\u{2026}") // ellipsis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_icon() {
        let p = DragPreview::icon("document-copy");
        assert!(p.is_icon());
        assert!(!p.is_thumbnail());
        match p {
            DragPreview::Icon(name) => assert_eq!(name, "document-copy"),
            _ => panic!("expected Icon"),
        }
    }

    #[test]
    fn test_preview_text_label() {
        let p = DragPreview::text_label("3 files");
        assert!(p.is_text_label());
        match p {
            DragPreview::TextLabel(s) => assert_eq!(s, "3 files"),
            _ => panic!("expected TextLabel"),
        }
    }

    #[test]
    fn test_preview_thumbnail() {
        let p = DragPreview::thumbnail(4, 4, vec![0u8; 64]);
        assert!(p.is_thumbnail());
        match p {
            DragPreview::Thumbnail {
                width,
                height,
                data,
            } => {
                assert_eq!(width, 4);
                assert_eq!(height, 4);
                assert_eq!(data.len(), 64);
            }
            _ => panic!("expected Thumbnail"),
        }
    }

    #[test]
    fn test_preview_window_snapshot() {
        let p = DragPreview::window_snapshot();
        assert!(p.is_window_snapshot());
    }

    // ---- DragPreviewStyle tests ----

    #[test]
    fn test_preview_style_default() {
        let style = DragPreviewStyle::default();
        assert_eq!(style, DragPreviewStyle::Icon);
    }

    #[test]
    fn test_preview_style_ghost() {
        let style = DragPreviewStyle::Ghost(0.5);
        assert_eq!(style, DragPreviewStyle::Ghost(0.5));
    }

    // ---- DragPreviewConfig tests ----

    #[test]
    fn test_config_new() {
        let cfg = DragPreviewConfig::new(DragPreview::icon("test"));
        assert!(cfg.label.is_none());
        assert_eq!(cfg.badge_count, 0);
        assert!((cfg.opacity - 0.85).abs() < 0.01);
        assert!((cfg.width - 48.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_builder() {
        let cfg = DragPreviewConfig::new(DragPreview::icon("test"))
            .with_label("hello")
            .with_badge(3)
            .with_opacity(0.6)
            .with_offset(-5.0, -10.0)
            .with_size(64.0, 64.0);
        assert_eq!(cfg.label.as_deref(), Some("hello"));
        assert_eq!(cfg.badge_count, 3);
        assert!((cfg.opacity - 0.6).abs() < 0.01);
        assert!((cfg.offset_x - (-5.0)).abs() < f32::EPSILON);
        assert!((cfg.width - 64.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_opacity_clamped() {
        let cfg = DragPreviewConfig::new(DragPreview::icon("x")).with_opacity(2.0);
        assert!((cfg.opacity - 1.0).abs() < f32::EPSILON);
        let cfg2 = DragPreviewConfig::new(DragPreview::icon("x")).with_opacity(-0.5);
        assert!((cfg2.opacity - 0.0).abs() < f32::EPSILON);
    }

    // ---- preview_at tests ----

    #[test]
    fn test_preview_at() {
        let cfg = DragPreviewConfig::new(DragPreview::icon("x"))
            .with_offset(-10.0, -10.0)
            .with_size(48.0, 48.0);
        let rect = cfg.preview_at(100.0, 200.0);
        assert!((rect.x - 90.0).abs() < f32::EPSILON);
        assert!((rect.y - 190.0).abs() < f32::EPSILON);
        assert!((rect.width - 48.0).abs() < f32::EPSILON);
        assert!((rect.height - 48.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_preview_rect_contains() {
        let r = PreviewRect {
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 30.0,
        };
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(35.0, 35.0));
        assert!(!r.contains(60.0, 20.0)); // right edge exclusive
        assert!(!r.contains(9.0, 25.0)); // left outside
    }

    // ---- compute_preview tests ----

    #[test]
    fn test_compute_preview_text() {
        let data = DragData::text("hello world");
        let cfg = compute_preview(&data, &DragPreviewStyle::Icon);
        assert!(cfg.preview.is_text_label());
        assert!(cfg.label.is_some());
    }

    #[test]
    fn test_compute_preview_single_file() {
        let data = DragData::file_paths(vec!["/home/user/document.txt".into()]);
        let cfg = compute_preview(&data, &DragPreviewStyle::Icon);
        assert!(cfg.preview.is_icon());
        assert_eq!(cfg.label.as_deref(), Some("document.txt"));
        assert_eq!(cfg.badge_count, 0); // single file, no badge
    }

    #[test]
    fn test_compute_preview_multi_file() {
        let data = DragData::file_paths(vec![
            "/home/a.txt".into(),
            "/home/b.png".into(),
            "/home/c.rs".into(),
        ]);
        let cfg = compute_preview(&data, &DragPreviewStyle::Icon);
        assert!(cfg.preview.is_icon());
        assert_eq!(cfg.badge_count, 3);
        assert_eq!(cfg.label.as_deref(), Some("3 items"));
    }

    #[test]
    fn test_compute_preview_ghost() {
        let data = DragData::text("ghost");
        let cfg = compute_preview(&data, &DragPreviewStyle::Ghost(0.3));
        assert!((cfg.opacity - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_compute_preview_thumbnail_with_image() {
        let mut data = DragData::new();
        data.add_format(DragFormat::Image {
            width: 200,
            height: 100,
            data: vec![0; 200 * 100 * 4],
        });
        let cfg = compute_preview(&data, &DragPreviewStyle::Thumbnail);
        assert!(cfg.preview.is_thumbnail());
        // 200x100 scaled to max 96 -> 96x48
        assert!((cfg.width - 96.0).abs() < f32::EPSILON);
        assert!((cfg.height - 48.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compute_preview_thumbnail_fallback() {
        // No image data -> falls back to icon
        let data = DragData::text("no image here");
        let cfg = compute_preview(&data, &DragPreviewStyle::Thumbnail);
        // Should fall back to text label (icon preview of text)
        assert!(cfg.preview.is_text_label());
    }

    #[test]
    fn test_compute_preview_uri() {
        let data = DragData::uri("https://example.com/page");
        let cfg = compute_preview(&data, &DragPreviewStyle::Icon);
        assert!(cfg.preview.is_icon());
        assert!(cfg.label.is_some());
    }

    #[test]
    fn test_compute_preview_custom_style() {
        let data = DragData::text("whatever");
        let cfg = compute_preview(&data, &DragPreviewStyle::Custom);
        assert!(cfg.preview.is_icon()); // custom returns generic icon
    }

    // ---- Helper function tests ----

    #[test]
    fn test_text_snippet_short() {
        let s = text_snippet("hello", 24);
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_text_snippet_truncated() {
        let long = "a".repeat(50);
        let s = text_snippet(&long, 10);
        assert_eq!(s.chars().count(), 10); // 9 chars + ellipsis
        assert!(s.ends_with('\u{2026}'));
    }

    #[test]
    fn test_text_snippet_multiline() {
        let text = "first line\nsecond line";
        let s = text_snippet(text, 50);
        assert_eq!(s, "first line");
    }

    #[test]
    fn test_file_icon_for_path() {
        assert_eq!(file_icon_for_path("/home/test.txt"), "text-plain");
        assert_eq!(file_icon_for_path("/home/main.rs"), "text-x-source");
        assert_eq!(file_icon_for_path("/home/photo.png"), "image-x-generic");
        assert_eq!(file_icon_for_path("/home/song.mp3"), "audio-x-generic");
        assert_eq!(file_icon_for_path("/home/movie.mp4"), "video-x-generic");
        assert_eq!(file_icon_for_path("/home/doc.pdf"), "application-pdf");
        assert_eq!(file_icon_for_path("/home/archive.zip"), "package-x-generic");
        assert_eq!(
            file_icon_for_path("/home/unknown.xyz"),
            "application-x-generic"
        );
    }

    #[test]
    fn test_file_basename() {
        assert_eq!(file_basename("/home/user/doc.txt"), "doc.txt");
        assert_eq!(file_basename("nopath"), "nopath");
        assert_eq!(file_basename("/trailing/"), "");
    }

    #[test]
    fn test_thumbnail_size_fits() {
        assert_eq!(thumbnail_size(32, 32, 96), (32, 32));
    }

    #[test]
    fn test_thumbnail_size_landscape() {
        let (w, h) = thumbnail_size(200, 100, 96);
        assert_eq!(w, 96);
        assert_eq!(h, 48);
    }

    #[test]
    fn test_thumbnail_size_portrait() {
        let (w, h) = thumbnail_size(100, 200, 96);
        assert_eq!(w, 48);
        assert_eq!(h, 96);
    }

    #[test]
    fn test_thumbnail_size_zero() {
        assert_eq!(thumbnail_size(0, 0, 96), (96, 96));
    }
}
