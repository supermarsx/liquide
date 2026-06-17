//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! task manager, exposing the live process list as list content rows.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, AppWidget, AppWidgetAction,
    AppWidgetModel, ButtonKind, ContentKind, ContentRow, SelectionMode, SortDirection, Tab,
    TableColumn, TableSort,
};

use crate::runtime::{ProcessSortColumn, TaskManagerRuntime};
use crate::ui::{SortOrder, TabId};

/// Stable widget key for the tabbed container.
const TABS_KEY: &str = "tabs";
/// Stable widget id for the Processes tab.
const TAB_PROCESSES: &str = "processes";
/// Stable widget id for the Performance tab.
const TAB_PERFORMANCE: &str = "performance";
/// Stable widget key for the process table.
const TABLE_KEY: &str = "process_table";
/// Stable widget id for the End-task button.
const END_TASK_ID: &str = "end_task";

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

impl TaskManagerRuntime {
    /// Build the process table widget from the current (sorted) snapshot.
    fn build_process_table(&self) -> AppWidget {
        let processes = self.sorted_processes();
        let rows: Vec<Vec<String>> = processes
            .iter()
            .map(|p| {
                vec![
                    p.name.clone(),
                    p.pid.to_string(),
                    format!("{:.1}%", p.cpu_percent),
                    format!("{} MB", p.mem_working_bytes / (1024 * 1024)),
                ]
            })
            .collect();

        // The selected index is the position of the selected PID *within the
        // currently-sorted rows* (selection rides the PID, not the row index, so
        // a re-sort keeps the right process highlighted).
        let selected: Vec<u32> = self
            .selected_pid()
            .and_then(|pid| processes.iter().position(|p| p.pid == pid))
            .map(|i| i as u32)
            .into_iter()
            .collect();

        let sort = TableSort {
            column: self.sort_column().index(),
            direction: match self.sort_order() {
                SortOrder::Ascending => SortDirection::Ascending,
                SortOrder::Descending => SortDirection::Descending,
            },
        };

        AppWidget::Table {
            key: TABLE_KEY.to_string(),
            columns: vec![
                TableColumn {
                    label: "Name".to_string(),
                    sortable: true,
                },
                TableColumn {
                    label: "PID".to_string(),
                    sortable: true,
                },
                TableColumn {
                    label: "CPU".to_string(),
                    sortable: true,
                },
                TableColumn {
                    label: "Memory".to_string(),
                    sortable: true,
                },
            ],
            rows,
            sort: Some(sort),
            selection_mode: SelectionMode::Single,
            selected,
        }
    }

    /// Build the toolkit-free widget model: Processes/Performance tabs holding a
    /// sortable process table plus an End-task button for the selection.
    fn build_widget_model(&self) -> AppWidgetModel {
        let table = self.build_process_table();

        // The End-task button only appears when a row is selected.
        let mut processes_body = vec![table];
        if self.selected_pid().is_some() {
            processes_body.push(AppWidget::Button {
                id: END_TASK_ID.to_string(),
                label: "End task".to_string(),
                kind: ButtonKind::Danger,
            });
        }

        let metrics = self.system_metrics();
        let performance_body = vec![
            AppWidget::Label {
                text: format!("CPU: {:.1}%", metrics.cpu_percent),
            },
            AppWidget::Label {
                text: format!("Memory: {:.1}%", metrics.memory_percent),
            },
            AppWidget::Label {
                text: format!("Processes: {}", self.process_count()),
            },
        ];

        let selected = match self.widget_tab() {
            TabId::Performance => 1,
            _ => 0,
        };

        let tabs = AppWidget::Tabs {
            tabs: vec![
                Tab {
                    id: TAB_PROCESSES.to_string(),
                    label: "Processes".to_string(),
                    children: processes_body,
                },
                Tab {
                    id: TAB_PERFORMANCE.to_string(),
                    label: "Performance".to_string(),
                    children: performance_body,
                },
            ],
            selected,
        };

        // The Tabs container itself carries the `tabs` key so a tab-change action
        // routes back here (its `id` field is reused for routing in apply_action).
        AppWidgetModel {
            title: Some("Task Manager".to_string()),
            root: vec![tabs],
        }
    }

    /// Apply a host-delivered widget action, returning `true` when runtime state
    /// changed.
    fn apply_widget_action(&mut self, action: &AppWidgetAction) -> bool {
        match action.widget.as_str() {
            // Tab change: payload is the tab id or its index.
            TABS_KEY | TAB_PROCESSES | TAB_PERFORMANCE => {
                let tab = match action.payload.as_str() {
                    TAB_PERFORMANCE | "1" => TabId::Performance,
                    TAB_PROCESSES | "0" => TabId::Processes,
                    _ => return false,
                };
                self.set_widget_tab(tab)
            }
            // Process table: a `sort` action carries the clicked column index; a
            // `select` action carries the clicked row index (into the sorted rows).
            TABLE_KEY => match action.name.as_str() {
                "sort" => {
                    let Ok(col_idx) = action.payload.parse::<u32>() else {
                        return false;
                    };
                    let Some(column) = ProcessSortColumn::from_index(col_idx) else {
                        return false;
                    };
                    self.sort_by(column)
                }
                "select" => {
                    let Ok(idx) = action.payload.parse::<usize>() else {
                        return false;
                    };
                    let Some(pid) = self.sorted_processes().get(idx).map(|p| p.pid) else {
                        return false;
                    };
                    self.select_pid(pid)
                }
                _ => false,
            },
            // End-task button: kill/remove the selected process.
            END_TASK_ID => self.end_selected_task(),
            _ => false,
        }
    }
}

impl AppView for TaskManagerRuntime {
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

    // ---- widget seam ------------------------------------------------------

    use crate::process::ProcessInfo;

    /// One known process for a frozen snapshot.
    fn proc(name: &str, pid: u32, cpu: f64, mem_mb: u64) -> ProcessInfo {
        ProcessInfo {
            name: name.to_string(),
            pid,
            cpu_percent: cpu,
            mem_working_bytes: mem_mb * 1024 * 1024,
            ..ProcessInfo::default()
        }
    }

    /// A runtime with a deterministic, frozen process set (no live tick/sampler).
    /// The set is intentionally NOT pre-sorted so widget sort is observable.
    fn frozen_runtime() -> TaskManagerRuntime {
        let mut rt = runtime();
        rt.set_processes(vec![
            proc("bravo", 200, 5.0, 64),
            proc("alpha", 100, 30.0, 256),
            proc("charlie", 300, 1.0, 16),
        ]);
        rt
    }

    fn find_table(model: &AppWidgetModel) -> &AppWidget {
        // The table lives inside the first Tab of the Tabs container.
        let AppWidget::Tabs { tabs, .. } = &model.root[0] else {
            panic!("root[0] must be Tabs, got {:?}", model.root[0]);
        };
        tabs[0]
            .children
            .iter()
            .find(|w| w.key() == Some(TABLE_KEY))
            .expect("process table present in Processes tab")
    }

    fn find_button<'a>(model: &'a AppWidgetModel, id: &str) -> Option<&'a AppWidget> {
        let AppWidget::Tabs { tabs, .. } = &model.root[0] else {
            return None;
        };
        tabs[0].children.iter().find(|w| w.key() == Some(id))
    }

    fn names(table: &AppWidget) -> Vec<String> {
        match table {
            AppWidget::Table { rows, .. } => rows.iter().map(|r| r[0].clone()).collect(),
            other => panic!("expected a Table, got {other:?}"),
        }
    }

    #[test]
    fn opts_into_widget_seam_not_trait_default_none() {
        let rt = frozen_runtime();
        assert!(
            rt.widget_model().is_some(),
            "task manager must expose a widget model"
        );
    }

    #[test]
    fn widget_model_reflects_frozen_process_set() {
        let rt = frozen_runtime();
        let model = rt.widget_model().expect("model");
        assert_eq!(model.title.as_deref(), Some("Task Manager"));

        // Tabs: Processes + Performance.
        let AppWidget::Tabs { tabs, .. } = &model.root[0] else {
            panic!("expected Tabs root");
        };
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].label, "Processes");
        assert_eq!(tabs[1].label, "Performance");

        // Table reflects all 3 frozen rows with name/pid/cpu/mem columns.
        match find_table(&model) {
            AppWidget::Table { columns, rows, .. } => {
                let labels: Vec<&str> = columns.iter().map(|c| c.label.as_str()).collect();
                assert_eq!(labels, vec!["Name", "PID", "CPU", "Memory"]);
                assert_eq!(rows.len(), 3, "all three frozen processes are rows");
                // Find alpha's row regardless of sort and check its cells.
                let alpha = rows.iter().find(|r| r[0] == "alpha").expect("alpha row");
                assert_eq!(alpha[1], "100");
                assert_eq!(alpha[2], "30.0%");
                assert_eq!(alpha[3], "256 MB");
            }
            other => panic!("expected a Table, got {other:?}"),
        }
    }

    #[test]
    fn default_sort_is_cpu_descending() {
        let rt = frozen_runtime();
        let model = rt.widget_model().expect("model");
        let table = find_table(&model);
        // CPU desc: alpha(30) > bravo(5) > charlie(1).
        assert_eq!(names(table), vec!["alpha", "bravo", "charlie"]);
        match table {
            AppWidget::Table { sort: Some(s), .. } => {
                assert_eq!(s.column, ProcessSortColumn::Cpu.index());
                assert_eq!(s.direction, SortDirection::Descending);
            }
            other => panic!("expected a sorted Table, got {other:?}"),
        }
    }

    #[test]
    fn header_sort_action_resorts_rows_deterministically() {
        let mut rt = frozen_runtime();
        // Click the Name column header (index 0) -> ascending by name.
        let changed = rt.apply_action(&AppWidgetAction::new(TABLE_KEY, "sort", "0"));
        assert!(changed, "sort action must report a change");

        let model = rt.widget_model().expect("model");
        let table = find_table(&model);
        assert_eq!(
            names(table),
            vec!["alpha", "bravo", "charlie"],
            "ascending by name"
        );
        match table {
            AppWidget::Table { sort: Some(s), .. } => {
                assert_eq!(s.column, 0);
                assert_eq!(s.direction, SortDirection::Ascending);
            }
            other => panic!("expected sorted table, got {other:?}"),
        }

        // Clicking the same header again toggles to descending.
        assert!(rt.apply_action(&AppWidgetAction::new(TABLE_KEY, "sort", "0")));
        let model = rt.widget_model().expect("model");
        assert_eq!(
            names(find_table(&model)),
            vec!["charlie", "bravo", "alpha"],
            "descending by name after toggle"
        );
    }

    #[test]
    fn sort_by_pid_orders_by_pid() {
        let mut rt = frozen_runtime();
        // PID column is index 1.
        assert!(rt.apply_action(&AppWidgetAction::new(TABLE_KEY, "sort", "1")));
        let model = rt.widget_model().expect("model");
        // Ascending PID: 100(alpha) < 200(bravo) < 300(charlie).
        assert_eq!(names(find_table(&model)), vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn row_select_updates_selection_by_pid_and_survives_resort() {
        let mut rt = frozen_runtime();
        // Default CPU-desc order: [alpha(100), bravo(200), charlie(300)].
        // Select row index 1 -> bravo (pid 200).
        assert!(rt.apply_action(&AppWidgetAction::new(TABLE_KEY, "select", "1")));
        assert_eq!(rt.selected_pid(), Some(200));

        let model = rt.widget_model().expect("model");
        match find_table(&model) {
            AppWidget::Table { selected, rows, .. } => {
                assert_eq!(selected, &vec![1u32]);
                assert_eq!(rows[1][0], "bravo");
            }
            other => panic!("expected table, got {other:?}"),
        }

        // Re-sort by name ascending: [alpha, bravo, charlie] -> bravo is now idx 1.
        // Sort by PID ascending instead to move bravo: [alpha, bravo, charlie] still idx1;
        // sort descending by CPU keeps default. Use name-desc to move bravo.
        assert!(rt.apply_action(&AppWidgetAction::new(TABLE_KEY, "sort", "0")));
        assert!(rt.apply_action(&AppWidgetAction::new(TABLE_KEY, "sort", "0"))); // name desc
        let model = rt.widget_model().expect("model");
        match find_table(&model) {
            // name desc: [charlie, bravo, alpha] -> bravo at index 1, still selected.
            AppWidget::Table { selected, rows, .. } => {
                assert_eq!(rows[1][0], "bravo");
                assert_eq!(selected, &vec![1u32], "selection tracks the PID across re-sort");
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn end_task_button_appears_only_with_selection_and_removes_process() {
        let mut rt = frozen_runtime();

        // No button before a selection.
        let model = rt.widget_model().expect("model");
        assert!(find_button(&model, END_TASK_ID).is_none());

        // Select bravo (row 1 in CPU-desc order).
        assert!(rt.apply_action(&AppWidgetAction::new(TABLE_KEY, "select", "1")));
        assert_eq!(rt.selected_pid(), Some(200));

        // Button now present (danger).
        let model = rt.widget_model().expect("model");
        match find_button(&model, END_TASK_ID) {
            Some(AppWidget::Button { label, kind, .. }) => {
                assert_eq!(label, "End task");
                assert_eq!(*kind, ButtonKind::Danger);
            }
            other => panic!("expected End task button, got {other:?}"),
        }

        // Click End task -> bravo removed, selection cleared.
        assert!(rt.apply_action(&AppWidgetAction::new(END_TASK_ID, "click", "")));
        assert_eq!(rt.selected_pid(), None);
        assert_eq!(rt.visible_processes().len(), 2);
        assert!(
            rt.visible_processes().iter().all(|p| p.pid != 200),
            "bravo (pid 200) was ended"
        );

        let model = rt.widget_model().expect("model");
        assert_eq!(names(find_table(&model)), vec!["alpha", "charlie"]);
        // Button gone again with no selection.
        assert!(find_button(&model, END_TASK_ID).is_none());
    }

    #[test]
    fn end_task_with_no_selection_is_a_no_op() {
        let mut rt = frozen_runtime();
        assert!(!rt.apply_action(&AppWidgetAction::new(END_TASK_ID, "click", "")));
        assert_eq!(rt.visible_processes().len(), 3);
    }

    #[test]
    fn tab_change_action_switches_active_tab() {
        let mut rt = frozen_runtime();
        assert_eq!(rt.widget_tab(), TabId::Processes);

        let changed = rt.apply_action(&AppWidgetAction::new(TABS_KEY, "change", "performance"));
        assert!(changed, "tab change must report a change");
        assert_eq!(rt.widget_tab(), TabId::Performance);

        // Model reflects the active tab via Tabs.selected.
        let model = rt.widget_model().expect("model");
        match &model.root[0] {
            AppWidget::Tabs { selected, .. } => assert_eq!(*selected, 1),
            other => panic!("expected Tabs, got {other:?}"),
        }

        // Switching back.
        assert!(rt.apply_action(&AppWidgetAction::new(TABS_KEY, "change", "0")));
        assert_eq!(rt.widget_tab(), TabId::Processes);
    }

    #[test]
    fn apply_action_is_a_no_op_for_unknown_widget() {
        let mut rt = frozen_runtime();
        let before = rt.widget_model().expect("model");
        assert!(!rt.apply_action(&AppWidgetAction::new("nope", "click", "")));
        let after = rt.widget_model().expect("model");
        assert_eq!(before, after, "unknown widget action leaves the model unchanged");
    }
}
