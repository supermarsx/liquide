//! A richer, toolkit-free app-UI description seam (Seam-0).
//!
//! The flat-text [`crate::AppContentView`] can describe a terminal grid or a
//! styled document, but it cannot express a *widget* UI — a slider, a table
//! with sortable columns, a tabbed panel, a tree. This module adds a plain,
//! `serde`-serialisable description of such a UI: [`AppWidgetModel`].
//!
//! ## No toolkit coupling
//!
//! This is deliberately **plain data**. It does **not** depend on
//! `liquide-widgets`, on the shell's `SceneNode`, on any style/scene type, nor
//! on `liquide-ui-core`. Apps depend only on `liquide-interop`, so describing
//! their UI through this model introduces **no** dependency cycle (the same
//! contract that the existing [`crate::AppContentView`] seam guarantees).
//!
//! The host (`liquide-shell` + `liquide-widgets`) is responsible for turning an
//! [`AppWidgetModel`] into real `<lq-*>` widgets, and for translating the real
//! `liquide-widgets::WidgetAction` back into the toolkit-free
//! [`AppWidgetAction`] it feeds to [`AppWidgetProvider::apply_action`].
//!
//! ## Migration is opt-in
//!
//! [`AppWidgetProvider::widget_model`] defaults to `None`. Terminal and
//! un-migrated apps keep working through the existing
//! [`crate::AppContentView`] text path untouched; an app *opts in* to the
//! widget seam by returning `Some(model)`.

use serde::{Deserialize, Serialize};

/// How a [`Button`](AppWidget::Button) presents itself / what role it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonKind {
    /// The default neutral button.
    Normal,
    /// The emphasised / call-to-action button.
    Primary,
    /// A destructive action (delete, discard).
    Danger,
    /// A low-emphasis text-like button.
    Ghost,
}

impl Default for ButtonKind {
    fn default() -> Self {
        ButtonKind::Normal
    }
}

/// Selection behaviour for a [`List`](AppWidget::List) / [`Table`](AppWidget::Table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    /// No selection is offered.
    None,
    /// At most one item may be selected.
    Single,
    /// Any number of items may be selected.
    Multiple,
}

impl Default for SelectionMode {
    fn default() -> Self {
        SelectionMode::Single
    }
}

/// The direction a [`Table`](AppWidget::Table) is sorted by, for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// The active sort of a [`Table`](AppWidget::Table): which column, which way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSort {
    /// Index into the table's `columns`.
    pub column: u32,
    pub direction: SortDirection,
}

/// A column header in a [`Table`](AppWidget::Table).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableColumn {
    /// The visible header label.
    pub label: String,
    /// Whether the user can sort by this column.
    #[serde(default)]
    pub sortable: bool,
}

impl TableColumn {
    /// A non-sortable column with the given header label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            sortable: false,
        }
    }
}

/// One option in a [`RadioGroup`](AppWidget::RadioGroup),
/// [`Dropdown`](AppWidget::Dropdown), or [`Segmented`](AppWidget::Segmented):
/// a stable value plus a human label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetOption {
    /// The stable machine value reported back in an action payload.
    pub value: String,
    /// The visible label.
    pub label: String,
}

impl WidgetOption {
    /// An option whose machine value and label are the same string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: value.clone(),
            value,
        }
    }

    /// An option with a distinct machine value and visible label.
    #[must_use]
    pub fn labelled(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// A node in a [`Tree`](AppWidget::Tree).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeNode {
    /// A stable identifier for this node (reported on expand/select).
    pub id: String,
    /// The visible label.
    pub label: String,
    /// Whether the node is currently expanded.
    #[serde(default)]
    pub expanded: bool,
    /// Child nodes (empty = leaf).
    #[serde(default)]
    pub children: Vec<TreeNode>,
}

/// A single section of an [`Accordion`](AppWidget::Accordion).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccordionSection {
    /// Stable identifier for the section.
    pub id: String,
    /// The header label.
    pub title: String,
    /// Whether the section is currently expanded.
    #[serde(default)]
    pub expanded: bool,
    /// The section body.
    #[serde(default)]
    pub children: Vec<AppWidget>,
}

/// Where a [`WasmApp`](AppWidget::WasmApp)'s module bytes come from.
///
/// This is **plain data** — a path the host loads, or the bytes inline. It does
/// NOT depend on `liquide-wasm-host`; the shell turns it into a real
/// `WasmHost`/`NullWasmHost` at mount time. Keeping it here (interop) means an
/// app can describe a WASM-backed sub-UI without depending on the runtime crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WasmModuleSource {
    /// A filesystem path the host reads the `.wasm` bytes from at mount time.
    Path { path: String },
    /// The module bytes inlined directly (e.g. embedded / already fetched).
    Bytes {
        #[serde(with = "serde_bytes_vec")]
        bytes: Vec<u8>,
    },
}

/// `serde` adapter so inline WASM `bytes` round-trip as a JSON array of `u8`
/// (portable, no base64 dependency) while staying a `Vec<u8>` in Rust.
mod serde_bytes_vec {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(bytes.iter().copied())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        Vec::<u8>::deserialize(d)
    }
}

/// The authoring language of a [`ScriptApp`](AppWidget::ScriptApp)'s `source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLang {
    /// TypeScript — the host transpiles (type-strips) it to JS before running.
    TypeScript,
    /// Plain JavaScript — run as-is (still passes through the transpile step,
    /// which is a no-op for type-free JS).
    JavaScript,
}

impl Default for ScriptLang {
    fn default() -> Self {
        ScriptLang::TypeScript
    }
}

/// One tab of a [`Tabs`](AppWidget::Tabs) container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    /// Stable identifier for the tab.
    pub id: String,
    /// The tab's visible label.
    pub label: String,
    /// The tab's body content.
    #[serde(default)]
    pub children: Vec<AppWidget>,
}

/// A single, plain-data widget node.
///
/// This is a tree: container variants (`Panel`, `Card`, `GroupBox`, `Tabs`,
/// `Toolbar`, `Accordion`, `Segmented`) carry `children`/sub-trees; leaf
/// variants describe a single control. Every *interactive* control carries a
/// stable `key`/`id` (a `String`) plus its current value, so the host can
/// route an [`AppWidgetAction`] back to the right place and the app can update
/// the value in place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppWidget {
    // ---- containers --------------------------------------------------------
    /// A bare grouping container.
    Panel { children: Vec<AppWidget> },
    /// A titled, elevated container.
    Card {
        #[serde(default)]
        title: Option<String>,
        children: Vec<AppWidget>,
    },
    /// A labelled frame around its children.
    GroupBox {
        label: String,
        children: Vec<AppWidget>,
    },
    /// A tabbed container; `selected` indexes `tabs`.
    Tabs {
        tabs: Vec<Tab>,
        #[serde(default)]
        selected: u32,
    },
    /// A horizontal strip of controls (typically buttons).
    Toolbar { children: Vec<AppWidget> },
    /// Stacked collapsible sections.
    Accordion { sections: Vec<AccordionSection> },

    // ---- text / static -----------------------------------------------------
    /// Static, non-interactive text.
    Label { text: String },
    /// A hyperlink.
    Link {
        text: String,
        href: String,
    },
    /// A small status / tag pill.
    Chip { label: String },
    /// A breadcrumb trail (root → … → current).
    Breadcrumb { crumbs: Vec<String> },

    // ---- buttons -----------------------------------------------------------
    /// A clickable button.
    Button {
        id: String,
        label: String,
        #[serde(default)]
        kind: ButtonKind,
    },
    /// An exclusive set of button-styled options; `selected` is the chosen value.
    Segmented {
        key: String,
        options: Vec<WidgetOption>,
        #[serde(default)]
        selected: Option<String>,
    },

    // ---- inputs ------------------------------------------------------------
    /// A single-line text field.
    TextInput { key: String, value: String },
    /// A multi-line text field / editor body. `value` is the full text with
    /// lines joined by `\n`. `gutter` shows line numbers; `readonly` disables
    /// editing.
    TextArea {
        key: String,
        value: String,
        #[serde(default)]
        gutter: bool,
        #[serde(default)]
        readonly: bool,
    },
    /// A boolean checkbox.
    Checkbox { key: String, checked: bool },
    /// A boolean switch/toggle.
    Switch { key: String, checked: bool },
    /// An exclusive radio group; `selected` is the chosen option value.
    RadioGroup {
        key: String,
        options: Vec<WidgetOption>,
        #[serde(default)]
        selected: Option<String>,
    },
    /// A numeric slider.
    Slider {
        key: String,
        min: f64,
        max: f64,
        step: f64,
        value: f64,
    },
    /// A dropdown / select; `selected` is the chosen option value.
    Dropdown {
        key: String,
        options: Vec<WidgetOption>,
        #[serde(default)]
        selected: Option<String>,
    },

    // ---- collections -------------------------------------------------------
    /// A flat list of items.
    List {
        key: String,
        items: Vec<String>,
        #[serde(default)]
        selection_mode: SelectionMode,
        /// Indices of the currently-selected items.
        #[serde(default)]
        selected: Vec<u32>,
    },
    /// A tabular grid.
    Table {
        key: String,
        columns: Vec<TableColumn>,
        /// Each row is a vector of cell strings (column-aligned).
        rows: Vec<Vec<String>>,
        #[serde(default)]
        sort: Option<TableSort>,
        #[serde(default)]
        selection_mode: SelectionMode,
        #[serde(default)]
        selected: Vec<u32>,
    },
    /// A hierarchical tree.
    Tree { key: String, nodes: Vec<TreeNode> },

    // ---- indicators / navigation ------------------------------------------
    /// A determinate progress bar; `value` in `0.0..=1.0`.
    Progress { value: f64 },
    /// A pager; `page` is the current 0-based page of `pages` total.
    Pagination { key: String, page: u32, pages: u32 },

    // ---- embedded runtimes (t152) -----------------------------------------
    /// A sub-UI produced by an untrusted **WASM** module. The host loads
    /// `module` into a sandboxed `liquide-wasm-host`, calls `render()` to obtain
    /// an inner [`AppWidgetModel`], and mounts THAT through the normal widget
    /// pipeline. Actions targeting widgets inside the emitted UI are routed back
    /// through the host's `apply_action()`. This is **plain data**: it carries
    /// only the module source, never a runtime handle.
    WasmApp { module: WasmModuleSource },
    /// A sub-UI produced by an untrusted **TypeScript/JavaScript** module. The
    /// host transpiles + runs `source` (per `lang`) in a sandboxed
    /// `liquide-script-host`, calls `render()` to obtain an inner
    /// [`AppWidgetModel`], and mounts THAT through the normal widget pipeline.
    /// Like [`WasmApp`](AppWidget::WasmApp) this is plain data.
    ScriptApp {
        source: String,
        #[serde(default)]
        lang: ScriptLang,
    },
}

impl AppWidget {
    /// The control's stable interaction key/id, if it is interactive.
    ///
    /// Returns the `key` for keyed controls and the `id` for [`Button`](AppWidget::Button);
    /// containers and static controls return `None`.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        match self {
            AppWidget::Button { id, .. } => Some(id),
            AppWidget::Segmented { key, .. }
            | AppWidget::TextInput { key, .. }
            | AppWidget::TextArea { key, .. }
            | AppWidget::Checkbox { key, .. }
            | AppWidget::Switch { key, .. }
            | AppWidget::RadioGroup { key, .. }
            | AppWidget::Slider { key, .. }
            | AppWidget::Dropdown { key, .. }
            | AppWidget::List { key, .. }
            | AppWidget::Table { key, .. }
            | AppWidget::Tree { key, .. }
            | AppWidget::Pagination { key, .. } => Some(key),
            _ => None,
        }
    }

    /// The direct children of a container variant, for traversal.
    fn children_mut(&mut self) -> Option<&mut Vec<AppWidget>> {
        match self {
            AppWidget::Panel { children }
            | AppWidget::Card { children, .. }
            | AppWidget::GroupBox { children, .. }
            | AppWidget::Toolbar { children } => Some(children),
            _ => None,
        }
    }

    /// Depth-first find of the interactive widget whose key/id equals `key`.
    #[must_use]
    pub fn find_mut(&mut self, key: &str) -> Option<&mut AppWidget> {
        if self.key() == Some(key) {
            return Some(self);
        }
        // Recurse into the structural sub-trees that `children_mut` doesn't cover.
        match self {
            AppWidget::Tabs { tabs, .. } => {
                for tab in tabs {
                    for child in &mut tab.children {
                        if let Some(found) = child.find_mut(key) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            AppWidget::Accordion { sections } => {
                for section in sections {
                    for child in &mut section.children {
                        if let Some(found) = child.find_mut(key) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            other => {
                let children = other.children_mut()?;
                for child in children {
                    if let Some(found) = child.find_mut(key) {
                        return Some(found);
                    }
                }
                None
            }
        }
    }
}

/// The root of an app's widget UI: a title plus a tree of widgets.
///
/// This is the plain-data description the host turns into real `<lq-*>`
/// widgets, and the model the app mutates in response to an
/// [`AppWidgetAction`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppWidgetModel {
    /// Optional window/content title.
    #[serde(default)]
    pub title: Option<String>,
    /// The top-level widget tree.
    pub root: Vec<AppWidget>,
}

impl AppWidgetModel {
    /// An empty model.
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            root: Vec::new(),
        }
    }

    /// A model with the given top-level widgets.
    #[must_use]
    pub fn with_root(root: Vec<AppWidget>) -> Self {
        Self { title: None, root }
    }

    /// Find an interactive widget by its key/id anywhere in the tree.
    #[must_use]
    pub fn find_mut(&mut self, key: &str) -> Option<&mut AppWidget> {
        self.root.iter_mut().find_map(|w| w.find_mut(key))
    }
}

impl Default for AppWidgetModel {
    fn default() -> Self {
        Self::new()
    }
}

/// A toolkit-free action delivered from the host into an app's widget model.
///
/// The host translates the real `liquide-widgets::WidgetAction` (a toolkit
/// type the app must not see) into this plain triple. `widget` is the target
/// control's `key`/`id`; `name` is the verb (e.g. `"click"`, `"change"`,
/// `"toggle"`, `"select"`, `"sort"`, `"navigate"`); `payload` is the verb's
/// data (e.g. the new text, the chosen value, the slider position as a string,
/// the selected index). Keeping these as plain strings avoids leaking any
/// toolkit enum across the seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppWidgetAction {
    /// The target control's stable `key`/`id`.
    pub widget: String,
    /// The action verb (e.g. `"click"`, `"change"`, `"toggle"`).
    pub name: String,
    /// The verb's data, as a plain string (empty when the verb carries none).
    #[serde(default)]
    pub payload: String,
}

impl AppWidgetAction {
    /// Construct an action.
    #[must_use]
    pub fn new(
        widget: impl Into<String>,
        name: impl Into<String>,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            widget: widget.into(),
            name: name.into(),
            payload: payload.into(),
        }
    }
}

/// An app's optional widget-UI seam.
///
/// This is a *super-trait extension* of the existing [`crate::AppView`] content
/// path, not a replacement: an app may expose a [`crate::AppContentView`] (text
/// path), an [`AppWidgetModel`] (widget path), or both. The default
/// [`widget_model`](AppWidgetProvider::widget_model) returns `None` so terminal
/// and un-migrated apps keep using the text path unchanged.
pub trait AppWidgetProvider {
    /// The current widget UI, or `None` to fall back to the text content path.
    ///
    /// Defaults to `None` (no widget UI).
    fn widget_model(&self) -> Option<AppWidgetModel> {
        None
    }

    /// Apply a host-delivered action to the model.
    ///
    /// Returns `true` if the model changed (and the window should be redrawn).
    /// The default implementation is a no-op returning `false`, so an app that
    /// doesn't expose a widget model need not implement it.
    fn apply_action(&mut self, action: &AppWidgetAction) -> bool {
        let _ = action;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a representative model exercising several control families.
    fn sample_model() -> AppWidgetModel {
        AppWidgetModel {
            title: Some("Settings".into()),
            root: vec![
                AppWidget::Card {
                    title: Some("Appearance".into()),
                    children: vec![
                        AppWidget::Label {
                            text: "Theme".into(),
                        },
                        AppWidget::Dropdown {
                            key: "theme".into(),
                            options: vec![
                                WidgetOption::new("Light"),
                                WidgetOption::new("Dark"),
                            ],
                            selected: Some("Dark".into()),
                        },
                        AppWidget::Slider {
                            key: "opacity".into(),
                            min: 0.0,
                            max: 1.0,
                            step: 0.05,
                            value: 0.5,
                        },
                        AppWidget::Switch {
                            key: "reduce_motion".into(),
                            checked: false,
                        },
                    ],
                },
                AppWidget::Tabs {
                    tabs: vec![Tab {
                        id: "general".into(),
                        label: "General".into(),
                        children: vec![
                            AppWidget::TextInput {
                                key: "username".into(),
                                value: "ada".into(),
                            },
                            AppWidget::TextArea {
                                key: "bio".into(),
                                value: "line one\nline two".into(),
                                gutter: true,
                                readonly: false,
                            },
                        ],
                    }],
                    selected: 0,
                },
                AppWidget::Table {
                    key: "processes".into(),
                    columns: vec![
                        TableColumn::new("PID"),
                        TableColumn {
                            label: "CPU".into(),
                            sortable: true,
                        },
                    ],
                    rows: vec![
                        vec!["1".into(), "0.1".into()],
                        vec!["2".into(), "9.3".into()],
                    ],
                    sort: None,
                    selection_mode: SelectionMode::Single,
                    selected: vec![],
                },
                AppWidget::Button {
                    id: "save".into(),
                    label: "Save".into(),
                    kind: ButtonKind::Primary,
                },
            ],
        }
    }

    #[test]
    fn model_round_trips_through_serde_json() {
        let model = sample_model();
        let json = serde_json::to_string(&model).expect("serialize");
        let back: AppWidgetModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(model, back);
    }

    #[test]
    fn textarea_node_round_trips_through_serde_json() {
        // A multi-line TextArea must serialize and deserialize byte-for-byte,
        // including its newline-bearing value and the gutter/readonly flags.
        let node = AppWidget::TextArea {
            key: "doc".into(),
            value: "first\nsecond\nthird".into(),
            gutter: true,
            readonly: true,
        };
        let json = serde_json::to_string(&node).expect("serialize");
        // The tagged enum names the variant in snake_case.
        assert!(json.contains("\"type\":\"text_area\""), "tag: {json}");
        let back: AppWidget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(node, back);
        // The value's newlines survive the round-trip.
        assert!(matches!(
            back,
            AppWidget::TextArea { value, gutter: true, readonly: true, .. }
                if value == "first\nsecond\nthird"
        ));
    }

    #[test]
    fn wasm_app_node_round_trips_through_serde_json() {
        // Path form.
        let node = AppWidget::WasmApp {
            module: WasmModuleSource::Path {
                path: "apps/clock.wasm".into(),
            },
        };
        let json = serde_json::to_string(&node).expect("serialize");
        assert!(json.contains("\"type\":\"wasm_app\""), "tag: {json}");
        assert!(json.contains("\"kind\":\"path\""), "module tag: {json}");
        let back: AppWidget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(node, back);

        // Inline-bytes form (a tiny wasm header) round-trips byte-for-byte.
        let node = AppWidget::WasmApp {
            module: WasmModuleSource::Bytes {
                bytes: vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00],
            },
        };
        let json = serde_json::to_string(&node).expect("serialize");
        let back: AppWidget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(node, back);
        assert!(matches!(
            back,
            AppWidget::WasmApp { module: WasmModuleSource::Bytes { bytes } }
                if bytes == vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
        ));
    }

    #[test]
    fn script_app_node_round_trips_through_serde_json() {
        let node = AppWidget::ScriptApp {
            source: "export function render(){ return { root: [] }; }".into(),
            lang: ScriptLang::TypeScript,
        };
        let json = serde_json::to_string(&node).expect("serialize");
        assert!(json.contains("\"type\":\"script_app\""), "tag: {json}");
        assert!(json.contains("\"lang\":\"type_script\""), "lang: {json}");
        let back: AppWidget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(node, back);

        // `lang` defaults to TypeScript when omitted.
        let back: AppWidget =
            serde_json::from_str(r#"{"type":"script_app","source":"x"}"#).expect("deserialize");
        assert!(matches!(
            back,
            AppWidget::ScriptApp { lang: ScriptLang::TypeScript, .. }
        ));
    }

    #[test]
    fn find_mut_recurses_past_embedded_runtime_nodes() {
        // A WasmApp / ScriptApp node has NO interop-level key and NO interop-level
        // children (its inner model only materialises at host render time), so a
        // find_mut walk neither matches them nor crashes on them, and STILL finds
        // a keyed sibling inside the same container.
        let mut model = AppWidgetModel::with_root(vec![AppWidget::Panel {
            children: vec![
                AppWidget::WasmApp {
                    module: WasmModuleSource::Path {
                        path: "a.wasm".into(),
                    },
                },
                AppWidget::ScriptApp {
                    source: "x".into(),
                    lang: ScriptLang::JavaScript,
                },
                AppWidget::Button {
                    id: "ok".into(),
                    label: "OK".into(),
                    kind: ButtonKind::Primary,
                },
            ],
        }]);
        // The runtime nodes are not keyed and are skipped.
        assert!(model.find_mut("a.wasm").is_none());
        // The keyed sibling is still reachable past them.
        assert!(matches!(
            model.find_mut("ok"),
            Some(AppWidget::Button { .. })
        ));
    }

    #[test]
    fn action_round_trips_through_serde_json() {
        let action = AppWidgetAction::new("opacity", "change", "0.75");
        let json = serde_json::to_string(&action).expect("serialize");
        let back: AppWidgetAction = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(action, back);
    }

    #[test]
    fn find_mut_locates_nested_keyed_widget() {
        let mut model = sample_model();
        // Inside a Card.
        assert!(matches!(
            model.find_mut("opacity"),
            Some(AppWidget::Slider { .. })
        ));
        // Inside a Tab sub-tree (not covered by plain children traversal).
        assert!(matches!(
            model.find_mut("username"),
            Some(AppWidget::TextInput { .. })
        ));
        // Multi-line TextArea, also inside the Tab sub-tree.
        assert!(matches!(
            model.find_mut("bio"),
            Some(AppWidget::TextArea { gutter: true, .. })
        ));
        // Button by id.
        assert!(matches!(
            model.find_mut("save"),
            Some(AppWidget::Button { .. })
        ));
        // Missing key.
        assert!(model.find_mut("nope").is_none());
    }

    /// A model-backed app that applies actions by mutating in place. A
    /// fake/empty `apply_action` (the default `false` no-op) would FAIL every
    /// mutation assertion below.
    struct WidgetApp {
        model: AppWidgetModel,
    }

    impl AppWidgetProvider for WidgetApp {
        fn widget_model(&self) -> Option<AppWidgetModel> {
            Some(self.model.clone())
        }

        fn apply_action(&mut self, action: &AppWidgetAction) -> bool {
            let Some(widget) = self.model.find_mut(&action.widget) else {
                return false;
            };
            match (widget, action.name.as_str()) {
                (AppWidget::Slider { value, .. }, "change") => {
                    if let Ok(v) = action.payload.parse::<f64>() {
                        *value = v;
                        return true;
                    }
                    false
                }
                (AppWidget::Switch { checked, .. }, "toggle")
                | (AppWidget::Checkbox { checked, .. }, "toggle") => {
                    *checked = !*checked;
                    true
                }
                (AppWidget::TextInput { value, .. }, "change") => {
                    *value = action.payload.clone();
                    true
                }
                (AppWidget::TextArea { value, .. }, "change") => {
                    *value = action.payload.clone();
                    true
                }
                (AppWidget::Dropdown { selected, .. }, "select") => {
                    *selected = Some(action.payload.clone());
                    true
                }
                _ => false,
            }
        }
    }

    #[test]
    fn apply_action_mutates_the_model() {
        let mut app = WidgetApp {
            model: super::tests::sample_model(),
        };

        // Slider change.
        assert!(app.apply_action(&AppWidgetAction::new("opacity", "change", "0.75")));
        assert!(matches!(
            app.model.find_mut("opacity"),
            Some(AppWidget::Slider { value, .. }) if (*value - 0.75).abs() < f64::EPSILON
        ));

        // Switch toggle (false -> true).
        assert!(app.apply_action(&AppWidgetAction::new("reduce_motion", "toggle", "")));
        assert!(matches!(
            app.model.find_mut("reduce_motion"),
            Some(AppWidget::Switch { checked: true, .. })
        ));

        // Text input change (inside a Tab).
        assert!(app.apply_action(&AppWidgetAction::new("username", "change", "grace")));
        assert!(matches!(
            app.model.find_mut("username"),
            Some(AppWidget::TextInput { value, .. }) if value == "grace"
        ));

        // TextArea change carrying a multi-line payload (must preserve the newline).
        assert!(app.apply_action(&AppWidgetAction::new(
            "bio",
            "change",
            "alpha\nbeta\ngamma"
        )));
        assert!(matches!(
            app.model.find_mut("bio"),
            Some(AppWidget::TextArea { value, .. }) if value == "alpha\nbeta\ngamma"
        ));

        // Dropdown select.
        assert!(app.apply_action(&AppWidgetAction::new("theme", "select", "Light")));
        assert!(matches!(
            app.model.find_mut("theme"),
            Some(AppWidget::Dropdown { selected: Some(s), .. }) if s == "Light"
        ));
    }

    #[test]
    fn apply_action_reports_no_change_for_unknown_widget() {
        let mut app = WidgetApp {
            model: super::tests::sample_model(),
        };
        assert!(!app.apply_action(&AppWidgetAction::new("ghost", "click", "")));
    }

    #[test]
    fn default_provider_returns_none_and_no_change() {
        struct TextOnly;
        impl AppWidgetProvider for TextOnly {}
        let mut app = TextOnly;
        assert!(app.widget_model().is_none());
        assert!(!app.apply_action(&AppWidgetAction::new("x", "click", "")));
    }
}
