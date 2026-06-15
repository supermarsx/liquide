//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! file manager, plus the real content view that replaces the `Label`
//! placeholder.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
};

use crate::runtime::FilesRuntime;

impl AppTextInput for FilesRuntime {
    fn handle_text(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let mut q = self.search_query().to_string();
        q.push_str(text);
        self.set_search_query(q);
        true
    }

    fn handle_key(&mut self, key: &AppKey) -> bool {
        match key {
            AppKey::Char(c) => {
                let mut q = self.search_query().to_string();
                q.push(*c);
                self.set_search_query(q);
                true
            }
            AppKey::Backspace => {
                let mut q = self.search_query().to_string();
                let removed = q.pop().is_some();
                self.set_search_query(q);
                removed
            }
            _ => false,
        }
    }
}

impl AppContentProvider for FilesRuntime {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        let listing = self.current_listing();
        let mut view = AppContentView::new(ContentKind::List);
        view.title = Some(listing.path.clone());

        // A search-bar row appears at the top only while a query is being typed.
        let query = self.search_query();
        if !query.is_empty() {
            let mut row = ContentRow::plain(format!("Search: {query}"));
            row.active = true;
            view.rows.push(row);
        }

        // One row per entry; directories get an ascii "[DIR] " prefix, files an
        // equal-width blank prefix so the names line up in a list view.
        for entry in &listing.entries {
            let prefix = if entry.is_dir() { "[DIR] " } else { "      " };
            view.rows
                .push(ContentRow::plain(format!("{prefix}{}", entry.name)));
        }

        view.cursor = None;
        view
    }
}

impl AppView for FilesRuntime {
    fn app_id(&self) -> &str {
        crate::FILES_APP_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FilesConfig;
    use crate::entry::FileEntry;

    /// A runtime with a small, deterministic listing (a dir and a file).
    fn runtime_with_entries() -> FilesRuntime {
        let mut rt = FilesRuntime::new(FilesConfig::default());
        let entries = vec![
            FileEntry::directory("src".to_string(), "/proj/src".to_string(), 0),
            FileEntry::file("readme.md".to_string(), "/proj/readme.md".to_string(), 42, 0),
        ];
        rt.navigate("/proj".to_string(), entries);
        rt
    }

    #[test]
    fn typed_text_routes_into_search_query_and_view() {
        let mut rt = runtime_with_entries();
        // Exercise the seam through a trait object to prove object-safety.
        let view: &mut dyn AppView = &mut rt;
        assert!(view.handle_text("ab"));
        assert!(view.handle_key(&AppKey::Char('c')));
        assert_eq!(rt.search_query(), "abc");

        let content = rt.content_view(80, 24);
        let joined: String = content
            .rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("Search: abc"),
            "typed text not visible in content view: {joined:?}"
        );
    }

    #[test]
    fn backspace_removes_a_char() {
        let mut rt = runtime_with_entries();
        assert!(rt.handle_text("hi"));
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.search_query(), "h");
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.search_query(), "");
        // Backspace on an empty buffer reports no change.
        assert!(!AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        // Non-text keys are ignored.
        assert!(!AppTextInput::handle_key(&mut rt, &AppKey::Enter));
        assert!(!rt.handle_text(""));
    }

    #[test]
    fn content_view_is_non_placeholder_list() {
        let rt = runtime_with_entries();
        let view: &dyn AppView = &rt;
        let content = view.content_view(80, 24);
        assert_eq!(content.kind, ContentKind::List);
        assert!(!content.is_empty());
        assert_eq!(content.title.as_deref(), Some("/proj"));
        // One row per entry (no search bar when the query is empty).
        assert_eq!(content.rows.len(), 2);
        assert!(content.rows[0].text.starts_with("[DIR] "));
        assert!(content.rows[0].text.contains("src"));
        assert!(content.rows[1].text.contains("readme.md"));
        assert_eq!(view.app_id(), crate::FILES_APP_ID);
    }
}
