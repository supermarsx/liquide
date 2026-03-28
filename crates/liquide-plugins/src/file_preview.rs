use std::path::Path;

/// Preview content types
#[derive(Debug, Clone)]
pub enum PreviewContent {
    Text(String),
    Image { width: u32, height: u32, data: Vec<u8> },
    Html(String),
    Metadata(Vec<(String, String)>),
    Unsupported,
}

pub trait FilePreviewProvider: Send + Sync {
    fn supported_extensions(&self) -> Vec<String>;
    fn supported_mime_types(&self) -> Vec<String>;
    fn generate_preview(&self, path: &Path, max_width: u32, max_height: u32) -> PreviewContent;
    fn generate_thumbnail(&self, path: &Path, size: u32) -> Option<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_content_text() {
        let content = PreviewContent::Text("Hello, world!".into());
        assert!(matches!(content, PreviewContent::Text(_)));
    }

    #[test]
    fn preview_content_image() {
        let content = PreviewContent::Image {
            width: 100,
            height: 50,
            data: vec![0u8; 100 * 50 * 4],
        };
        match &content {
            PreviewContent::Image { width, height, data } => {
                assert_eq!(*width, 100);
                assert_eq!(*height, 50);
                assert_eq!(data.len(), 20000);
            }
            _ => panic!("expected Image"),
        }
    }

    #[test]
    fn preview_content_html() {
        let content = PreviewContent::Html("<h1>Preview</h1>".into());
        assert!(matches!(content, PreviewContent::Html(_)));
    }

    #[test]
    fn preview_content_metadata() {
        let content = PreviewContent::Metadata(vec![
            ("Size".into(), "1.2 MB".into()),
            ("Modified".into(), "2026-01-15".into()),
        ]);
        match &content {
            PreviewContent::Metadata(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].0, "Size");
            }
            _ => panic!("expected Metadata"),
        }
    }

    #[test]
    fn preview_content_unsupported() {
        let content = PreviewContent::Unsupported;
        assert!(matches!(content, PreviewContent::Unsupported));
    }
}
