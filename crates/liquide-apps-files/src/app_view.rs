//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! file manager, plus the real content view that replaces the `Label`
//! placeholder and the real widget UI exposed through the `AppWidgetModel`
//! seam (a places sidebar, a navigation toolbar, a breadcrumb of the current
//! path, and a multi-select table of the directory listing).

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, AppWidget, AppWidgetAction,
    AppWidgetModel, ContentKind, ContentRow, SelectionMode, SortDirection, TableColumn, TableSort,
};

use crate::config::SortField;
use crate::runtime::FilesRuntime;

// ---- stable widget keys ----------------------------------------------------

/// The places / bookmarks sidebar list.
const PLACES_KEY: &str = "places";
/// The breadcrumb of the current path.
const CRUMBS_KEY: &str = "crumbs";
/// The main directory-listing table.
const LISTING_KEY: &str = "listing";
/// Toolbar button ids.
const BACK_ID: &str = "nav.back";
const FORWARD_ID: &str = "nav.forward";
const UP_ID: &str = "nav.up";
const REFRESH_ID: &str = "nav.refresh";

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

/// Split a path into its breadcrumb segments, returning `(label, full_path)`
/// pairs from the root down to the current directory.
///
/// `"/home/user/docs"` → `[("/", "/"), ("home", "/home"), ("user",
/// "/home/user"), ("docs", "/home/user/docs")]`. A bare virtual path like `"~"`
/// yields a single crumb.
fn breadcrumb_segments(path: &str) -> Vec<(String, String)> {
    // Normalise Windows separators so both `/` and `\` paths split cleanly.
    let normalised = path.replace('\\', "/");
    let trimmed = normalised.trim_end_matches('/');

    // An absolute POSIX path keeps a leading "/" root crumb.
    let is_absolute = normalised.starts_with('/');
    let mut out: Vec<(String, String)> = Vec::new();
    if is_absolute {
        out.push(("/".to_string(), "/".to_string()));
    }

    let mut acc = String::new();
    for seg in trimmed.split('/').filter(|s| !s.is_empty()) {
        if acc.is_empty() && !is_absolute {
            acc.push_str(seg);
        } else if acc == "/" || acc.is_empty() {
            acc = format!("/{seg}");
        } else {
            acc = format!("{acc}/{seg}");
        }
        out.push((seg.to_string(), acc.clone()));
    }

    // Fallback: a path with no usable segments (e.g. empty) still yields one
    // crumb so the breadcrumb is never empty.
    if out.is_empty() {
        out.push((path.to_string(), path.to_string()));
    }
    out
}

/// Format an epoch-seconds timestamp into a compact, locale-free string. `0`
/// (the "unknown" sentinel used by the in-memory entries) renders as a dash.
fn format_modified(modified: u64) -> String {
    if modified == 0 {
        return "--".to_string();
    }
    // Days since the Unix epoch — deterministic and dependency-free; good
    // enough for a stable, sortable column without pulling in chrono.
    let days = modified / 86_400;
    format!("{days}d")
}

/// The listing-table column index for a [`SortField`], matching the column order
/// emitted by `build_widget_model` (`[Name, Size, Modified]`). `Type` is not a
/// visible column here, so it maps to `None`.
fn column_index_for_field(field: SortField) -> Option<u32> {
    match field {
        SortField::Name => Some(0),
        SortField::Size => Some(1),
        SortField::Modified => Some(2),
        SortField::Type => None,
    }
}

/// The [`SortField`] for a clicked listing-table column index (inverse of
/// [`column_index_for_field`]). Out-of-range / non-sortable indices map to
/// `None` so an unknown header click is a no-op rather than a wrong sort.
fn field_for_column_index(col: u32) -> Option<SortField> {
    match col {
        0 => Some(SortField::Name),
        1 => Some(SortField::Size),
        2 => Some(SortField::Modified),
        _ => None,
    }
}

impl FilesRuntime {
    /// Build the toolkit-free widget model from the live runtime state.
    fn build_widget_model(&self) -> AppWidgetModel {
        let listing = self.current_listing();

        // --- places sidebar -------------------------------------------------
        // One item per sidebar bookmark; the active place (the one whose path
        // equals the current directory) is selected.
        let bookmarks = self.sidebar().bookmarks();
        let place_items: Vec<String> = bookmarks.iter().map(|b| b.name.clone()).collect();
        // Each bookmark carries its own icon name (folder-home for Home, folder
        // for Desktop/Documents/Downloads, starred/network-server/… as declared
        // by the places model). Forward them positionally so the sidebar draws a
        // glyph per bookmark; a bookmark with no icon (`None`) stays icon-less.
        let place_icons: Vec<Option<String>> = bookmarks.iter().map(|b| b.icon.clone()).collect();
        let place_selected: Vec<u32> = bookmarks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.path == listing.path)
            .map(|(i, _)| i as u32)
            .collect();
        let places = AppWidget::List {
            key: PLACES_KEY.to_string(),
            items: place_items,
            selection_mode: SelectionMode::Single,
            selected: place_selected,
            icons: place_icons,
        };

        // --- navigation toolbar ---------------------------------------------
        let toolbar = AppWidget::Toolbar {
            children: vec![
                AppWidget::Button {
                    id: BACK_ID.to_string(),
                    label: "Back".to_string(),
                    kind: Default::default(),
                },
                AppWidget::Button {
                    id: FORWARD_ID.to_string(),
                    label: "Forward".to_string(),
                    kind: Default::default(),
                },
                AppWidget::Button {
                    id: UP_ID.to_string(),
                    label: "Up".to_string(),
                    kind: Default::default(),
                },
                AppWidget::Button {
                    id: REFRESH_ID.to_string(),
                    label: "Refresh".to_string(),
                    kind: Default::default(),
                },
            ],
        };

        // --- breadcrumb of the current path ---------------------------------
        let crumbs = AppWidget::Breadcrumb {
            crumbs: breadcrumb_segments(&listing.path)
                .into_iter()
                .map(|(label, _)| label)
                .collect(),
        };

        // --- main directory listing as a multi-select table -----------------
        let rows: Vec<Vec<String>> = listing
            .entries
            .iter()
            .map(|e| {
                let name = if e.is_dir() {
                    format!("{}/", e.name)
                } else {
                    e.name.clone()
                };
                vec![name, e.human_size(), format_modified(e.modified)]
            })
            .collect();
        let selected: Vec<u32> = self.selection().iter().map(|&i| i as u32).collect();
        let table = AppWidget::Table {
            key: LISTING_KEY.to_string(),
            columns: vec![
                TableColumn {
                    label: "Name".to_string(),
                    sortable: true,
                },
                TableColumn {
                    label: "Size".to_string(),
                    sortable: true,
                },
                TableColumn {
                    label: "Modified".to_string(),
                    sortable: true,
                },
            ],
            rows,
            // Reflect the listing's active sort so the header shows the sorted
            // column + direction (and the column index is the canonical contract
            // the shell normalizes a header-click sort payload to — see
            // `apply_listing_action`).
            sort: column_index_for_field(listing.sort_field).map(|column| TableSort {
                column,
                direction: if listing.sort_ascending {
                    SortDirection::Ascending
                } else {
                    SortDirection::Descending
                },
            }),
            selection_mode: SelectionMode::Multiple,
            selected,
        };

        // Fill layout: a horizontal row of [places sidebar | main column].
        // The sidebar keeps its fixed width (a `lq-list`); the main column is a
        // `Panel` that fills the remaining space and stacks the nav toolbar, the
        // breadcrumb, and the FILE TABLE — the table grows to fill the window's
        // width/height (widgets.css `app-content-body …`), so content fills the
        // frame instead of sitting small at the top-left. (A single root
        // container is what the content body stretches; the widgets keep their
        // stable keys so hit-geometry + action routing are unchanged.)
        let main_column = AppWidget::Panel {
            children: vec![toolbar, crumbs, table],
        };
        let body = AppWidget::Toolbar {
            children: vec![places, main_column],
        };

        AppWidgetModel {
            title: Some(format!("Files — {}", listing.path)),
            root: vec![body],
        }
    }

    /// Apply a host-delivered widget action, returning `true` when the runtime
    /// state changed (and the window should be redrawn).
    fn apply_widget_action(&mut self, action: &AppWidgetAction) -> bool {
        match action.widget.as_str() {
            // Places sidebar: navigate to the selected bookmark's path.
            PLACES_KEY => {
                let target = action.payload.parse::<usize>().ok().and_then(|i| {
                    self.sidebar().bookmarks().get(i).map(|b| b.path.clone())
                });
                match target {
                    Some(path) if path != self.current_listing().path => {
                        self.navigate(path, Vec::new());
                        true
                    }
                    _ => false,
                }
            }

            // Breadcrumb: navigate to the clicked crumb's accumulated path.
            CRUMBS_KEY => {
                let segments = breadcrumb_segments(&self.current_listing().path);
                let target = action
                    .payload
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| segments.get(i).map(|(_, full)| full.clone()));
                match target {
                    Some(path) if path != self.current_listing().path => {
                        self.navigate(path, Vec::new());
                        true
                    }
                    _ => false,
                }
            }

            // Toolbar buttons drive the history / hierarchy navigation.
            BACK_ID => self.go_back_to_listing().is_some(),
            FORWARD_ID => self.go_forward_to_listing().is_some(),
            UP_ID => self.go_up_to_listing().is_some(),
            REFRESH_ID => {
                // Re-show the current path; a no-op for the in-memory listing,
                // but reports no change so it never spuriously redraws.
                false
            }

            // The directory listing table: select rows or activate (open) one.
            LISTING_KEY => self.apply_listing_action(action),

            _ => false,
        }
    }

    /// Handle a `select` / `activate` / `sort` action targeting the listing
    /// table.
    fn apply_listing_action(&mut self, action: &AppWidgetAction) -> bool {
        // A header-click sort: the payload is the bare clicked column index (the
        // shell's `translate_action` normalizes the toolkit's `"<col>:<dir>"` form
        // down to `"<col>"`, so this app sees one stable contract). The app owns
        // the direction: re-clicking the active column toggles ascending/desc.
        if action.name == "sort" {
            return self.apply_listing_sort(&action.payload);
        }

        let count = self.current_listing().visible_count();
        // The payload is the row index, optionally suffixed with a modifier
        // verb so Ctrl/Shift multi-select flows through the plain-string seam:
        //   "3"          -> single select (replace)
        //   "3:toggle"   -> Ctrl-click: toggle this row in/out of the set
        //   "3:range"    -> Shift-click: extend from the anchor to this row
        let (idx_str, modifier) = match action.payload.split_once(':') {
            Some((idx, m)) => (idx, m),
            None => (action.payload.as_str(), ""),
        };
        let Ok(index) = idx_str.parse::<usize>() else {
            return false;
        };
        if index >= count {
            return false;
        }

        match action.name.as_str() {
            // Open / activate a row: descend into a directory.
            "activate" | "open" | "navigate" => {
                let Some(entry) = self.current_listing().get(index) else {
                    return false;
                };
                if entry.is_dir() {
                    let path = entry.path.clone();
                    self.navigate(path, Vec::new());
                    true
                } else {
                    // Opening a file selects it (no app launcher at this layer).
                    self.select_single(index)
                }
            }
            // Selection update.
            "select" | "change" | "" => match modifier {
                "toggle" => self.select_toggle(index),
                "range" => self.select_range_to(index),
                _ => self.select_single(index),
            },
            _ => false,
        }
    }

    /// Re-sort the directory listing by the clicked column index. Re-clicking the
    /// already-active column toggles the direction; clicking a new column sorts it
    /// ascending. Returns `true` when the sort actually changed.
    fn apply_listing_sort(&mut self, payload: &str) -> bool {
        let Ok(col) = payload.parse::<u32>() else {
            return false;
        };
        let Some(field) = field_for_column_index(col) else {
            return false;
        };
        let listing = self.current_listing();
        let ascending = if listing.sort_field == field {
            // Toggle direction on a re-click of the active column.
            !listing.sort_ascending
        } else {
            // A new column starts ascending.
            true
        };
        // Selection points at display positions that move when rows re-sort.
        self.clear_selection();
        self.current_listing_mut().set_sort(field, ascending);
        true
    }

    /// Replace the selection with a single row, reporting whether it changed.
    fn select_single(&mut self, index: usize) -> bool {
        if self.selection() == [index] {
            return false;
        }
        self.set_selection(vec![index]);
        true
    }

    /// Toggle a single row in/out of the current selection (Ctrl-click).
    fn select_toggle(&mut self, index: usize) -> bool {
        let mut sel: Vec<usize> = self.selection().to_vec();
        if let Some(pos) = sel.iter().position(|&i| i == index) {
            sel.remove(pos);
        } else {
            sel.push(index);
            sel.sort_unstable();
        }
        self.set_selection(sel);
        true
    }

    /// Extend the selection as a contiguous range from the current anchor (the
    /// first selected row, or `index` itself when nothing is selected) to
    /// `index` inclusive (Shift-click).
    fn select_range_to(&mut self, index: usize) -> bool {
        let anchor = self.selection().first().copied().unwrap_or(index);
        let (lo, hi) = if anchor <= index {
            (anchor, index)
        } else {
            (index, anchor)
        };
        let range: Vec<usize> = (lo..=hi).collect();
        if self.selection() == range.as_slice() {
            return false;
        }
        self.set_selection(range);
        true
    }
}

impl AppView for FilesRuntime {
    fn app_id(&self) -> &str {
        crate::FILES_APP_ID
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

    // ---- widget seam ------------------------------------------------------

    use liquide_interop::AppWidgetModel;

    /// Find a widget by key/id anywhere in the model tree.
    fn find<'a>(model: &'a AppWidgetModel, key: &str) -> Option<&'a liquide_interop::AppWidget> {
        use liquide_interop::AppWidget;
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

    /// A runtime sitting in `/home/user` with a dir + two files.
    fn runtime_in_home() -> FilesRuntime {
        let mut rt = FilesRuntime::new(FilesConfig::default());
        let entries = vec![
            FileEntry::directory("docs".into(), "/home/user/docs".into(), 0),
            FileEntry::file("a.txt".into(), "/home/user/a.txt".into(), 10, 200_000),
            FileEntry::file("b.txt".into(), "/home/user/b.txt".into(), 20, 300_000),
        ];
        rt.navigate("/home/user".into(), entries);
        rt
    }

    #[test]
    fn breadcrumb_segments_splits_absolute_path() {
        let segs = breadcrumb_segments("/home/user/docs");
        let labels: Vec<&str> = segs.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["/", "home", "user", "docs"]);
        // The full path of crumb index 2 ("user") reconstructs "/home/user".
        assert_eq!(segs[2].1, "/home/user");
        assert_eq!(segs[3].1, "/home/user/docs");
    }

    #[test]
    fn default_widget_model_is_some_not_the_trait_default_none() {
        // Guards against regressing back to the AppView default (None).
        let rt = FilesRuntime::new(FilesConfig::default());
        assert!(rt.widget_model().is_some(), "files must opt into the widget seam");
    }

    #[test]
    fn widget_model_reflects_current_dir_entries_and_path() {
        let rt = runtime_in_home();
        let model = rt.widget_model().expect("files exposes a widget model");

        // Breadcrumb matches the current path. It carries no interaction key,
        // so locate it structurally anywhere in the (nested) model tree.
        fn find_crumbs(w: &AppWidget) -> Option<&Vec<String>> {
            match w {
                AppWidget::Breadcrumb { crumbs } => Some(crumbs),
                AppWidget::Panel { children }
                | AppWidget::Card { children, .. }
                | AppWidget::GroupBox { children, .. }
                | AppWidget::Toolbar { children } => children.iter().find_map(find_crumbs),
                _ => None,
            }
        }
        let crumbs = model
            .root
            .iter()
            .find_map(find_crumbs)
            .expect("breadcrumb present");
        assert_eq!(crumbs, &vec!["/", "home", "user"]);

        // The listing table shows one row per entry, with the dir name suffixed.
        let table = find(&model, LISTING_KEY).expect("listing table present");
        match table {
            AppWidget::Table { rows, selection_mode, columns, .. } => {
                assert_eq!(*selection_mode, SelectionMode::Multiple);
                assert_eq!(columns.len(), 3);
                assert_eq!(rows.len(), 3);
                // Default sort forces directories first, then name-ascending:
                // docs/, a.txt, b.txt.
                let names: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
                assert_eq!(names, ["docs/", "a.txt", "b.txt"], "row order");
            }
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn widget_model_reflects_selection() {
        let mut rt = runtime_in_home();
        rt.set_selection(vec![0, 2]);
        let model = rt.widget_model().expect("model");
        match find(&model, LISTING_KEY).unwrap() {
            AppWidget::Table { selected, .. } => assert_eq!(selected, &vec![0u32, 2u32]),
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn listing_header_sort_uses_the_bare_column_index_contract() {
        // CANONICAL CONTRACT (t124): the files listing table receives the sort
        // payload as a BARE column index ("1"), because the shell normalizes the
        // toolkit Table's raw "<col>:<dir>" form at the chokepoint. Clicking the
        // Size column (index 1) sorts by size ascending and reflects it in the
        // model's TableSort.
        let mut rt = runtime_in_home();
        // Default sort is name-ascending (dirs first): docs/, a.txt, b.txt.
        let names0: Vec<String> = match find(&rt.widget_model().unwrap(), LISTING_KEY).unwrap() {
            AppWidget::Table { rows, sort, .. } => {
                assert_eq!(
                    sort,
                    &Some(TableSort { column: 0, direction: SortDirection::Ascending }),
                    "default model sort must reflect name-ascending"
                );
                rows.iter().map(|r| r[0].clone()).collect()
            }
            other => panic!("expected Table, got {other:?}"),
        };
        assert_eq!(names0, ["docs/", "a.txt", "b.txt"]);

        // Sort by Size ascending via the bare-index payload.
        assert!(
            rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "sort", "1")),
            "a header-click sort on the Size column must change the listing"
        );
        match find(&rt.widget_model().unwrap(), LISTING_KEY).unwrap() {
            AppWidget::Table { rows, sort, .. } => {
                assert_eq!(
                    sort,
                    &Some(TableSort { column: 1, direction: SortDirection::Ascending }),
                    "the model must reflect the new sort column/direction"
                );
                // Dirs first (size 0), then files by ascending size: a.txt(10) < b.txt(20).
                let names: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
                assert_eq!(names, ["docs/", "a.txt", "b.txt"]);
            }
            other => panic!("expected Table, got {other:?}"),
        }

        // Re-clicking the active column toggles to descending.
        assert!(rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "sort", "1")));
        match find(&rt.widget_model().unwrap(), LISTING_KEY).unwrap() {
            AppWidget::Table { rows, sort, .. } => {
                assert_eq!(
                    sort.as_ref().expect("sorted").direction,
                    SortDirection::Descending,
                    "re-clicking the active column toggles direction"
                );
                // Dirs still first; files by descending size: b.txt(20) > a.txt(10).
                let names: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
                assert_eq!(names, ["docs/", "b.txt", "a.txt"]);
            }
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn listing_header_sort_rejects_the_raw_toolkit_payload() {
        // The app's contract is the bare index; the raw toolkit "<col>:<dir>" form
        // must NOT be parsed here (the shell normalizes it first). This documents
        // why the chokepoint normalization is required (the t124 bug surface).
        let mut rt = runtime_in_home();
        assert!(
            !rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "sort", "1:asc")),
            "a raw '<col>:<dir>' payload must be a no-op; the shell normalizes it first"
        );
    }

    #[test]
    fn widget_model_marks_active_place_selected() {
        let mut rt = FilesRuntime::new(FilesConfig::default());
        rt.sidebar_mut()
            .add_bookmark("Proj".into(), "/proj".into());
        // Navigate to the bookmark's path so it becomes the active place.
        rt.navigate("/proj".into(), Vec::new());
        let model = rt.widget_model().expect("model");
        match find(&model, PLACES_KEY).unwrap() {
            AppWidget::List { items, selected, .. } => {
                let idx = items.iter().position(|n| n == "Proj").expect("bookmark listed");
                assert_eq!(selected, &vec![idx as u32], "active place selected");
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn places_list_supplies_bookmark_icons() {
        // The Files places/bookmarks sidebar List must carry each bookmark's
        // icon name so the shell can draw a glyph per row (folder-home for Home,
        // folder-* for the other system places). RED before wiring: the List
        // carried only names, so `icons` was empty and the sidebar was blank.
        let rt = FilesRuntime::new(FilesConfig::default());
        let model = rt.widget_model().expect("model");
        match find(&model, PLACES_KEY).unwrap() {
            AppWidget::List { items, icons, .. } => {
                // One icon slot per bookmark, positionally parallel to items.
                assert_eq!(icons.len(), items.len(), "an icon slot per bookmark");
                // Home is a default system bookmark carrying folder-home.
                let home = items
                    .iter()
                    .position(|n| n == "Home")
                    .expect("Home bookmark present");
                assert_eq!(
                    icons[home].as_deref(),
                    Some("folder-home"),
                    "the Home bookmark supplies its folder-home icon"
                );
                // Every default system bookmark supplies a non-empty icon name.
                assert!(
                    icons
                        .iter()
                        .all(|i| i.as_deref().is_some_and(|s| !s.is_empty())),
                    "all default bookmarks supply an icon: {icons:?}"
                );
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn apply_action_places_navigate_changes_dir() {
        let mut rt = runtime_in_home();
        rt.sidebar_mut()
            .add_bookmark("Proj".into(), "/proj".into());
        let idx = rt
            .sidebar()
            .bookmarks()
            .iter()
            .position(|b| b.name == "Proj")
            .unwrap();
        let changed = rt.apply_action(&AppWidgetAction::new(PLACES_KEY, "select", idx.to_string()));
        assert!(changed, "navigating to a place must report a change");
        assert_eq!(rt.current_listing().path, "/proj");
    }

    #[test]
    fn apply_action_breadcrumb_navigate_changes_dir() {
        let mut rt = FilesRuntime::new(FilesConfig::default());
        rt.navigate("/home/user/docs".into(), Vec::new());
        // Crumb index 2 == "user" -> "/home/user".
        let changed = rt.apply_action(&AppWidgetAction::new(CRUMBS_KEY, "navigate", "2"));
        assert!(changed);
        assert_eq!(rt.current_listing().path, "/home/user");
    }

    #[test]
    fn apply_action_toolbar_back_and_forward_change_dir() {
        let mut rt = FilesRuntime::new(FilesConfig::default());
        rt.navigate("/a".into(), Vec::new());
        rt.navigate("/b".into(), Vec::new());
        assert_eq!(rt.current_listing().path, "/b");

        let changed = rt.apply_action(&AppWidgetAction::new(BACK_ID, "click", ""));
        assert!(changed, "back must report a change");
        assert_eq!(rt.current_listing().path, "/a");

        let changed = rt.apply_action(&AppWidgetAction::new(FORWARD_ID, "click", ""));
        assert!(changed, "forward must report a change");
        assert_eq!(rt.current_listing().path, "/b");

        // No further forward step available.
        assert!(!rt.apply_action(&AppWidgetAction::new(FORWARD_ID, "click", "")));
    }

    #[test]
    fn apply_action_toolbar_up_changes_dir() {
        let mut rt = FilesRuntime::new(FilesConfig::default());
        rt.navigate("/home/user/docs".into(), Vec::new());
        let changed = rt.apply_action(&AppWidgetAction::new(UP_ID, "click", ""));
        assert!(changed);
        assert_eq!(rt.current_listing().path, "/home/user");
    }

    #[test]
    fn apply_action_row_select_updates_selection() {
        let mut rt = runtime_in_home();
        let changed = rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "select", "1"));
        assert!(changed, "selecting a row must report a change");
        assert_eq!(rt.selection(), &[1]);
        // The model reflects it.
        let model = rt.widget_model().unwrap();
        match find(&model, LISTING_KEY).unwrap() {
            AppWidget::Table { selected, .. } => assert_eq!(selected, &vec![1u32]),
            _ => unreachable!(),
        }
    }

    #[test]
    fn apply_action_row_toggle_extends_selection() {
        let mut rt = runtime_in_home();
        assert!(rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "select", "0")));
        // Ctrl-click row 2: now {0, 2}.
        assert!(rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "select", "2:toggle")));
        assert_eq!(rt.selection(), &[0, 2]);
        // Ctrl-click row 0 again: removes it -> {2}.
        assert!(rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "select", "0:toggle")));
        assert_eq!(rt.selection(), &[2]);
    }

    #[test]
    fn apply_action_row_range_selects_contiguous() {
        let mut rt = runtime_in_home();
        assert!(rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "select", "0")));
        // Shift-click row 2: range [0..=2].
        assert!(rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "select", "2:range")));
        assert_eq!(rt.selection(), &[0, 1, 2]);
    }

    #[test]
    fn apply_action_row_activate_opens_directory() {
        let mut rt = runtime_in_home();
        // Find the "docs" directory's row index in the (sorted) listing.
        let docs_idx = rt
            .current_listing()
            .entries
            .iter()
            .position(|e| e.is_dir())
            .expect("a directory entry exists");
        let changed =
            rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "activate", docs_idx.to_string()));
        assert!(changed, "activating a directory must navigate");
        assert_eq!(rt.current_listing().path, "/home/user/docs");
    }

    #[test]
    fn apply_action_is_a_no_op_for_unknown_widget() {
        let mut rt = runtime_in_home();
        rt.set_selection(vec![1]);
        let changed = rt.apply_action(&AppWidgetAction::new("does.not.exist", "click", ""));
        assert!(!changed, "unknown widget must report no change");
        // And nothing was mutated.
        assert_eq!(rt.selection(), &[1]);
        assert_eq!(rt.current_listing().path, "/home/user");
    }

    #[test]
    fn apply_action_out_of_range_row_is_a_no_op() {
        let mut rt = runtime_in_home();
        let changed = rt.apply_action(&AppWidgetAction::new(LISTING_KEY, "select", "999"));
        assert!(!changed, "an out-of-range row index must not select");
        assert!(rt.selection().is_empty());
    }
}
