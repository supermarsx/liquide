//! Generic `AppWidgetModel` → `liquide-widgets` integration engine (t108, P8).
//!
//! This module is the SHELL-SIDE pipeline that turns ANY app's toolkit-free
//! [`liquide_interop::AppWidgetModel`] into mounted, interactive
//! [`liquide_widgets`] Components rendered through the DOM/CSS pipeline. It is
//! deliberately **app-agnostic**: it lives in the shell because it bridges two
//! crates the shell already depends on (`liquide-interop` for the plain-data
//! model, `liquide-widgets` for the toolkit) and the apps must see neither the
//! toolkit nor the shell's scene types.
//!
//! ## The three pieces
//!
//! 1. **Mapper** ([`mount_model_into`]): walk an [`AppWidgetModel`] tree and, for
//!    each node, either create a structural container DOM element
//!    (`lq-panel`/`lq-card`/…) directly, or mount the matching
//!    [`liquide_widgets`] behavior as its own [`WidgetHost`] entry (so the host
//!    owns its runtime state — selection / checked / value / caret — keyed by the
//!    widget's stable key). Keys are namespaced per window
//!    (`aw-<window_id>-<key>`) so two windows showing the same app key never
//!    collide; the original key is recovered by [`strip_widget_id`] when an
//!    action is translated back to the app.
//!
//! 2. **Per-window host registry** ([`crate::shell::Shell::app_widget_hosts`]):
//!    one [`WidgetHost`] per widget-backed window. The host's widget DOM is
//!    mounted under a per-window `app-content-host` element positioned over the
//!    window's content rect (mirroring the P6 `window-frame` scaffold), so the
//!    widgets flow through the same CSS pipeline that paints all chrome, clipped
//!    to the content rect.
//!
//! 3. **Action translation** ([`translate_action`]): the real
//!    [`liquide_widgets::WidgetAction`] (a toolkit type the app must not see) is
//!    mapped to the plain-data [`liquide_interop::AppWidgetAction`] triple the
//!    app's `apply_action` consumes, using the widget's variant to pick the verb.

use liquide_interop::{AppWidget, AppWidgetAction, AppWidgetModel, ButtonKind, SelectionMode};
use liquide_widgets::{
    Breadcrumb, Button, Chip, Dropdown, Label, Link, List, Pagination, Progress, RadioGroup,
    Segmented, Slider, Table, TextArea, TextInput, Toggle, Tree, WidgetBehavior,
};

use liquide_dom::{Document, NodeId};
use liquide_widgets::host::WidgetHost;
use liquide_widgets::tree::TreeNode as WTreeNode;

/// Per-window prefix for namespacing a widget's stable key into a globally-unique
/// [`liquide_widgets::WidgetId`]: `aw-<window_id>-<key>`.
#[must_use]
pub(crate) fn widget_id(window_id: u64, key: &str) -> String {
    format!("aw-{window_id}-{key}")
}

/// Recover the app-side key from a namespaced widget id produced by
/// [`widget_id`]. Returns the original `key` (the substring after the
/// `aw-<window_id>-` prefix), or the whole id unchanged if it lacks the prefix.
#[must_use]
pub(crate) fn strip_widget_id(window_id: u64, id: &str) -> String {
    let prefix = format!("aw-{window_id}-");
    id.strip_prefix(&prefix).unwrap_or(id).to_string()
}

/// Map a [`ButtonKind`] to the widget toolkit's CSS variant class.
fn button_variant(kind: ButtonKind) -> &'static str {
    match kind {
        ButtonKind::Normal => "",
        ButtonKind::Primary => "primary",
        ButtonKind::Danger => "danger",
        ButtonKind::Ghost => "ghost",
    }
}

/// Convert an interop `TreeNode` (plain data) into a `liquide-widgets` `TreeNode`.
fn map_tree_node(node: &liquide_interop::TreeNode) -> WTreeNode {
    if node.children.is_empty() {
        WTreeNode::leaf(node.id.clone(), node.label.clone()).expanded(node.expanded)
    } else {
        let children: Vec<WTreeNode> = node.children.iter().map(map_tree_node).collect();
        WTreeNode::branch(node.id.clone(), node.label.clone(), children).expanded(node.expanded)
    }
}

/// Build the `liquide-widgets` behavior for a single *interactive / leaf*
/// [`AppWidget`], preserving its current value/selection so the mounted widget
/// reflects the model. Containers return `None` (they are handled structurally
/// by [`mount_model_into`]).
///
/// The index of `selected` in an option list is recovered by matching the
/// option value, so the toolkit's index-based `select(idx)` lands on the right
/// option even though the model stores the selection by value.
#[must_use]
pub(crate) fn behavior_for(widget: &AppWidget) -> Option<Box<dyn WidgetBehavior>> {
    let b: Box<dyn WidgetBehavior> = match widget {
        // ── static / inert ──────────────────────────────────────────────
        AppWidget::Label { text } => Box::new(Label::new(text.clone())),
        AppWidget::Link { text, href } => Box::new(Link::new(text.clone(), href.clone())),
        AppWidget::Chip { label } => Box::new(Chip::new(label.clone())),
        AppWidget::Breadcrumb { crumbs } => Box::new(Breadcrumb::new(crumbs.clone())),
        AppWidget::Progress { value } => Box::new(Progress::fraction(*value as f32)),

        // ── buttons ─────────────────────────────────────────────────────
        AppWidget::Button { label, kind, .. } => {
            // The button's `action` IS its key/id — translate_action keys on the
            // emitting widget id, so the action name carried by the widget can be
            // a fixed verb ("click").
            Box::new(Button::new(label.clone(), "click").variant(button_variant(*kind)))
        }

        AppWidget::Segmented {
            options, selected, ..
        } => {
            let pairs = options.iter().map(|o| (o.value.clone(), o.label.clone()));
            let mut seg = Segmented::new(pairs);
            if let Some(idx) = selected_index(options, selected.as_deref()) {
                seg = seg.select(idx);
            }
            Box::new(seg)
        }

        // ── inputs ──────────────────────────────────────────────────────
        AppWidget::TextInput { value, .. } => Box::new(TextInput::new("").with_text(value.clone())),
        AppWidget::TextArea {
            value,
            gutter,
            readonly,
            ..
        } => Box::new(
            TextArea::new("")
                .with_text(value.clone())
                .with_gutter(*gutter)
                .disabled(*readonly),
        ),
        AppWidget::Checkbox { checked, .. } => Box::new(Toggle::checkbox("").checked(*checked)),
        AppWidget::Switch { checked, .. } => Box::new(Toggle::switch("").checked(*checked)),
        AppWidget::RadioGroup {
            key,
            options,
            selected,
        } => {
            let pairs = options.iter().map(|o| (o.value.clone(), o.label.clone()));
            let mut rg = RadioGroup::new(key.clone(), pairs);
            if let Some(idx) = selected_index(options, selected.as_deref()) {
                rg = rg.select(idx);
            }
            Box::new(rg)
        }
        AppWidget::Slider {
            min,
            max,
            step,
            value,
            ..
        } => Box::new(Slider::new(*min as f32, *max as f32, *value as f32).step(*step as f32)),
        AppWidget::Dropdown {
            options, selected, ..
        } => {
            let pairs = options.iter().map(|o| (o.value.clone(), o.label.clone()));
            let mut dd = Dropdown::new(pairs);
            if let Some(idx) = selected_index(options, selected.as_deref()) {
                dd = dd.select(idx);
            }
            Box::new(dd)
        }

        // ── collections ─────────────────────────────────────────────────
        AppWidget::List {
            items,
            selection_mode,
            selected,
            ..
        } => {
            // The list stores (value, label); the app uses indices as the value,
            // so value == label == item text and selection is by index.
            let pairs = items
                .iter()
                .map(|item| (item.clone(), item.clone()));
            let mut list = List::new(pairs);
            if matches!(selection_mode, SelectionMode::Multiple) {
                list = list.multi();
            }
            // Pre-select the first selected index (List::select clears prior),
            // matching the model's primary selection.
            if let Some(&first) = selected.first() {
                list = list.select(first as usize);
            }
            Box::new(list)
        }
        AppWidget::Table {
            columns,
            rows,
            sort,
            selection_mode,
            ..
        } => {
            let mut table = Table::new();
            let any_sortable = columns.iter().any(|c| c.sortable);
            for col in columns {
                table = table.column(col.label.clone());
            }
            for row in rows {
                table = table.row(row.clone());
            }
            if any_sortable {
                table = table.sortable(true);
            }
            let _ = (sort, selection_mode); // initial sort/selection: model-driven re-render keeps them in sync
            Box::new(table)
        }
        AppWidget::Tree { nodes, .. } => {
            let mut tree = Tree::new();
            for node in nodes {
                tree = tree.root(map_tree_node(node));
            }
            Box::new(tree)
        }
        AppWidget::Pagination { page, pages, .. } => {
            Box::new(Pagination::new(*pages as usize).page(*page as usize))
        }

        // ── containers: handled structurally, not as a behavior ─────────
        AppWidget::Panel { .. }
        | AppWidget::Card { .. }
        | AppWidget::GroupBox { .. }
        | AppWidget::Tabs { .. }
        | AppWidget::Toolbar { .. }
        | AppWidget::Accordion { .. } => return None,
    };
    Some(b)
}

/// A STRUCTURE-only signature of a model: the variant shape, keys, option
/// values, and container nesting — but NOT the mutable per-widget values
/// (checked / value / selected / caret). Two models with the same structure but
/// different values hash the same, so an action-driven value change does NOT
/// trigger a remount (the affected widget re-renders in place); only an
/// external structural change (added/removed/reordered widgets, changed option
/// sets) remounts. Returned as a `String` so the caller can hash it.
#[must_use]
pub(crate) fn model_structure(model: &AppWidgetModel) -> String {
    let mut out = String::new();
    for w in &model.root {
        structure_into(w, &mut out);
    }
    out
}

fn structure_into(widget: &AppWidget, out: &mut String) {
    match widget {
        AppWidget::Panel { children } => {
            out.push_str("P(");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::Card { title, children } => {
            out.push_str("Cd[");
            out.push_str(title.as_deref().unwrap_or(""));
            out.push_str("](");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::GroupBox { label, children } => {
            out.push_str("Gb[");
            out.push_str(label);
            out.push_str("](");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::Toolbar { children } => {
            out.push_str("Tb(");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::Tabs { tabs, selected } => {
            // The selected tab determines which panel is mounted, so it is part
            // of the structure (changing tabs remounts the panel).
            out.push_str(&format!("Tabs[{selected}](" ));
            if let Some(tab) = tabs.get(*selected as usize) {
                for c in &tab.children {
                    structure_into(c, out);
                }
            }
            out.push(')');
        }
        AppWidget::Accordion { sections } => {
            out.push_str("Ac(");
            for s in sections {
                // Expanded sections mount their bodies, so expansion is structure.
                out.push_str(&format!("s[{}:{}]", s.id, s.expanded));
                if s.expanded {
                    for c in &s.children {
                        structure_into(c, out);
                    }
                }
            }
            out.push(')');
        }
        AppWidget::Label { .. } => out.push_str("L;"),
        AppWidget::Link { href, .. } => out.push_str(&format!("Ln[{href}];")),
        AppWidget::Chip { .. } => out.push_str("Ch;"),
        AppWidget::Breadcrumb { crumbs } => out.push_str(&format!("Bc[{}];", crumbs.len())),
        AppWidget::Progress { .. } => out.push_str("Pr;"),
        AppWidget::Button { id, kind, .. } => out.push_str(&format!("B[{id}:{kind:?}];")),
        AppWidget::Segmented { key, options, .. } => {
            out.push_str(&format!("Sg[{key}:{}];", options.len()))
        }
        AppWidget::TextInput { key, .. } => out.push_str(&format!("Ti[{key}];")),
        AppWidget::TextArea { key, gutter, .. } => {
            out.push_str(&format!("Tx[{key}:{gutter}];"))
        }
        AppWidget::Checkbox { key, .. } => out.push_str(&format!("Cb[{key}];")),
        AppWidget::Switch { key, .. } => out.push_str(&format!("Sw[{key}];")),
        AppWidget::RadioGroup { key, options, .. } => {
            out.push_str(&format!("Rg[{key}:{}];", options.len()))
        }
        AppWidget::Slider { key, .. } => out.push_str(&format!("Sl[{key}];")),
        AppWidget::Dropdown { key, options, .. } => {
            out.push_str(&format!("Dd[{key}:{}];", options.len()))
        }
        AppWidget::List {
            key,
            items,
            selection_mode,
            ..
        } => out.push_str(&format!("Li[{key}:{}:{selection_mode:?}];", items.len())),
        AppWidget::Table {
            key, columns, rows, ..
        } => out.push_str(&format!("Ta[{key}:{}x{}];", columns.len(), rows.len())),
        AppWidget::Tree { key, nodes } => out.push_str(&format!("Tr[{key}:{}];", nodes.len())),
        AppWidget::Pagination { key, pages, .. } => {
            out.push_str(&format!("Pg[{key}:{pages}];"))
        }
    }
}

/// The index of the option whose `value` equals `selected`, if any.
fn selected_index(
    options: &[liquide_interop::WidgetOption],
    selected: Option<&str>,
) -> Option<usize> {
    let want = selected?;
    options.iter().position(|o| o.value == want)
}

/// Whether an [`AppWidget`] is a structural container (mounted as a plain DOM
/// wrapper) rather than an interactive behavior.
fn is_container(widget: &AppWidget) -> bool {
    matches!(
        widget,
        AppWidget::Panel { .. }
            | AppWidget::Card { .. }
            | AppWidget::GroupBox { .. }
            | AppWidget::Tabs { .. }
            | AppWidget::Toolbar { .. }
            | AppWidget::Accordion { .. }
    )
}

/// Mount an entire [`AppWidgetModel`] under `parent` for window `window_id`:
/// create container DOM wrappers directly and mount each interactive widget as
/// its own [`WidgetHost`] entry (so its runtime state is keyed and survives
/// reconciliation). Returns the namespaced widget ids that were mounted, in
/// document order.
///
/// The dispatcher is needed because [`WidgetHost::mount`] registers the
/// per-widget event handlers on it (the single source of truth for events).
pub(crate) fn mount_model_into(
    model: &AppWidgetModel,
    window_id: u64,
    parent: NodeId,
    host: &mut WidgetHost,
    doc: &mut Document,
    dispatcher: &mut liquide_hit_test::EventDispatcher,
) -> Vec<String> {
    let mut mounted = Vec::new();
    for widget in &model.root {
        mount_widget(widget, window_id, parent, host, doc, dispatcher, &mut mounted);
    }
    mounted
}

/// Recursively mount one widget (and its children, for containers).
fn mount_widget(
    widget: &AppWidget,
    window_id: u64,
    parent: NodeId,
    host: &mut WidgetHost,
    doc: &mut Document,
    dispatcher: &mut liquide_hit_test::EventDispatcher,
    mounted: &mut Vec<String>,
) {
    if is_container(widget) {
        // Create a plain DOM wrapper element (styled by widgets.css) and mount
        // the interactive children inside it, so nesting is preserved while each
        // interactive widget keeps its own keyed host state.
        let (tag, caption, children): (&str, Option<&str>, Vec<&AppWidget>) = match widget {
            AppWidget::Panel { children } => ("lq-panel", None, children.iter().collect()),
            AppWidget::Card { title, children } => {
                ("lq-card", title.as_deref(), children.iter().collect())
            }
            AppWidget::GroupBox { label, children } => {
                ("lq-groupbox", Some(label.as_str()), children.iter().collect())
            }
            AppWidget::Toolbar { children } => ("lq-toolbar", None, children.iter().collect()),
            AppWidget::Tabs { tabs, selected } => {
                // Flatten to the selected tab's children (a structural tab strip
                // would need its own behavior; the selected panel is what paints).
                let sel = *selected as usize;
                let panel = doc.create_element("lq-tabs");
                doc.append_child(parent, panel);
                if let Some(tab) = tabs.get(sel) {
                    for child in &tab.children {
                        mount_widget(child, window_id, panel, host, doc, dispatcher, mounted);
                    }
                }
                return;
            }
            AppWidget::Accordion { sections } => {
                let acc = doc.create_element("lq-accordion");
                doc.append_child(parent, acc);
                for section in sections {
                    if section.expanded {
                        for child in &section.children {
                            mount_widget(child, window_id, acc, host, doc, dispatcher, mounted);
                        }
                    }
                }
                return;
            }
            _ => unreachable!("is_container guard"),
        };

        let wrapper = doc.create_element(tag);
        doc.append_child(parent, wrapper);
        if let Some(cap) = caption {
            let cap_el = doc.create_element("lq-caption");
            let txt = doc.create_text(cap);
            doc.append_child(cap_el, txt);
            doc.append_child(wrapper, cap_el);
        }
        for child in children {
            mount_widget(child, window_id, wrapper, host, doc, dispatcher, mounted);
        }
        return;
    }

    // Interactive / leaf widget: mount as its own keyed host entry.
    let Some(behavior) = behavior_for(widget) else {
        return;
    };
    // Use the widget's stable key when it has one; otherwise synthesize a stable
    // ordinal so non-keyed leaves (labels, chips) still get a unique mount id.
    let key = widget
        .key()
        .map(str::to_string)
        .unwrap_or_else(|| format!("leaf{}", mounted.len()));
    let id = widget_id(window_id, &key);
    host.mount(id.clone(), behavior, doc, parent, dispatcher);
    mounted.push(id);
}

/// Translate a toolkit [`liquide_widgets::WidgetAction`] into the plain-data
/// [`AppWidgetAction`] the app's `apply_action` consumes.
///
/// `widget` is the model node that emitted the action (looked up by stripped
/// key), used to choose the verb. `app_key` is the app-side key (already
/// stripped of the per-window namespace).
#[must_use]
pub(crate) fn translate_action(
    app_key: &str,
    model_widget: Option<&AppWidget>,
    action: &liquide_widgets::WidgetAction,
) -> AppWidgetAction {
    let mut payload = action.payload.clone().unwrap_or_default();

    // CANONICAL TABLE-SORT CONTRACT (chokepoint normalization).
    //
    // The `liquide_widgets::Table` emits its sort action with the payload
    // `"<col>:<dir>"` (e.g. `"0:asc"`, see `table.rs::sort_by`) — it carries the
    // direction it just toggled to. But every Table-consuming app (task-manager,
    // files, …) toggles the direction *itself* on a re-click of the same column
    // (`ProcessSortColumn` / `DirectoryListing::set_sort`), so the toolkit's
    // direction suffix is redundant — and an app that parses the payload as a bare
    // column index silently drops the whole header click (the t124 bug the e2e
    // harness surfaced). Rather than make each app defensively re-parse
    // `"col:dir"` (duplicated, easy to regress in a future Table consumer), we
    // normalize here, at the single shell-side chokepoint that already translates
    // toolkit → interop: a Table `sorted` payload is reduced to the bare column
    // index `"<col>"`. So EVERY app — present and future — receives the one
    // documented, stable sort payload form: the column index, nothing else.
    if matches!(model_widget, Some(AppWidget::Table { .. })) && action.name == "sorted" {
        if let Some((col, _dir)) = payload.split_once(':') {
            payload = col.to_string();
        }
    }

    // Map the toolkit verb (+ the target widget family) to the interop verb the
    // app understands. The toolkit emits a small set of names: "click",
    // "changed", "navigate", "toggled", "sorted", "remove".
    let name: &str = match (model_widget, action.name.as_str()) {
        (Some(AppWidget::Button { .. }), _) => "click",
        (Some(AppWidget::Checkbox { .. }), _) | (Some(AppWidget::Switch { .. }), _) => "toggle",
        (Some(AppWidget::TextInput { .. }), "changed") => "change",
        (Some(AppWidget::TextArea { .. }), "changed") => "change",
        (Some(AppWidget::Slider { .. }), "changed") => "change",
        (Some(AppWidget::Dropdown { .. }), "changed") => "select",
        (Some(AppWidget::Segmented { .. }), "changed") => "select",
        (Some(AppWidget::RadioGroup { .. }), "changed") => "select",
        (Some(AppWidget::List { .. }), "changed") => "select",
        (Some(AppWidget::Table { .. }), "sorted") => "sort",
        (Some(AppWidget::Table { .. }), "changed") => "select",
        (Some(AppWidget::Tree { .. }), "toggled") => "toggle",
        (Some(AppWidget::Tree { .. }), "changed") => "select",
        (Some(AppWidget::Accordion { .. }), "toggled") => "toggle",
        (Some(AppWidget::Pagination { .. }), "changed") => "navigate",
        (Some(AppWidget::Breadcrumb { .. }), "navigate") => "navigate",
        (Some(AppWidget::Link { .. }), "navigate") => "navigate",
        (Some(AppWidget::Chip { .. }), "remove") => "remove",
        (Some(AppWidget::Chip { .. }), "changed") => "toggle",
        // Fall back to the toolkit verb verbatim for anything unmapped.
        (_, other) => other,
    };
    AppWidgetAction::new(app_key.to_string(), name.to_string(), payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_interop::{TableColumn, WidgetOption};

    #[test]
    fn widget_id_round_trips() {
        let id = widget_id(7, "opacity");
        assert_eq!(id, "aw-7-opacity");
        assert_eq!(strip_widget_id(7, &id), "opacity");
        // A key containing dashes survives the strip (only the prefix is removed).
        let id2 = widget_id(3, "sort-by-name");
        assert_eq!(strip_widget_id(3, &id2), "sort-by-name");
    }

    #[test]
    fn behavior_for_maps_each_interactive_family() {
        assert!(behavior_for(&AppWidget::Button {
            id: "b".into(),
            label: "Save".into(),
            kind: ButtonKind::Primary,
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Checkbox {
            key: "c".into(),
            checked: true,
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Slider {
            key: "s".into(),
            min: 0.0,
            max: 1.0,
            step: 0.1,
            value: 0.5,
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Dropdown {
            key: "d".into(),
            options: vec![WidgetOption::new("a"), WidgetOption::new("b")],
            selected: Some("b".into()),
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Table {
            key: "t".into(),
            columns: vec![TableColumn::new("PID")],
            rows: vec![vec!["1".into()]],
            sort: None,
            selection_mode: SelectionMode::Single,
            selected: vec![],
        })
        .is_some());
        // Containers are NOT behaviors.
        assert!(behavior_for(&AppWidget::Panel { children: vec![] }).is_none());
    }

    #[test]
    fn behavior_for_textarea_produces_a_multiline_textarea_with_key_value_and_gutter() {
        // The TextArea node must map to a real liquide_widgets::TextArea carrying
        // the model's full (newline-bearing) text and the gutter flag — NOT a
        // single-line TextInput. Downcast the behavior to prove the concrete type.
        let widget = AppWidget::TextArea {
            key: "document".into(),
            value: "line one\nline two\nline three".into(),
            gutter: true,
            readonly: false,
        };
        let behavior = behavior_for(&widget).expect("TextArea maps to a behavior");

        // It must be a TextArea (the multi-line widget), not a TextInput.
        assert!(
            behavior.as_any().downcast_ref::<TextInput>().is_none(),
            "TextArea must NOT map to the single-line TextInput"
        );
        let ta = behavior
            .as_any()
            .downcast_ref::<TextArea>()
            .expect("behavior must be a liquide_widgets::TextArea");

        // The full multi-line value round-trips (the newline is preserved).
        assert_eq!(ta.text(), "line one\nline two\nline three");
        assert_eq!(ta.line_count(), 3);
        // The gutter flag flowed through.
        assert!(ta.gutter_visible(), "gutter flag must reach the widget");
    }

    #[test]
    fn behavior_for_textarea_readonly_disables_the_widget() {
        let widget = AppWidget::TextArea {
            key: "doc".into(),
            value: "ro".into(),
            gutter: false,
            readonly: true,
        };
        let behavior = behavior_for(&widget).expect("behavior");
        let ta = behavior
            .as_any()
            .downcast_ref::<TextArea>()
            .expect("TextArea");
        // A read-only TextArea is not focusable (disabled).
        assert!(!ta.focusable(), "readonly TextArea must be disabled");
    }

    #[test]
    fn translate_action_textarea_changed_maps_to_change_with_full_text() {
        let a = translate_action(
            "document",
            Some(&AppWidget::TextArea {
                key: "document".into(),
                value: "old".into(),
                gutter: false,
                readonly: false,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-document".into(),
                name: "changed".into(),
                payload: Some("new\nbody".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("document", "change", "new\nbody"));
    }

    #[test]
    fn translate_action_table_sorted_normalizes_to_bare_column_index() {
        // The toolkit Table emits the REAL payload "<col>:<dir>" (here "0:asc",
        // exactly as `table.rs::sort_by` produces). The chokepoint must normalize
        // it to the bare column index "0" so every consuming app's bare-u32 parse
        // succeeds — this is the t124 wiring fix. Asserting on the realistic
        // "<col>:<dir>" form (NOT a hand-cleaned "0") is the no-fake-green teeth:
        // before the fix this produced AppWidgetAction(.., "sort", "0:asc").
        let table = AppWidget::Table {
            key: "process_table".into(),
            columns: vec![TableColumn::new("Name"), TableColumn::new("PID")],
            rows: vec![vec!["a".into(), "1".into()]],
            sort: None,
            selection_mode: SelectionMode::Single,
            selected: vec![],
        };
        // Ascending.
        let a = translate_action(
            "process_table",
            Some(&table),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-process_table".into(),
                name: "sorted".into(),
                payload: Some("0:asc".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("process_table", "sort", "0"));

        // Descending (re-click) — the direction suffix is also stripped; the app
        // owns the toggle, so it only needs the column.
        let a = translate_action(
            "process_table",
            Some(&table),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-process_table".into(),
                name: "sorted".into(),
                payload: Some("1:desc".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("process_table", "sort", "1"));

        // A bare column index (no suffix) passes through unchanged — idempotent.
        let a = translate_action(
            "process_table",
            Some(&table),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-process_table".into(),
                name: "sorted".into(),
                payload: Some("0".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("process_table", "sort", "0"));
    }

    #[test]
    fn translate_action_picks_the_right_verb() {
        // Button → click.
        let a = translate_action(
            "save",
            Some(&AppWidget::Button {
                id: "save".into(),
                label: "Save".into(),
                kind: ButtonKind::Normal,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-save".into(),
                name: "click".into(),
                payload: None,
            },
        );
        assert_eq!(a, AppWidgetAction::new("save", "click", ""));

        // Slider changed → change with the value payload.
        let a = translate_action(
            "opacity",
            Some(&AppWidget::Slider {
                key: "opacity".into(),
                min: 0.0,
                max: 1.0,
                step: 0.05,
                value: 0.5,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-opacity".into(),
                name: "changed".into(),
                payload: Some("0.75".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("opacity", "change", "0.75"));

        // Dropdown changed → select with the chosen value.
        let a = translate_action(
            "theme",
            Some(&AppWidget::Dropdown {
                key: "theme".into(),
                options: vec![WidgetOption::new("Light"), WidgetOption::new("Dark")],
                selected: Some("Dark".into()),
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-theme".into(),
                name: "changed".into(),
                payload: Some("Light".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("theme", "select", "Light"));

        // Checkbox → toggle.
        let a = translate_action(
            "wifi",
            Some(&AppWidget::Checkbox {
                key: "wifi".into(),
                checked: false,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-wifi".into(),
                name: "changed".into(),
                payload: Some("true".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("wifi", "toggle", "true"));
    }
}
