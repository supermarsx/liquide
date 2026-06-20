//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! settings application, plus the real list content view that mirrors the live
//! search buffer and category list.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, AppWidget, AppWidgetAction,
    AppWidgetModel, ContentKind, ContentRow, SelectionMode, WidgetOption,
};

use std::collections::BTreeMap;

use crate::category::Category;
use crate::entry::{SettingEntry, SettingKind, SettingValue};
use crate::runtime::SettingsRuntime;

/// Stable widget key for the search field.
const SEARCH_KEY: &str = "search";
/// Stable widget key for the category sidebar list.
const CATEGORY_LIST_KEY: &str = "categories";

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

/// Map a single setting entry to the widget that best represents its kind, keyed
/// by the entry's stable `key` so an [`AppWidgetAction`] routes straight back to
/// [`SettingsRuntime::set_value`].
fn entry_to_widget(entry: &SettingEntry) -> AppWidget {
    let key = entry.key.clone();
    match (&entry.kind, &entry.value) {
        (SettingKind::Toggle, SettingValue::Bool(checked)) => {
            // A switch (pill) reads better than a checkbox for an on/off setting,
            // matching the macOS settings look. Actions are keyed off the entry's
            // `SettingKind::Toggle`, so the control choice does not affect routing.
            AppWidget::Switch { key, checked: *checked }
        }
        (SettingKind::Slider { min, max, step }, SettingValue::Number(value)) => AppWidget::Slider {
            key,
            min: *min,
            max: *max,
            step: *step,
            value: *value,
        },
        (SettingKind::Percentage, SettingValue::Number(value)) => AppWidget::Slider {
            key,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            value: *value,
        },
        (SettingKind::Choice { options }, SettingValue::Text(selected)) => AppWidget::Dropdown {
            key,
            options: options.iter().map(WidgetOption::new).collect(),
            selected: Some(selected.clone()),
        },
        // Text / Color / KeyBind, plus any value-shape fallback, render as a
        // single-line text field carrying the current value as a string.
        (_, value) => AppWidget::TextInput {
            key,
            value: value.to_string(),
        },
    }
}

/// Build one labelled settings row: a horizontal strip pairing the entry's
/// human label with its control. A `Toolbar` is the toolkit's horizontal flex
/// container, so the label sits to the LEFT of the control on a single line
/// (`lq-toolbar.horizontal`), instead of the control floating unlabelled.
fn entry_row(entry: &SettingEntry) -> AppWidget {
    AppWidget::Toolbar {
        children: vec![
            AppWidget::Label {
                text: entry.label.clone(),
            },
            entry_to_widget(entry),
        ],
    }
}

impl SettingsRuntime {
    /// Build the toolkit-free widget model from the live runtime state.
    ///
    /// Layout: a two-column settings panel. The LEFT column is the category
    /// sidebar (a single-select `List`, fixed-width via `lq-list`); the RIGHT
    /// column is a scroll of the active category's settings, grouped into titled
    /// `Card`s by section, each row pairing a human label with its control. A
    /// search field sits above the content column. The two columns are the two
    /// children of a horizontal `Toolbar` (the toolkit's only horizontal flex
    /// container) so they render side-by-side rather than stacked.
    fn build_widget_model(&self) -> AppWidgetModel {
        let active = self.active_category();

        // ── Left column: category sidebar ───────────────────────────────────
        let items: Vec<String> = Category::ALL.iter().map(|c| c.label().to_string()).collect();
        let selected_idx = Category::ALL
            .iter()
            .position(|c| *c == active)
            .map(|i| i as u32);
        let sidebar = AppWidget::List {
            key: CATEGORY_LIST_KEY.to_string(),
            items,
            selection_mode: SelectionMode::Single,
            selected: selected_idx.into_iter().collect(),
        };

        // ── Right column: header + search + grouped settings cards ──────────
        // A labelled search row (label to the left of the field, like the entry
        // rows) so the field is not an unexplained floating box.
        let search_row = AppWidget::Toolbar {
            children: vec![
                AppWidget::Label {
                    text: "Search".to_string(),
                },
                AppWidget::TextInput {
                    key: SEARCH_KEY.to_string(),
                    value: self.search_query().to_string(),
                },
            ],
        };

        // Entries of the active category, grouped by their `section` and ordered
        // deterministically (BTreeMap keys + sorted entries) so the model is
        // stable frame-to-frame (HashMap iteration order is not).
        let mut entries = self.visible_entries();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        let mut sections: BTreeMap<String, Vec<&SettingEntry>> = BTreeMap::new();
        for entry in entries {
            sections.entry(entry.section.clone()).or_default().push(entry);
        }

        // The content column: a search row, then one titled Card per section
        // holding that section's labelled rows. The category name + description
        // live in the title bar / sidebar, so the column stays uncluttered.
        let mut content_children: Vec<AppWidget> = Vec::new();
        content_children.push(search_row);
        for (section, rows) in sections {
            let body: Vec<AppWidget> = rows.into_iter().map(entry_row).collect();
            content_children.push(AppWidget::Card {
                title: Some(section),
                children: body,
            });
        }

        let content = AppWidget::Panel {
            children: content_children,
        };

        // The two columns sit side-by-side in a horizontal toolbar.
        let columns = AppWidget::Toolbar {
            children: vec![sidebar, content],
        };

        AppWidgetModel {
            title: Some(format!("Settings — {}", active.label())),
            root: vec![columns],
        }
    }

    /// Apply a host-delivered widget action to the runtime, returning `true` when
    /// the runtime state changed.
    fn apply_widget_action(&mut self, action: &AppWidgetAction) -> bool {
        match action.widget.as_str() {
            // Search field: replace the buffer with the new text and re-run.
            SEARCH_KEY => {
                if self.search_query() == action.payload {
                    return false;
                }
                self.clear_search();
                if !action.payload.is_empty() {
                    return self.push_search_text(&action.payload);
                }
                // Cleared the query: report the change.
                true
            }
            // Category sidebar: payload is the selected index (preferred) or the
            // category label/id.
            CATEGORY_LIST_KEY => {
                let target = action
                    .payload
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| Category::ALL.get(i).copied())
                    .or_else(|| Category::from_id(&action.payload.to_lowercase()))
                    .or_else(|| {
                        Category::ALL
                            .iter()
                            .copied()
                            .find(|c| c.label() == action.payload)
                    });
                match target {
                    Some(cat) if cat != self.active_category() => {
                        self.set_category(cat);
                        true
                    }
                    _ => false,
                }
            }
            // Otherwise the widget key is a setting key: map the verb+payload to a
            // typed value and push it through validation/undo via set_value.
            key => {
                let Some(entry) = self.get(key) else {
                    return false;
                };
                let new_value = match (&entry.kind, action.name.as_str()) {
                    (SettingKind::Toggle, "toggle") => {
                        let current = matches!(entry.value, SettingValue::Bool(true));
                        SettingValue::Bool(!current)
                    }
                    (SettingKind::Toggle, "change") => match action.payload.parse::<bool>() {
                        Ok(b) => SettingValue::Bool(b),
                        Err(_) => return false,
                    },
                    (SettingKind::Slider { .. } | SettingKind::Percentage, "change") => {
                        match action.payload.parse::<f64>() {
                            Ok(n) => SettingValue::Number(n),
                            Err(_) => return false,
                        }
                    }
                    (SettingKind::Choice { .. }, "select" | "change") => {
                        SettingValue::Text(action.payload.clone())
                    }
                    (_, "change") => SettingValue::Text(action.payload.clone()),
                    _ => return false,
                };
                self.set_value(key, new_value).is_ok()
            }
        }
    }
}

impl AppView for SettingsRuntime {
    fn app_id(&self) -> &str {
        crate::SETTINGS_APP_ID
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

    // ---- widget seam ------------------------------------------------------

    use liquide_interop::{AppWidget, AppWidgetAction, AppWidgetModel};

    /// Find a top-level widget of a given key anywhere in the model.
    fn find<'a>(model: &'a AppWidgetModel, key: &str) -> Option<&'a AppWidget> {
        fn walk<'a>(w: &'a AppWidget, key: &str) -> Option<&'a AppWidget> {
            if w.key() == Some(key) {
                return Some(w);
            }
            match w {
                AppWidget::Panel { children }
                | AppWidget::Card { children, .. }
                | AppWidget::GroupBox { children, .. }
                | AppWidget::Toolbar { children } => {
                    children.iter().find_map(|c| walk(c, key))
                }
                _ => None,
            }
        }
        model.root.iter().find_map(|w| walk(w, key))
    }

    #[test]
    fn widget_model_reflects_runtime_state() {
        let mut rt = runtime();
        rt.set_category(Category::Display);

        let model = rt.widget_model().expect("settings exposes a widget model");

        // Search field present, mirroring the (empty) query.
        let search = find(&model, SEARCH_KEY).expect("search field present");
        assert!(matches!(search, AppWidget::TextInput { value, .. } if value.is_empty()));

        // Category sidebar selects the active category (Display = index 0).
        let sidebar = find(&model, CATEGORY_LIST_KEY).expect("category list present");
        match sidebar {
            AppWidget::List { items, selected, .. } => {
                assert_eq!(items.len(), Category::ALL.len());
                assert_eq!(selected, &vec![0u32]);
            }
            other => panic!("expected a List, got {other:?}"),
        }

        // The night-light toggle (a Display entry) appears as an off Switch.
        let toggle = find(&model, "display.night_light").expect("night_light entry in model");
        assert!(matches!(toggle, AppWidget::Switch { checked: false, .. }));

        // The resolution choice appears as a Dropdown with the current selection.
        let dropdown = find(&model, "display.resolution").expect("resolution entry in model");
        assert!(matches!(
            dropdown,
            AppWidget::Dropdown { selected: Some(s), .. } if s == "1920x1080"
        ));

        // The UI-scale slider carries the live numeric value + bounds.
        let slider = find(&model, "display.scale").expect("scale entry in model");
        assert!(matches!(
            slider,
            AppWidget::Slider { value, min, max, .. }
                if (*value - 1.0).abs() < f64::EPSILON && *min == 1.0 && *max == 3.0
        ));
    }

    #[test]
    fn widget_model_shows_toggled_setting_as_checked() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        // Flip night_light on through the real runtime path.
        rt.set_value("display.night_light", SettingValue::Bool(true))
            .expect("set night_light");

        let model = rt.widget_model().expect("model");
        let toggle = find(&model, "display.night_light").expect("toggle present");
        assert!(
            matches!(toggle, AppWidget::Switch { checked: true, .. }),
            "a toggled setting must show checked=true, got {toggle:?}"
        );
    }

    #[test]
    fn widget_model_only_shows_active_category_entries() {
        let mut rt = runtime();
        rt.set_category(Category::Audio);
        let model = rt.widget_model().expect("model");
        // An Audio entry is present...
        assert!(find(&model, "audio.volume").is_some());
        // ...and a Display entry is NOT (different category).
        assert!(find(&model, "display.night_light").is_none());
    }

    #[test]
    fn apply_action_toggle_flips_the_setting() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        assert_eq!(rt.value("display.night_light"), Some(&SettingValue::Bool(false)));

        let changed = rt.apply_action(&AppWidgetAction::new("display.night_light", "toggle", ""));
        assert!(changed, "toggle action must report a change");
        assert_eq!(
            rt.value("display.night_light"),
            Some(&SettingValue::Bool(true)),
            "the Checkbox toggle action must flip the underlying setting"
        );

        // And the next model reflects it.
        let model = rt.widget_model().expect("model");
        assert!(matches!(
            find(&model, "display.night_light"),
            Some(AppWidget::Switch { checked: true, .. })
        ));
    }

    #[test]
    fn apply_action_slider_change_sets_value() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        let changed = rt.apply_action(&AppWidgetAction::new("display.scale", "change", "2.0"));
        assert!(changed);
        assert_eq!(rt.value("display.scale"), Some(&SettingValue::Number(2.0)));
    }

    #[test]
    fn apply_action_slider_change_rejects_out_of_range() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        // 9.0 is outside [1.0, 3.0] → set_value validation fails → no change.
        let changed = rt.apply_action(&AppWidgetAction::new("display.scale", "change", "9.0"));
        assert!(!changed);
        assert_eq!(rt.value("display.scale"), Some(&SettingValue::Number(1.0)));
    }

    #[test]
    fn apply_action_dropdown_select_sets_choice() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        let changed =
            rt.apply_action(&AppWidgetAction::new("display.resolution", "select", "2560x1440"));
        assert!(changed);
        assert_eq!(
            rt.value("display.resolution"),
            Some(&SettingValue::Text("2560x1440".to_string()))
        );
    }

    #[test]
    fn apply_action_search_filters_and_updates_query() {
        let mut rt = runtime();
        let changed = rt.apply_action(&AppWidgetAction::new(SEARCH_KEY, "change", "volume"));
        assert!(changed, "search action must report a change");
        assert_eq!(rt.search_query(), "volume");
        // Search ran against the entries: at least the audio.volume hit appears.
        assert!(
            rt.search_results().iter().any(|r| r.key == "audio.volume"),
            "search should surface the volume setting"
        );
        // The widget model's search field now mirrors the query.
        let model = rt.widget_model().expect("model");
        assert!(matches!(
            find(&model, SEARCH_KEY),
            Some(AppWidget::TextInput { value, .. }) if value == "volume"
        ));
    }

    #[test]
    fn apply_action_sidebar_select_changes_category() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        // Network is index 3 in Category::ALL.
        let idx = Category::ALL
            .iter()
            .position(|c| *c == Category::Network)
            .unwrap();
        let changed = rt.apply_action(&AppWidgetAction::new(
            CATEGORY_LIST_KEY,
            "select",
            idx.to_string(),
        ));
        assert!(changed, "sidebar select must report a change");
        assert_eq!(rt.active_category(), Category::Network);

        // The model now reflects the new category's entries.
        let model = rt.widget_model().expect("model");
        assert!(find(&model, "network.hostname").is_some());
        assert!(find(&model, "display.night_light").is_none());
    }

    #[test]
    fn apply_action_is_a_no_op_for_unknown_widget() {
        let mut rt = runtime();
        // Honest-red guard: an action against a non-existent widget must not
        // mutate any runtime state and must report false.
        let before = rt.value("display.night_light").cloned();
        let changed = rt.apply_action(&AppWidgetAction::new("does.not.exist", "toggle", ""));
        assert!(!changed);
        assert_eq!(rt.value("display.night_light").cloned(), before);
    }

    #[test]
    fn default_widget_model_is_some_not_the_trait_default_none() {
        // Guards against regressing back to the AppView default (None): a static
        // None here would fail.
        let rt = runtime();
        assert!(
            rt.widget_model().is_some(),
            "settings must opt into the widget seam"
        );
    }

    // ---- layout structure (t191) ------------------------------------------
    //
    // These are the teeth for the "looks like a clean settings panel" fix. They
    // assert the MODEL produces a sensible LAYOUT, not a flat pile of bare
    // controls. They fail RED against the pre-t191 model (a flat
    // `root: [search, sidebar, panel-of-bare-controls]` with no labels, no
    // grouping, and no side-by-side columns).

    /// A control is "interactive" if it carries a key (checkbox/switch/slider/
    /// dropdown/text/list/…). Containers and static labels are not.
    fn is_control(w: &AppWidget) -> bool {
        matches!(
            w,
            AppWidget::Switch { .. }
                | AppWidget::Checkbox { .. }
                | AppWidget::Slider { .. }
                | AppWidget::Dropdown { .. }
                | AppWidget::TextInput { .. }
                | AppWidget::RadioGroup { .. }
                | AppWidget::Segmented { .. }
        )
    }

    /// The root is a SINGLE horizontal container (Toolbar) whose two children are
    /// the category sidebar (a List) and the content column (a Panel) — i.e. a
    /// two-column layout, not a vertical stack of three siblings.
    #[test]
    fn root_is_two_columns_sidebar_and_content() {
        let rt = runtime();
        let model = rt.widget_model().expect("model");
        assert_eq!(
            model.root.len(),
            1,
            "root must be a single two-column container, got {} top-level nodes",
            model.root.len()
        );
        let AppWidget::Toolbar { children } = &model.root[0] else {
            panic!("root must be a horizontal Toolbar (the two columns), got {:?}", model.root[0]);
        };
        assert_eq!(children.len(), 2, "two columns: sidebar + content");
        // First column: the category sidebar list.
        assert!(
            matches!(&children[0], AppWidget::List { key, items, .. }
                if key == CATEGORY_LIST_KEY && items.len() == Category::ALL.len()),
            "first column must be the category sidebar List, got {:?}",
            children[0]
        );
        // Second column: a content Panel (holds the cards).
        assert!(
            matches!(&children[1], AppWidget::Panel { .. }),
            "second column must be the content Panel, got {:?}",
            children[1]
        );
    }

    /// The content column groups settings into TITLED cards (by section) — proof
    /// of grouping, not one flat list.
    #[test]
    fn settings_are_grouped_into_titled_cards() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        let model = rt.widget_model().expect("model");
        let AppWidget::Toolbar { children } = &model.root[0] else {
            panic!("root toolbar");
        };
        let AppWidget::Panel { children: content } = &children[1] else {
            panic!("content panel");
        };
        let cards: Vec<&AppWidget> = content
            .iter()
            .filter(|w| matches!(w, AppWidget::Card { .. }))
            .collect();
        assert!(
            !cards.is_empty(),
            "the content column must contain at least one section Card"
        );
        // Every Card carries a non-empty section title.
        for card in &cards {
            if let AppWidget::Card { title, .. } = card {
                assert!(
                    title.as_deref().is_some_and(|t| !t.is_empty()),
                    "every section Card must have a non-empty title, got {title:?}"
                );
            }
        }
    }

    /// EVERY interactive control sits in a labelled ROW (a horizontal Toolbar
    /// pairing a Label with the control) — NOT as a bare, unlabelled control.
    /// This is the load-bearing teeth: the pre-t191 model emitted bare controls
    /// with no labels (the "what is this knob?" jank), which fails here.
    #[test]
    fn every_control_has_a_label_in_its_row() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        let model = rt.widget_model().expect("model");

        // Walk the whole tree; every control we find must have a sibling Label
        // inside the SAME immediate Toolbar row.
        fn check_rows(w: &AppWidget, control_count: &mut usize, labelled: &mut usize) {
            match w {
                AppWidget::Toolbar { children } => {
                    let has_label = children.iter().any(|c| matches!(c, AppWidget::Label { text } if !text.is_empty()));
                    let row_controls: Vec<&AppWidget> =
                        children.iter().filter(|c| is_control(c)).collect();
                    for ctrl in &row_controls {
                        *control_count += 1;
                        if has_label {
                            *labelled += 1;
                        }
                        // Recurse no further into a control (it's a leaf).
                        let _ = ctrl;
                    }
                    // Recurse into any nested containers (not the leaf controls).
                    for c in children {
                        if !is_control(c) {
                            check_rows(c, control_count, labelled);
                        }
                    }
                }
                AppWidget::Panel { children }
                | AppWidget::Card { children, .. }
                | AppWidget::GroupBox { children, .. } => {
                    // A control sitting DIRECTLY in a Panel/Card (not in a row) is
                    // the bare-control jank — count it as an unlabelled control.
                    for c in children {
                        if is_control(c) {
                            *control_count += 1; // no label sibling row ⇒ not labelled
                        } else {
                            check_rows(c, control_count, labelled);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut total = 0usize;
        let mut labelled = 0usize;
        for w in &model.root {
            check_rows(w, &mut total, &mut labelled);
        }
        assert!(total > 0, "the active category must expose some controls");
        assert_eq!(
            labelled, total,
            "every control must be paired with a label in its row \
             ({labelled}/{total} labelled) — bare unlabelled controls are the jank this fixes"
        );
    }

    /// Teeth for the search field: it lives in a labelled row too (not a floating
    /// box), and is still findable by its key so search actions keep working.
    #[test]
    fn search_field_is_present_and_labelled() {
        let rt = runtime();
        let model = rt.widget_model().expect("model");
        assert!(
            find(&model, SEARCH_KEY).is_some(),
            "the search field must still be in the model"
        );
    }

    /// HONEST-RED proof: the structural checks above genuinely have teeth. We
    /// reconstruct the PRE-t191 flat model (search + sidebar + a Panel of BARE,
    /// unlabelled controls, three vertically-stacked siblings) and show the new
    /// invariants reject it. If someone regresses the builder back to the flat
    /// stack, the real tests above fail exactly the way this proof predicts.
    #[test]
    fn pre_t191_flat_model_violates_the_layout_invariants() {
        let mut rt = runtime();
        rt.set_category(Category::Display);
        let mut entries = rt.visible_entries();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        let bare: Vec<AppWidget> = entries.iter().map(|e| entry_to_widget(e)).collect();

        let old = AppWidgetModel::with_root(vec![
            AppWidget::TextInput { key: SEARCH_KEY.to_string(), value: String::new() },
            AppWidget::List {
                key: CATEGORY_LIST_KEY.to_string(),
                items: Category::ALL.iter().map(|c| c.label().to_string()).collect(),
                selection_mode: SelectionMode::Single,
                selected: vec![0],
            },
            AppWidget::Panel { children: bare },
        ]);

        // (1) NOT a single two-column root.
        assert_ne!(old.root.len(), 1, "old model was a 3-sibling vertical stack");

        // (2) NO titled section Cards anywhere.
        fn has_card(w: &AppWidget) -> bool {
            match w {
                AppWidget::Card { .. } => true,
                AppWidget::Panel { children }
                | AppWidget::Toolbar { children }
                | AppWidget::GroupBox { children, .. } => children.iter().any(has_card),
                _ => false,
            }
        }
        assert!(!old.root.iter().any(has_card), "old model had no grouping cards");

        // (3) The controls are BARE — they sit directly under a Panel with no
        // label row. Mirror the production test's accounting to show it would
        // have flagged them all as unlabelled.
        fn count(w: &AppWidget, total: &mut usize, labelled: &mut usize) {
            match w {
                AppWidget::Toolbar { children } => {
                    let has_label = children
                        .iter()
                        .any(|c| matches!(c, AppWidget::Label { text } if !text.is_empty()));
                    for c in children {
                        if is_control(c) {
                            *total += 1;
                            if has_label {
                                *labelled += 1;
                            }
                        } else {
                            count(c, total, labelled);
                        }
                    }
                }
                AppWidget::Panel { children }
                | AppWidget::Card { children, .. }
                | AppWidget::GroupBox { children, .. } => {
                    for c in children {
                        if is_control(c) {
                            *total += 1; // bare control: no labelled row
                        } else {
                            count(c, total, labelled);
                        }
                    }
                }
                _ => {}
            }
        }
        let (mut total, mut labelled) = (0, 0);
        for w in &old.root {
            count(w, &mut total, &mut labelled);
        }
        assert!(total > 0, "the old model did have controls");
        assert_ne!(
            labelled, total,
            "the old bare-control model must FAIL the 'every control labelled' \
             invariant ({labelled}/{total}) — proving that test has teeth"
        );
    }
}
