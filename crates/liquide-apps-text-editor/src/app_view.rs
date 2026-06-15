//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! text editor, plus the real content view that replaces the `Label`
//! placeholder.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
    ContentSpan,
};

use crate::runtime::EditorRuntime;
use crate::syntax::TokenKind;

/// Map a syntax token kind to a packed `0xRRGGBBAA` foreground color so the
/// shell can paint highlighted source without owning a theme.
fn token_color(kind: TokenKind) -> Option<u32> {
    let rgb = match kind {
        TokenKind::Keyword => 0xC5_92_E8,
        TokenKind::Type => 0x4E_C9_B0,
        TokenKind::Function => 0xDC_DC_AA,
        TokenKind::String => 0xCE_91_78,
        TokenKind::Number => 0xB5_CE_A8,
        TokenKind::Comment => 0x6A_99_55,
        TokenKind::Operator => 0xD4_D4_D4,
        TokenKind::Punctuation => 0xD4_D4_D4,
        TokenKind::Identifier => 0x9C_DC_FE,
        TokenKind::Whitespace | TokenKind::Unknown => return None,
    };
    Some((rgb << 8) | 0xFF)
}

impl AppTextInput for EditorRuntime {
    fn handle_text(&mut self, text: &str) -> bool {
        let mut changed = false;
        for ch in text.chars() {
            if ch == '\n' {
                // Newlines route through the key protocol so auto-indent runs.
                changed |= self.handle_key("Enter", false, false);
            } else if self.handle_char(ch) {
                changed = true;
            }
        }
        changed
    }

    fn handle_key(&mut self, key: &AppKey) -> bool {
        match key {
            AppKey::Char(c) => self.handle_char(*c),
            // The editor's key protocol takes a `&str` plus modifier flags;
            // the shell has already resolved modifiers into the AppKey, so
            // these are unmodified navigation/edit keys.
            other => self.handle_key(other.name(), false, false),
        }
    }
}

impl AppContentProvider for EditorRuntime {
    fn content_view(&self, _cols: u32, rows: u32) -> AppContentView {
        let mut view = AppContentView::new(ContentKind::Document);
        view.title = self
            .active_document()
            .map(|d| d.display_title())
            .or(Some("untitled".to_string()));

        let visible_rows = if rows == 0 { 256 } else { rows as usize };
        for line in self.visible_lines(0, visible_rows) {
            let spans = line
                .highlights
                .iter()
                .filter_map(|t| {
                    token_color(t.kind).map(|color| ContentSpan {
                        start_col: t.start as u32,
                        end_col: t.end() as u32,
                        color: Some(color),
                        bold: matches!(t.kind, TokenKind::Keyword | TokenKind::Type),
                    })
                })
                .collect();
            view.rows.push(ContentRow {
                text: line.text,
                spans,
                gutter: Some(line.number.to_string()),
                active: line.is_current,
            });
        }
        let (line, col) = self.cursor_position();
        view.cursor = Some((line as u32, col as u32));
        view
    }
}

impl AppView for EditorRuntime {
    fn app_id(&self) -> &str {
        crate::APP_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EditorConfig;

    fn editor_with_doc() -> EditorRuntime {
        let mut rt = EditorRuntime::new(EditorConfig::default());
        rt.new_document();
        rt
    }

    #[test]
    fn typed_text_routes_into_buffer() {
        let mut rt = editor_with_doc();
        let view: &mut dyn AppView = &mut rt;
        assert!(view.handle_text("hello"));
        let content = view.content_view(80, 24);
        assert_eq!(content.rows[0].text, "hello");
        // Cursor advanced to column 5.
        assert_eq!(content.cursor, Some((0, 5)));
    }

    #[test]
    fn newline_in_text_splits_lines() {
        let mut rt = editor_with_doc();
        assert!(rt.handle_text("ab\ncd"));
        let content = rt.content_view(80, 24);
        assert!(content.rows.len() >= 2, "expected 2 lines, got {}", content.rows.len());
        assert_eq!(content.rows[0].text, "ab");
        assert_eq!(content.rows[1].text, "cd");
    }

    #[test]
    fn content_view_is_non_placeholder_document() {
        // Structural proof against a fresh typed document: a real multi-line
        // surface with a per-line gutter and title (vs. the old single Label).
        let mut rt = editor_with_doc();
        rt.handle_text("alpha\nbeta");
        let content = rt.content_view(80, 24);
        assert_eq!(content.kind, ContentKind::Document);
        assert!(content.title.is_some());
        assert!(content.rows.len() >= 2);
        assert_eq!(content.rows[0].gutter.as_deref(), Some("1"));
        assert_eq!(content.rows[1].gutter.as_deref(), Some("2"));
    }

    #[test]
    fn known_language_produces_highlight_spans() {
        // Opening a .rs file gives the highlighter a language, so keywords get
        // colored spans — proving the styling path, not just plain text.
        let mut rt = EditorRuntime::new(EditorConfig::default());
        rt.open_file("main.rs", "fn main() {}");
        let content = rt.content_view(80, 24);
        assert!(
            !content.rows[0].spans.is_empty(),
            "expected highlighted spans for Rust source"
        );
    }

    #[test]
    fn backspace_key_routes() {
        let mut rt = editor_with_doc();
        rt.handle_text("xy");
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.content_view(0, 0).rows[0].text, "x");
    }
}
