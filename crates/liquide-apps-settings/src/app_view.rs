//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! settings application, plus the real list content view that mirrors the live
//! search buffer and category list.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
};

use crate::runtime::SettingsRuntime;

impl AppTextInput for SettingsRuntime {
    fn handle_text(&mut self, text: &str) -> bool {
        // Append typed text into the live search buffer and re-run the search.
        self.push_search_text(text)
    }

    fn handle_key(&mut self, key: &AppKey) -> bool {
        match key {
            AppKey::Char(c) => self.push_search_text(&c.to_string()),
            AppKey::Backspace => self.pop_search_char(),
            _ => false,
        }
    }
}

impl AppContentProvider for SettingsRuntime {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        let mut view = AppContentView::new(ContentKind::List);
        view.title = Some("Settings".to_string());

        let query = self.search_query();
        if query.is_empty() {
            // No active search: list every category by its display label.
            for info in self.category_infos() {
                view.rows
                    .push(ContentRow::plain(info.category.label()));
            }
        } else {
            // Active search: show the query line followed by one row per hit.
            view.rows.push(ContentRow {
                text: format!("Search: {query}"),
                spans: Vec::new(),
                gutter: None,
                active: true,
            });
            for result in self.search_results() {
                view.rows.push(ContentRow::plain(result.label.clone()));
            }
        }

        view.cursor = None;
        view
    }
}

impl AppView for SettingsRuntime {
    fn app_id(&self) -> &str {
        crate::SETTINGS_APP_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SettingsConfig;

    fn runtime() -> SettingsRuntime {
        SettingsRuntime::new(SettingsConfig::default())
    }

    #[test]
    fn typed_text_routes_into_search_query_and_content_view() {
        let mut rt = runtime();
        assert!(rt.handle_text("dis"));
        assert_eq!(rt.search_query(), "dis");

        let view = rt.content_view(80, 24);
        assert_eq!(view.kind, ContentKind::List);
        let joined: String = view
            .rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("Search: dis"),
            "query not visible in content view: {joined:?}"
        );
    }

    #[test]
    fn backspace_removes_a_char() {
        let mut rt = runtime();
        assert!(rt.handle_text("abc"));
        assert_eq!(rt.search_query(), "abc");
        // Disambiguate against any inherent method of the same name.
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.search_query(), "ab");
    }

    #[test]
    fn backspace_on_empty_returns_false() {
        let mut rt = runtime();
        assert!(!AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.search_query(), "");
    }

    #[test]
    fn default_content_view_lists_categories() {
        let rt = runtime();
        let view = rt.content_view(80, 24);
        assert_eq!(view.kind, ContentKind::List);
        assert_eq!(view.title.as_deref(), Some("Settings"));
        assert!(view.cursor.is_none());
        // One row per category.
        assert_eq!(view.rows.len(), crate::category::Category::ALL.len());
        let joined: String = view
            .rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Display"), "categories missing: {joined:?}");
    }

    #[test]
    fn object_safe_via_dyn_app_view() {
        let mut rt = runtime();
        let view: &mut dyn AppView = &mut rt;
        assert!(view.handle_text("x"));
        let content = view.content_view(80, 24);
        assert_eq!(content.kind, ContentKind::List);
        assert_eq!(view.app_id(), crate::SETTINGS_APP_ID);
    }
}
