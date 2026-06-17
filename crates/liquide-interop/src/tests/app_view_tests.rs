//! Tests for the shell↔app view seam.

use crate::app_view::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
};
use crate::app_widget::AppWidgetProvider;

/// A tiny model implementing the full seam, used to prove object-safety and the
/// routing contract independent of any real app crate.
#[derive(Default)]
struct StubModel {
    text: String,
}

impl AppTextInput for StubModel {
    fn handle_text(&mut self, text: &str) -> bool {
        self.text.push_str(text);
        !text.is_empty()
    }

    fn handle_key(&mut self, key: &AppKey) -> bool {
        match key {
            AppKey::Char(c) => {
                self.text.push(*c);
                true
            }
            AppKey::Backspace => self.text.pop().is_some(),
            _ => false,
        }
    }
}

impl AppContentProvider for StubModel {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        let mut v = AppContentView::new(ContentKind::Document);
        v.rows.push(ContentRow::plain(self.text.clone()));
        v
    }
}

// An un-migrated app: it satisfies the widget seam purely through the default
// methods (model `None`, `apply_action` `false`) and keeps the text path.
impl AppWidgetProvider for StubModel {}

impl AppView for StubModel {
    fn app_id(&self) -> &str {
        "com.liquide.test.stub"
    }
}

#[test]
fn app_view_is_object_safe_and_routes_text() {
    let mut view: Box<dyn AppView> = Box::new(StubModel::default());
    assert!(view.handle_text("ab"));
    assert!(view.handle_key(&AppKey::Char('c')));
    let content = view.content_view(80, 24);
    assert_eq!(content.rows[0].text, "abc");
    assert_eq!(view.app_id(), "com.liquide.test.stub");
}

#[test]
fn backspace_routes_through_key() {
    let mut model = StubModel::default();
    model.handle_text("hi");
    assert!(model.handle_key(&AppKey::Backspace));
    assert_eq!(model.content_view(0, 0).rows[0].text, "h");
}

#[test]
fn empty_text_reports_no_change() {
    let mut model = StubModel::default();
    assert!(!model.handle_text(""));
}

#[test]
fn appkey_name_maps_to_str_protocol() {
    assert_eq!(AppKey::Enter.name(), "Enter");
    assert_eq!(AppKey::Left.name(), "ArrowLeft");
    assert_eq!(AppKey::Named("F5".into()).name(), "F5");
}

#[test]
fn unmigrated_app_view_keeps_text_path_and_has_no_widget_model() {
    use crate::app_widget::AppWidgetAction;
    let mut view: Box<dyn AppView> = Box::new(StubModel::default());
    // Widget seam reachable through the trait object, defaults to no model.
    assert!(view.widget_model().is_none());
    assert!(!view.apply_action(&AppWidgetAction::new("x", "click", "")));
    // Text path still works.
    assert!(view.handle_text("hi"));
    assert_eq!(view.content_view(80, 24).rows[0].text, "hi");
}

#[test]
fn empty_content_view_is_flagged() {
    let empty = AppContentView::new(ContentKind::List);
    assert!(empty.is_empty());
    let mut full = AppContentView::new(ContentKind::List);
    full.rows.push(ContentRow::plain("x"));
    assert!(!full.is_empty());
}
