//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! task manager, exposing the live process list as list content rows.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
};

use crate::runtime::TaskManagerRuntime;

impl AppTextInput for TaskManagerRuntime {
    fn handle_text(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        self.filter_query_mut().push_str(text);
        true
    }

    fn handle_key(&mut self, key: &AppKey) -> bool {
        match key {
            AppKey::Char(c) => {
                self.filter_query_mut().push(*c);
                true
            }
            AppKey::Backspace => self.filter_query_mut().pop().is_some(),
            _ => false,
        }
    }
}

impl AppContentProvider for TaskManagerRuntime {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        let mut view = AppContentView::new(ContentKind::List);
        view.title = Some("Task Manager".to_string());

        let query = self.filter_query();
        if !query.is_empty() {
            let mut row = ContentRow::plain(format!("Filter: {query}"));
            row.active = true;
            view.rows.push(row);
        }

        let needle = query.to_lowercase();
        let processes = self.visible_processes();
        if processes.is_empty() {
            view.rows.push(ContentRow::plain("No processes sampled"));
        } else {
            for p in processes {
                if !needle.is_empty() && !p.name.to_lowercase().contains(&needle) {
                    continue;
                }
                view.rows.push(ContentRow::plain(format!(
                    "{:>6}  {:>5.1}%  {}",
                    p.pid, p.cpu_percent, p.name
                )));
            }
        }

        view.cursor = None;
        view
    }
}

impl AppView for TaskManagerRuntime {
    fn app_id(&self) -> &str {
        crate::APP_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TaskManagerConfig;

    fn runtime() -> TaskManagerRuntime {
        TaskManagerRuntime::new(TaskManagerConfig::default())
    }

    #[test]
    fn typed_text_routes_into_filter_query() {
        let mut rt = runtime();
        assert!(rt.handle_text("note"));
        assert_eq!(rt.filter_query(), "note");
        let view = rt.content_view(80, 24);
        let joined: String = view.rows.iter().map(|r| r.text.as_str()).collect();
        assert!(joined.contains("Filter: note"), "filter row missing: {joined:?}");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut rt = runtime();
        assert!(rt.handle_text("ab"));
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.filter_query(), "a");
        // Backspace on empty returns false.
        assert!(AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert!(!AppTextInput::handle_key(&mut rt, &AppKey::Backspace));
        assert_eq!(rt.filter_query(), "");
    }

    #[test]
    fn content_view_is_list_with_title() {
        let rt = runtime();
        let view = rt.content_view(80, 24);
        assert_eq!(view.kind, ContentKind::List);
        assert_eq!(view.title.as_deref(), Some("Task Manager"));
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
