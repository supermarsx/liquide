//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! software center, exposing the package catalog as list content rows.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
};

use crate::runtime::SoftwareCenterRuntime;

impl AppTextInput for SoftwareCenterRuntime {
    fn handle_text(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        self.search_query_mut().push_str(text);
        true
    }

    fn handle_key(&mut self, key: &AppKey) -> bool {
        match key {
            AppKey::Char(c) => {
                self.search_query_mut().push(*c);
                true
            }
            AppKey::Backspace => self.search_query_mut().pop().is_some(),
            _ => false,
        }
    }
}

impl AppContentProvider for SoftwareCenterRuntime {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        let mut view = AppContentView::new(ContentKind::List);
        view.title = Some("Software Center".to_string());

        let query = self.search_query();
        if !query.is_empty() {
            let mut row = ContentRow::plain(format!("Search: {query}"));
            row.active = true;
            view.rows.push(row);
        }

        let needle = query.to_lowercase();
        let packages = self.catalog().all_packages();
        if packages.is_empty() {
            view.rows.push(ContentRow::plain("No packages available"));
        } else {
            for p in packages {
                if !needle.is_empty()
                    && !p.name.to_lowercase().contains(&needle)
                    && !p.summary.to_lowercase().contains(&needle)
                {
                    continue;
                }
                view.rows
                    .push(ContentRow::plain(format!("{} — {}", p.name, p.summary)));
            }
        }

        view.cursor = None;
        view
    }
}

impl AppView for SoftwareCenterRuntime {
    fn app_id(&self) -> &str {
        crate::APP_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SoftwareCenterConfig;
    use crate::package::{AppCategory, License, PackageInfo, Version};

    fn runtime() -> SoftwareCenterRuntime {
        SoftwareCenterRuntime::new(SoftwareCenterConfig::default())
    }

    fn sample_package(id: &str, name: &str, summary: &str) -> PackageInfo {
        PackageInfo {
            id: id.to_string(),
            name: name.to_string(),
            summary: summary.to_string(),
            description: String::new(),
            version: Version::new(1, 0, 0),
            category: AppCategory::Other,
            license: License::OpenSource,
            developer: String::new(),
            homepage: String::new(),
            download_size: 0,
            installed_size: 0,
            screenshots: Vec::new(),
            icon: String::new(),
            installed: false,
            installed_version: None,
            repository_id: String::new(),
        }
    }

    #[test]
    fn typed_text_routes_into_search_query() {
        let mut rt = runtime();
        assert!(rt.handle_text("fire"));
        assert_eq!(rt.search_query(), "fire");
        let view = rt.content_view(80, 24);
        let joined: String = view.rows.iter().map(|r| r.text.as_str()).collect();
        assert!(joined.contains("Search: fire"), "search row missing: {joined:?}");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut rt = runtime();
        assert!(rt.handle_text("ab"));
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.search_query(), "a");
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert!(!AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.search_query(), "");
    }

    #[test]
    fn content_view_is_list_with_title() {
        let mut rt = runtime();
        rt.load_packages(vec![
            sample_package("a.b", "Alpha", "first tool"),
            sample_package("c.d", "Beta", "second tool"),
        ]);
        let view = rt.content_view(80, 24);
        assert_eq!(view.kind, ContentKind::List);
        assert_eq!(view.title.as_deref(), Some("Software Center"));
        let joined: String = view.rows.iter().map(|r| r.text.as_str()).collect();
        assert!(joined.contains("Alpha — first tool"), "package row missing: {joined:?}");
    }

    #[test]
    fn object_safe_via_dyn_app_view() {
        let mut rt = runtime();
        let view: &mut dyn AppView = &mut rt;
        assert_eq!(view.app_id(), crate::APP_ID);
        assert!(view.handle_text("x"));
        let content = view.content_view(80, 24);
        assert_eq!(content.kind, ContentKind::List);
    }
}
