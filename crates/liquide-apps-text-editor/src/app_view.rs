//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! text editor, plus the real content view that replaces the `Label`
//! placeholder.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, AppWidget, AppWidgetAction,
    AppWidgetModel, ButtonKind, ContentKind, ContentRow, ContentSpan,
};

use crate::cursor::Position;
use crate::runtime::EditorRuntime;
use crate::syntax::TokenKind;

/// Stable widget id for the "New" toolbar button.
const NEW_BUTTON_ID: &str = "editor.new";
/// Stable widget id for the "Open" toolbar button.
const OPEN_BUTTON_ID: &str = "editor.open";
/// Stable widget id for the "Save" toolbar button.
const SAVE_BUTTON_ID: &str = "editor.save";
/// Stable widget key for the document body text field.
const DOCUMENT_KEY: &str = "document";

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

impl EditorRuntime {
    /// Build the toolkit-free widget model from the live runtime: a toolbar
    /// (new / open / save) over a multi-line text field bound to the active
    /// document buffer (key [`DOCUMENT_KEY`]).
    ///
    /// The document body is exposed as an [`AppWidget::TextArea`] whose `value`
    /// is the *entire* buffer text (lines joined by `\n`) and whose line-number
    /// gutter is enabled. The shell maps this to the real multi-line
    /// `liquide_widgets::TextArea` (with a per-line gutter and an in-flow caret),
    /// so the editor body is a genuine multi-line editor rather than the
    /// single-line `TextInput` fallback it used previously. A `change` action
    /// carries the full edited text back through [`set_document_text`], which
    /// preserves newlines.
    fn build_widget_model(&self) -> AppWidgetModel {
        let title = self
            .active_document()
            .map(|d| d.display_title())
            .unwrap_or_else(|| "untitled".to_string());

        let toolbar = AppWidget::Toolbar {
            children: vec![
                AppWidget::Button {
                    id: NEW_BUTTON_ID.to_string(),
                    label: "New".to_string(),
                    kind: ButtonKind::Normal,
                },
                AppWidget::Button {
                    id: OPEN_BUTTON_ID.to_string(),
                    label: "Open".to_string(),
                    kind: ButtonKind::Normal,
                },
                AppWidget::Button {
                    id: SAVE_BUTTON_ID.to_string(),
                    label: "Save".to_string(),
                    kind: ButtonKind::Primary,
                },
            ],
        };

        // The document body as a multi-line editor carrying the whole buffer.
        let body_text = self
            .active_document()
            .map(|d| d.buffer.text())
            .unwrap_or_default();
        let body = AppWidget::TextArea {
            key: DOCUMENT_KEY.to_string(),
            value: body_text,
            gutter: true,
            readonly: false,
        };

        AppWidgetModel {
            title: Some(title),
            root: vec![toolbar, body],
        }
    }

    /// Replace the active document's buffer with `text`, recording the change so
    /// the buffer is marked modified and the cursor is clamped into range.
    ///
    /// Returns `true` when the buffer content actually changed.
    fn set_document_text(&mut self, text: &str) -> bool {
        let Some(doc) = self.active_document_mut() else {
            return false;
        };
        if doc.buffer.text() == text {
            return false;
        }
        doc.buffer = crate::buffer::TextBuffer::from_text(text);
        // from_text starts unmodified; an edit through the seam IS a modification.
        doc.buffer.mark_modified();
        // Clamp the primary cursor to the (possibly shorter) new buffer.
        let last = doc.buffer.line_count().saturating_sub(1);
        let len = doc.buffer.line_len(last);
        doc.cursors.primary_mut().move_to(Position::new(last, len));
        doc.gutter.update_width(doc.buffer.line_count());
        true
    }

    /// Apply a host-delivered widget action, returning `true` when the runtime
    /// state changed.
    fn apply_widget_action(&mut self, action: &AppWidgetAction) -> bool {
        match action.widget.as_str() {
            // Toolbar: New opens a fresh empty document.
            NEW_BUTTON_ID if action.name == "click" => {
                self.new_document();
                true
            }
            // Toolbar: Open — without a platform file picker in this crate, opening
            // surfaces a fresh empty document (the shell owns the real chooser).
            OPEN_BUTTON_ID if action.name == "click" => {
                self.new_document();
                true
            }
            // Toolbar: Save persists the active document if it has a path. A
            // path-less (Untitled) document cannot be saved here (no chooser),
            // so report no change rather than faking success.
            SAVE_BUTTON_ID if action.name == "click" => self.save_active().is_ok(),
            // The document body field: a `change` carries the new full text.
            DOCUMENT_KEY if action.name == "change" => self.set_document_text(&action.payload),
            _ => false,
        }
    }
}

impl AppView for EditorRuntime {
    fn app_id(&self) -> &str {
        crate::APP_ID
    }

    fn widget_model(&self) -> Option<AppWidgetModel> {
        Some(self.build_widget_model())
    }

    fn apply_action(&mut self, action: &AppWidgetAction) -> bool {
        self.apply_widget_action(action)
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

    // ---- widget seam ------------------------------------------------------

    use liquide_interop::{AppWidget, AppWidgetAction, AppWidgetModel};

    /// Depth-first find of a keyed/ided widget anywhere in the model.
    fn find<'a>(model: &'a AppWidgetModel, key: &str) -> Option<&'a AppWidget> {
        fn walk<'a>(w: &'a AppWidget, key: &str) -> Option<&'a AppWidget> {
            if w.key() == Some(key) {
                return Some(w);
            }
            match w {
                AppWidget::Panel { children }
                | AppWidget::Card { children, .. }
                | AppWidget::GroupBox { children, .. }
                | AppWidget::Toolbar { children } => children.iter().find_map(|c| walk(c, key)),
                _ => None,
            }
        }
        model.root.iter().find_map(|w| walk(w, key))
    }

    #[test]
    fn default_widget_model_is_some_not_the_trait_default_none() {
        // Guards against regressing back to the AppView default (None): the
        // editor must opt into the widget seam.
        let rt = editor_with_doc();
        let view: &dyn AppView = &rt;
        assert!(
            view.widget_model().is_some(),
            "text editor must opt into the widget seam"
        );
    }

    #[test]
    fn widget_model_has_toolbar_buttons() {
        let rt = editor_with_doc();
        let model = rt.widget_model().expect("model");
        // New / Open / Save are present in the toolbar.
        assert!(matches!(
            find(&model, NEW_BUTTON_ID),
            Some(AppWidget::Button { .. })
        ));
        assert!(matches!(
            find(&model, OPEN_BUTTON_ID),
            Some(AppWidget::Button { .. })
        ));
        assert!(matches!(
            find(&model, SAVE_BUTTON_ID),
            Some(AppWidget::Button { .. })
        ));
    }

    #[test]
    fn widget_model_document_field_is_a_multiline_textarea_reflecting_buffer_text() {
        // The buffer's current (multi-line) text must appear in the document
        // body, and that body must be a multi-line TextArea (with a gutter), NOT
        // the single-line TextInput it used to fall back to.
        let mut rt = editor_with_doc();
        rt.handle_text("alpha\nbeta");
        let model = rt.widget_model().expect("model");
        let body = find(&model, DOCUMENT_KEY).expect("document field present");
        // Must NOT regress to a single-line TextInput.
        assert!(
            !matches!(body, AppWidget::TextInput { .. }),
            "document body must not be a single-line TextInput, got {body:?}"
        );
        // Must be a TextArea carrying the full newline-bearing buffer text.
        assert!(
            matches!(
                body,
                AppWidget::TextArea { value, gutter: true, .. } if value == "alpha\nbeta"
            ),
            "document field must be a multi-line TextArea mirroring the buffer, got {body:?}"
        );
    }

    #[test]
    fn widget_model_title_tracks_active_document() {
        let mut rt = editor_with_doc();
        // A fresh document is "Untitled"; the title reflects it.
        let model = rt.widget_model().expect("model");
        assert_eq!(model.title.as_deref(), Some("Untitled"));
        // After an edit the document is modified → title carries the marker.
        rt.handle_text("x");
        let model = rt.widget_model().expect("model");
        assert_eq!(model.title.as_deref(), Some("Untitled *"));
    }

    #[test]
    fn apply_action_change_updates_the_buffer() {
        // A change action carrying new text must mutate the real buffer.
        let mut rt = editor_with_doc();
        rt.handle_text("old");
        let changed = rt.apply_action(&AppWidgetAction::new(DOCUMENT_KEY, "change", "new text"));
        assert!(changed, "document change must report a change");
        assert_eq!(
            rt.active_document().unwrap().buffer.text(),
            "new text",
            "the change action must replace the buffer content"
        );
        // The next model reflects it.
        let model = rt.widget_model().expect("model");
        assert!(matches!(
            find(&model, DOCUMENT_KEY),
            Some(AppWidget::TextArea { value, .. }) if value == "new text"
        ));
    }

    #[test]
    fn apply_action_change_handles_multiline_payload() {
        let mut rt = editor_with_doc();
        let changed =
            rt.apply_action(&AppWidgetAction::new(DOCUMENT_KEY, "change", "line1\nline2\nline3"));
        assert!(changed);
        let doc = rt.active_document().unwrap();
        assert_eq!(doc.buffer.line_count(), 3);
        assert_eq!(doc.buffer.line(1), Some("line2"));
        // The replacement marks the document modified.
        assert!(doc.is_modified());
        // The re-emitted model carries the full multi-line text in a TextArea —
        // the newline is preserved across the apply→re-render round-trip.
        let model = rt.widget_model().expect("model");
        assert!(
            matches!(
                find(&model, DOCUMENT_KEY),
                Some(AppWidget::TextArea { value, .. }) if value == "line1\nline2\nline3"
            ),
            "the document TextArea must carry the full multi-line edit"
        );
    }

    #[test]
    fn apply_action_change_to_same_text_is_a_no_op() {
        let mut rt = editor_with_doc();
        rt.handle_text("same");
        let changed = rt.apply_action(&AppWidgetAction::new(DOCUMENT_KEY, "change", "same"));
        assert!(!changed, "an identical change must report no change");
    }

    #[test]
    fn apply_action_new_button_opens_a_document() {
        let mut rt = editor_with_doc();
        rt.handle_text("content");
        let before = rt.document_count();
        let changed = rt.apply_action(&AppWidgetAction::new(NEW_BUTTON_ID, "click", ""));
        assert!(changed, "New must report a change");
        assert_eq!(rt.document_count(), before + 1);
        // The fresh active document is empty.
        assert_eq!(rt.active_document().unwrap().buffer.text(), "");
    }

    #[test]
    fn apply_action_save_without_path_reports_no_change() {
        // An Untitled (path-less) document cannot be saved without a chooser, so
        // the Save action honestly reports no change rather than faking success.
        let mut rt = editor_with_doc();
        rt.handle_text("unsaved");
        let changed = rt.apply_action(&AppWidgetAction::new(SAVE_BUTTON_ID, "click", ""));
        assert!(!changed, "save of a path-less doc must report no change");
    }

    #[test]
    fn apply_action_save_persists_to_path() {
        use std::io::Write;
        // Open a real temp file so the active document has a path, then edit and
        // save it through the widget action; the file content reflects the buffer.
        let dir = std::env::temp_dir();
        let path = dir.join(format!("liquide_editor_seam_{}.txt", std::process::id()));
        {
            let mut f = std::fs::File::create(&path).expect("create temp file");
            writeln!(f, "original").expect("write");
        }
        let mut rt = EditorRuntime::new(EditorConfig::default());
        rt.open_path(&path).expect("open path");
        rt.apply_action(&AppWidgetAction::new(DOCUMENT_KEY, "change", "edited line"));
        let changed = rt.apply_action(&AppWidgetAction::new(SAVE_BUTTON_ID, "click", ""));
        assert!(changed, "save with a path must succeed");
        let on_disk = std::fs::read_to_string(&path).expect("read back");
        assert!(
            on_disk.contains("edited line"),
            "saved file must contain the edited buffer, got {on_disk:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn apply_action_unknown_widget_is_a_no_op() {
        let mut rt = editor_with_doc();
        rt.handle_text("keep");
        let changed = rt.apply_action(&AppWidgetAction::new("does.not.exist", "click", ""));
        assert!(!changed);
        assert_eq!(rt.active_document().unwrap().buffer.text(), "keep");
    }
}
