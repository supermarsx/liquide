//! `<lq-listbox>` + `<lq-listitem>` — the classic single/multi-select listbox
//! form control with EXPLICIT list-item children (Group GRID: G2).
//!
//! Where [`List`](crate::list::List) is a lightweight selectable row list, the
//! listbox is the Win32/HTML `<select multiple>`-style FORM CONTROL: it owns an
//! ordered set of explicit [`ListItem`]s (each addressable, individually
//! enable/disable-able), a selection model (single OR multi), a keyboard cursor,
//! and **type-ahead** (press a letter → the cursor jumps to the next item whose
//! label starts with it). The `<lq-listitem>` is the formal item element.
//!
//! Behavior:
//! - **Click** an item: selects the item whose LAID-OUT box
//!   (`data-part="item-<i>"`) contains the point — per-item from layout, never
//!   `index * row_height`. Disabled items reject the click.
//! - **Up/Down** move the cursor (and, single-select, the selection), skipping
//!   disabled items; **Home/End** jump to the first/last ENABLED item.
//! - **Space/Enter** select (multi-select: Ctrl+Space toggles) the cursor item;
//!   **Shift+Up/Down** extend a contiguous range (multi-select).
//! - **Type-ahead**: a printable key advances the cursor to the next enabled item
//!   whose label starts (case-insensitively) with that character (wrapping).
//! - Emits `Changed`(comma-joined selected values) on selection change.

use std::collections::BTreeSet;

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the selection changes (payload: comma-joined selected values).
pub const CHANGED_ACTION: &str = "changed";

/// Whether the listbox permits more than one selected item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// At most one item selected.
    Single,
    /// Any number of items (Ctrl toggle, Shift range).
    Multi,
}

/// One explicit listbox item.
#[derive(Debug, Clone)]
pub struct ListItem {
    value: String,
    label: String,
    disabled: bool,
}

impl ListItem {
    /// An enabled item with a value + display label.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// A disabled item (cannot be selected; skipped by keyboard nav).
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Whether the item is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

/// A single/multi-select listbox form control with explicit item children.
#[derive(Debug, Clone)]
pub struct ListBox {
    items: Vec<ListItem>,
    mode: SelectionMode,
    selected: BTreeSet<usize>,
    cursor: Option<usize>,
    anchor: Option<usize>,
    hovered: Option<usize>,
    disabled: bool,
}

impl ListBox {
    /// A single-select listbox over explicit items.
    pub fn new(items: impl IntoIterator<Item = ListItem>) -> Self {
        let items: Vec<ListItem> = items.into_iter().collect();
        let cursor = items.iter().position(|it| !it.disabled);
        Self {
            items,
            mode: SelectionMode::Single,
            selected: BTreeSet::new(),
            cursor,
            anchor: None,
            hovered: None,
            disabled: false,
        }
    }

    /// Convenience: build from `(value, label)` pairs (all enabled).
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self::new(pairs.into_iter().map(|(v, l)| ListItem::new(v, l)))
    }

    /// Allow multi-select.
    pub fn multi(mut self) -> Self {
        self.mode = SelectionMode::Multi;
        self
    }

    /// Pre-select an item by index (clears any prior selection).
    pub fn select(mut self, idx: usize) -> Self {
        if self.items.get(idx).map(|it| !it.disabled).unwrap_or(false) {
            self.selected.clear();
            self.selected.insert(idx);
            self.cursor = Some(idx);
            self.anchor = Some(idx);
        }
        self
    }

    /// Mark the whole listbox disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The selected indices, ascending.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// The selected values, ascending by index.
    pub fn selected_values(&self) -> Vec<String> {
        self.selected
            .iter()
            .filter_map(|&i| self.items.get(i).map(|it| it.value.clone()))
            .collect()
    }

    /// The keyboard cursor (lead) item.
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// Whether `idx` is selected.
    pub fn is_selected(&self, idx: usize) -> bool {
        self.selected.contains(&idx)
    }

    fn item_part(i: usize) -> String {
        format!("item-{i}")
    }

    fn selection_payload(&self) -> String {
        self.selected_values().join(",")
    }

    fn enabled(&self, idx: usize) -> bool {
        self.items.get(idx).map(|it| !it.disabled).unwrap_or(false)
    }

    /// The next enabled index from `from` (inclusive) in direction `dir` (+1/-1);
    /// `None` if none. Does not wrap.
    fn next_enabled(&self, from: usize, dir: i32) -> Option<usize> {
        let n = self.items.len() as i32;
        let mut i = from as i32;
        while i >= 0 && i < n {
            if self.enabled(i as usize) {
                return Some(i as usize);
            }
            i += dir;
        }
        None
    }

    fn first_enabled(&self) -> Option<usize> {
        self.next_enabled(0, 1)
    }

    fn last_enabled(&self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        self.next_enabled(self.items.len() - 1, -1)
    }

    fn select_single(&mut self, idx: usize) -> WidgetOutcome {
        if !self.enabled(idx) {
            return WidgetOutcome::Ignored;
        }
        let changed = self.selected.len() != 1 || !self.selected.contains(&idx);
        self.selected.clear();
        self.selected.insert(idx);
        self.cursor = Some(idx);
        self.anchor = Some(idx);
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
        } else {
            WidgetOutcome::Changed
        }
    }

    fn toggle_one(&mut self, idx: usize) -> WidgetOutcome {
        if !self.enabled(idx) {
            return WidgetOutcome::Ignored;
        }
        if self.selected.contains(&idx) {
            self.selected.remove(&idx);
        } else {
            self.selected.insert(idx);
        }
        self.cursor = Some(idx);
        self.anchor = Some(idx);
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
    }

    fn select_range(&mut self, idx: usize) -> WidgetOutcome {
        if !self.enabled(idx) {
            return WidgetOutcome::Ignored;
        }
        let anchor = self.anchor.unwrap_or(idx);
        let (lo, hi) = if anchor <= idx { (anchor, idx) } else { (idx, anchor) };
        self.selected.clear();
        for i in lo..=hi {
            if self.enabled(i) {
                self.selected.insert(i);
            }
        }
        self.cursor = Some(idx);
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
    }

    /// Type-ahead: move the cursor to the next enabled item (wrapping, starting
    /// AFTER the current cursor) whose label starts with `c` (case-insensitive).
    fn type_ahead(&mut self, c: char) -> WidgetOutcome {
        let n = self.items.len();
        if n == 0 {
            return WidgetOutcome::Ignored;
        }
        let target = c.to_ascii_lowercase();
        let start = self.cursor.map(|c| c + 1).unwrap_or(0);
        for off in 0..n {
            let i = (start + off) % n;
            if !self.enabled(i) {
                continue;
            }
            let label = self.items[i].label.to_ascii_lowercase();
            if label.starts_with(target) {
                self.cursor = Some(i);
                // Type-ahead in single-select also selects (Win32 behavior);
                // multi-select only moves the cursor.
                if self.mode == SelectionMode::Single {
                    return self.select_single(i);
                }
                return WidgetOutcome::Changed;
            }
        }
        WidgetOutcome::Ignored
    }

    /// Which item's LAID-OUT box contains `point`.
    fn item_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.items.len() {
            if let Some(r) = layout.box_of_part(root, &Self::item_part(i)) {
                if r.contains(p) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for ListBox {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Collection
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
            DomEventKind::MouseLeave,
            DomEventKind::Click {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
        ]
    }

    fn on_dom_event(
        &mut self,
        root: NodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                let hit = self.item_at(root, Point::new(*x, *y), layout);
                // Only hover enabled items.
                let hit = hit.filter(|&i| self.enabled(i));
                if hit == self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = hit;
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => match self.item_at(root, Point::new(*x, *y), layout) {
                Some(i) => self.select_single(i),
                None => WidgetOutcome::Ignored,
            },
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled || self.items.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let cur = self.cursor.or_else(|| self.first_enabled()).unwrap_or(0);
        let shift = key.modifiers & keys::modifiers::SHIFT != 0;
        let ctrl = key.modifiers & keys::modifiers::CTRL != 0;

        // Movement keys → next ENABLED cursor item.
        let next = match key.key {
            keys::ARROW_DOWN => self
                .next_enabled(cur + 1, 1)
                .or(Some(cur)),
            keys::ARROW_UP => {
                if cur == 0 {
                    Some(cur)
                } else {
                    self.next_enabled(cur - 1, -1).or(Some(cur))
                }
            }
            keys::HOME => self.first_enabled(),
            keys::END => self.last_enabled(),
            _ => None,
        };
        if let Some(idx) = next {
            return if shift && self.mode == SelectionMode::Multi {
                self.select_range(idx)
            } else if ctrl {
                self.cursor = Some(idx);
                WidgetOutcome::Changed
            } else {
                self.select_single(idx)
            };
        }

        match key.key {
            keys::SPACE if ctrl && self.mode == SelectionMode::Multi => self.toggle_one(cur),
            keys::SPACE | keys::ENTER => self.select_single(cur),
            other => {
                // Type-ahead on a plain printable character (no Ctrl/Alt/Super).
                if key.modifiers
                    & (keys::modifiers::CTRL | keys::modifiers::ALT | keys::modifiers::SUPER)
                    != 0
                {
                    return WidgetOutcome::Ignored;
                }
                match keys::printable_char(other) {
                    Some(c) if !c.is_whitespace() => self.type_ahead(c),
                    _ => WidgetOutcome::Ignored,
                }
            }
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut listbox = TemplateNode::el("lq-listbox")
            .attr("role", "listbox")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);
        if self.mode == SelectionMode::Multi {
            listbox = listbox.attr("aria-multiselectable", "true");
        }

        for (i, item) in self.items.iter().enumerate() {
            let sel = self.selected.contains(&i);
            let is_cursor = self.cursor == Some(i) && !self.disabled && !item.disabled;
            let row = TemplateNode::el("lq-listitem")
                .key(&item.value)
                .attr("data-part", &Self::item_part(i))
                .attr("data-index", &format!("{i}"))
                .attr("data-value", &item.value)
                .attr("role", "option")
                .attr("aria-selected", if sel { "true" } else { "false" })
                .attr(
                    "aria-disabled",
                    if item.disabled { "true" } else { "false" },
                )
                .class_if("selected", sel)
                .class_if("disabled", item.disabled)
                .pseudo_if(PseudoStateFlags::CHECKED, sel)
                .pseudo_if(PseudoStateFlags::FOCUS, is_cursor)
                .pseudo_if(PseudoStateFlags::DISABLED, item.disabled)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(i) && !self.disabled && !item.disabled,
                )
                .child(TemplateNode::text(&item.label));
            listbox = listbox.child(row);
        }
        if self.disabled {
            listbox = listbox.attr("disabled", "true");
        }
        listbox
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
