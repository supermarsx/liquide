//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! software center, exposing the package catalog as list content rows.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, AppWidget, AppWidgetAction,
    AppWidgetModel, ButtonKind, ContentKind, ContentRow, SelectionMode,
};

use crate::runtime::SoftwareCenterRuntime;

/// Stable widget key for the search field.
const SEARCH_KEY: &str = "search";
/// Stable widget key for the package list.
const APP_LIST_KEY: &str = "apps";
/// Stable widget id for the install/open action button.
const ACTION_BUTTON_ID: &str = "action";

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

impl SoftwareCenterRuntime {
    /// Build the toolkit-free widget model from live runtime state: a search
    /// field, a list of the packages matching the query (with the selected one
    /// highlighted), and an install/open button for the current selection.
    fn build_widget_model(&self) -> AppWidgetModel {
        let search = AppWidget::TextInput {
            key: SEARCH_KEY.to_string(),
            value: self.search_query().to_string(),
        };

        let visible = self.visible_packages();
        let items: Vec<String> = visible
            .iter()
            .map(|p| format!("{} — {}", p.name, p.summary))
            .collect();
        // The selected index is the position of the selected id *within the
        // currently-visible list* (a filtered-out selection shows nothing).
        let selected: Vec<u32> = self
            .selected_id()
            .and_then(|id| visible.iter().position(|p| p.id == id))
            .map(|i| i as u32)
            .into_iter()
            .collect();
        let list = AppWidget::List {
            key: APP_LIST_KEY.to_string(),
            items,
            selection_mode: SelectionMode::Single,
            selected,
        };

        let mut children = vec![search, list];

        // Per-selection action button: "Open" when installed, otherwise "Install".
        if let Some(pkg) = self
            .selected_id()
            .and_then(|id| self.catalog().find(id))
        {
            let (label, kind) = if pkg.installed {
                ("Open".to_string(), ButtonKind::Normal)
            } else {
                (format!("Install {}", pkg.name), ButtonKind::Primary)
            };
            children.push(AppWidget::Button {
                id: ACTION_BUTTON_ID.to_string(),
                label,
                kind,
            });
        }

        // Fill layout: a single root Panel that fills the window and stacks the
        // search field, the package LIST (the main content, which grows to fill
        // the remaining width/height — widgets.css `app-content-body lq-panel >
        // lq-list`), and the per-selection action button. So the catalog fills
        // the frame instead of a small cluster at the top-left.
        let root = AppWidget::Panel { children };

        AppWidgetModel {
            title: Some("Software Center".to_string()),
            root: vec![root],
        }
    }

    /// Apply a host-delivered widget action, returning `true` when the runtime
    /// state changed.
    fn apply_widget_action(&mut self, action: &AppWidgetAction) -> bool {
        match action.widget.as_str() {
            // Search field: replace the query (this re-filters the list).
            SEARCH_KEY => {
                if self.search_query() == action.payload {
                    return false;
                }
                self.set_search_query(action.payload.clone());
                true
            }
            // List selection: payload is the index into the *visible* list.
            APP_LIST_KEY => {
                let Ok(idx) = action.payload.parse::<usize>() else {
                    return false;
                };
                let Some(id) = self
                    .visible_packages()
                    .get(idx)
                    .map(|p| p.id.clone())
                else {
                    return false;
                };
                if self.selected_id() == Some(id.as_str()) {
                    return false;
                }
                self.select_package(&id);
                true
            }
            // Action button: install the selected package (open is a host concern;
            // an already-installed package install is a no-op).
            ACTION_BUTTON_ID => {
                let Some(id) = self.selected_id().map(str::to_string) else {
                    return false;
                };
                self.install(&id).is_ok()
            }
            _ => false,
        }
    }
}

impl AppView for SoftwareCenterRuntime {
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

    // ---- widget seam ------------------------------------------------------

    use liquide_interop::AppWidgetModel;

    fn loaded_runtime() -> SoftwareCenterRuntime {
        let mut rt = runtime();
        rt.load_packages(vec![
            sample_package("org.gimp", "GIMP", "image editor"),
            sample_package("org.firefox", "Firefox", "web browser"),
            sample_package("org.vlc", "VLC", "media player"),
        ]);
        rt
    }

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
    fn widget_model_lists_packages_and_search_field() {
        let rt = loaded_runtime();
        let model = rt.widget_model().expect("software center exposes a widget model");

        assert!(matches!(
            find(&model, SEARCH_KEY),
            Some(AppWidget::TextInput { value, .. }) if value.is_empty()
        ));
        match find(&model, APP_LIST_KEY) {
            Some(AppWidget::List { items, selected, .. }) => {
                assert_eq!(items.len(), 3, "all packages listed");
                assert!(items.iter().any(|i| i.contains("GIMP")));
                assert!(selected.is_empty(), "nothing selected initially");
            }
            other => panic!("expected a List, got {other:?}"),
        }
        // No action button without a selection.
        assert!(find(&model, ACTION_BUTTON_ID).is_none());
    }

    #[test]
    fn widget_model_reflects_selection_with_install_button() {
        let mut rt = loaded_runtime();
        rt.select_package("org.firefox");
        let model = rt.widget_model().expect("model");

        // The list marks the selected index (Firefox is index 1).
        match find(&model, APP_LIST_KEY) {
            Some(AppWidget::List { selected, .. }) => assert_eq!(selected, &vec![1u32]),
            other => panic!("expected a List, got {other:?}"),
        }
        // A primary install button appears for the (uninstalled) selection.
        match find(&model, ACTION_BUTTON_ID) {
            Some(AppWidget::Button { label, kind, .. }) => {
                assert!(label.contains("Install"), "expected install label, got {label:?}");
                assert_eq!(*kind, ButtonKind::Primary);
            }
            other => panic!("expected a Button, got {other:?}"),
        }
    }

    #[test]
    fn apply_action_search_filters_the_list() {
        let mut rt = loaded_runtime();
        let changed = rt.apply_action(&AppWidgetAction::new(SEARCH_KEY, "change", "fire"));
        assert!(changed, "search action must report a change");
        assert_eq!(rt.search_query(), "fire");

        let model = rt.widget_model().expect("model");
        match find(&model, APP_LIST_KEY) {
            Some(AppWidget::List { items, .. }) => {
                assert_eq!(items.len(), 1, "only Firefox matches 'fire'");
                assert!(items[0].contains("Firefox"));
            }
            other => panic!("expected a List, got {other:?}"),
        }
    }

    #[test]
    fn apply_action_select_changes_selection() {
        let mut rt = loaded_runtime();
        // Select index 2 (VLC).
        let changed = rt.apply_action(&AppWidgetAction::new(APP_LIST_KEY, "select", "2"));
        assert!(changed, "list select must report a change");
        assert_eq!(rt.selected_id(), Some("org.vlc"));
    }

    #[test]
    fn apply_action_select_index_is_relative_to_the_filtered_list() {
        let mut rt = loaded_runtime();
        // Filter so only Firefox remains, then select index 0.
        assert!(rt.apply_action(&AppWidgetAction::new(SEARCH_KEY, "change", "fire")));
        assert!(rt.apply_action(&AppWidgetAction::new(APP_LIST_KEY, "select", "0")));
        assert_eq!(
            rt.selected_id(),
            Some("org.firefox"),
            "index must map through the filtered (not full) list"
        );
    }

    #[test]
    fn apply_action_install_button_enqueues_install() {
        let mut rt = loaded_runtime();
        rt.select_package("org.gimp");
        assert_eq!(rt.queue().count(), 0);
        let changed = rt.apply_action(&AppWidgetAction::new(ACTION_BUTTON_ID, "click", ""));
        assert!(changed, "install button click must report a change");
        assert_eq!(rt.queue().count(), 1, "an install op was enqueued");
    }

    #[test]
    fn apply_action_is_a_no_op_for_unknown_widget() {
        let mut rt = loaded_runtime();
        rt.select_package("org.gimp");
        let before_query = rt.search_query().to_string();
        let before_sel = rt.selected_id().map(str::to_string);
        let changed = rt.apply_action(&AppWidgetAction::new("nope", "click", ""));
        assert!(!changed);
        assert_eq!(rt.search_query(), before_query);
        assert_eq!(rt.selected_id().map(str::to_string), before_sel);
        assert_eq!(rt.queue().count(), 0);
    }

    #[test]
    fn default_widget_model_is_some_not_the_trait_default_none() {
        let rt = loaded_runtime();
        assert!(
            rt.widget_model().is_some(),
            "software center must opt into the widget seam"
        );
    }
}
