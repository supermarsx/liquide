//! `<lq-list>` — a vertical, selectable item list (Group C: C1).
//!
//! State: N (value, label) items + a selection set + a "lead"/cursor index for
//! keyboard navigation + range-anchor for Shift-select. Behavior:
//!
//! - **Click** an item: the item whose LAID-OUT row box (`data-part="item-<i>"`)
//!   contains the point becomes the sole selection — hit-tested per-row from the
//!   real layout, NEVER an index computed from `index * row_height`. The two
//!   drift the moment CSS changes the row height; reading the laid-out box is the
//!   structural guard. Mouse clicks always single-select: the DOM mouse event
//!   model carries NO keyboard modifiers (`DomEventKind::Click` has only
//!   button + x + y), so modifier-driven multi-select rides the KEYBOARD path,
//!   where `KeyInput` carries real `modifiers`.
//! - **Shift+Arrow**: selects the contiguous range from the anchor to the cursor
//!   row (multi-select range).
//! - **Ctrl+Space**: toggles the cursor row in/out of the selection set
//!   (multi-select toggle), preserving the rest; **Ctrl+Arrow** moves the cursor
//!   without changing the selection.
//! - **Up/Down** move the cursor (and, without a modifier, the single selection);
//!   **Home/End** jump to first/last; **Space/Enter** select the cursor row.
//! - Selected rows carry `:checked` (CSS `.selected`-equivalent restyle) +
//!   `aria-selected`; the cursor row carries `:focus`; hover carries `:hover`.
//! - Emits a `Changed`(comma-joined selected values) Action whenever the
//!   selection changes.
//!
//! Long lists scroll: the list mounts inside the `lq-scroll-area` mechanism via
//! CSS `overflow` on a wrapper — but the list itself stays a flat row list so the
//! per-row hit-test is unaffected (the scroll offset is applied by the scroll
//! container, and `LayoutQuery` reads post-scroll screen-space boxes).

use std::collections::BTreeSet;

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action a list emits when the selection changes. The payload is the
/// comma-joined selected option values, in ascending index order.
pub const CHANGED_ACTION: &str = "changed";

/// Whether a list permits more than one selected row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// At most one row selected at a time.
    Single,
    /// Any number of rows selected (Ctrl toggle, Shift range).
    Multi,
}

/// A vertical, selectable item list.
#[derive(Debug, Clone)]
pub struct List {
    /// (value, label) items in order.
    items: Vec<(String, String)>,
    /// Selection mode.
    mode: SelectionMode,
    /// The selected row indices (a set so multi-select is order-independent).
    selected: BTreeSet<usize>,
    /// The keyboard cursor / lead row (the focus row, also the range pivot's
    /// moving end). `None` when the list is empty.
    cursor: Option<usize>,
    /// The Shift-range anchor (the fixed end of a range selection).
    anchor: Option<usize>,
    /// The hovered row, if any.
    hovered: Option<usize>,
    disabled: bool,
}

impl List {
    /// A single-select list over `(value, label)` items.
    pub fn new(items: impl IntoIterator<Item = (String, String)>) -> Self {
        let items: Vec<(String, String)> = items.into_iter().collect();
        let cursor = if items.is_empty() { None } else { Some(0) };
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

    /// Allow multi-select (Ctrl toggle, Shift range).
    pub fn multi(mut self) -> Self {
        self.mode = SelectionMode::Multi;
        self
    }

    /// Pre-select a row by index (clears any prior selection).
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.items.len() {
            self.selected.clear();
            self.selected.insert(idx);
            self.cursor = Some(idx);
            self.anchor = Some(idx);
        }
        self
    }

    /// Mark the whole list disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The selected indices, ascending.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// The selected values, in ascending index order.
    pub fn selected_values(&self) -> Vec<String> {
        self.selected
            .iter()
            .filter_map(|&i| self.items.get(i).map(|(v, _)| v.clone()))
            .collect()
    }

    /// The keyboard cursor (lead) row.
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

    /// The comma-joined selected values for the Changed action payload.
    fn selection_payload(&self) -> String {
        self.selected_values().join(",")
    }

    /// Replace the selection with the single row `idx`.
    fn select_single(&mut self, idx: usize) -> WidgetOutcome {
        let changed = self.selected.len() != 1 || !self.selected.contains(&idx);
        self.selected.clear();
        self.selected.insert(idx);
        self.cursor = Some(idx);
        self.anchor = Some(idx);
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
        } else {
            // Re-affirming the same single selection still moves the cursor; emit
            // Changed so the cursor :focus restyles, but no semantic change.
            WidgetOutcome::Changed
        }
    }

    /// Toggle the row `idx` in/out of the selection (Ctrl semantics).
    fn toggle_one(&mut self, idx: usize) -> WidgetOutcome {
        if self.selected.contains(&idx) {
            self.selected.remove(&idx);
        } else {
            self.selected.insert(idx);
        }
        self.cursor = Some(idx);
        self.anchor = Some(idx);
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
    }

    /// Select the contiguous range from the anchor to `idx` (Shift semantics),
    /// replacing the current selection.
    fn select_range(&mut self, idx: usize) -> WidgetOutcome {
        let anchor = self.anchor.unwrap_or(idx);
        let (lo, hi) = if anchor <= idx { (anchor, idx) } else { (idx, anchor) };
        self.selected.clear();
        for i in lo..=hi {
            self.selected.insert(i);
        }
        // The cursor moves to the active end; the anchor stays fixed.
        self.cursor = Some(idx);
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
    }

    /// Which row's LAID-OUT box contains `point` (per-row hit from layout).
    fn row_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.items.len() {
            if let Some(r) = layout.box_of_part(root, &Self::item_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for List {
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
                let hit = self.row_at(root, Point::new(*x, *y), layout);
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
            } => match self.row_at(root, Point::new(*x, *y), layout) {
                // Mouse clicks single-select (the DOM mouse event carries no
                // modifiers); modifier-driven range/toggle ride the keyboard path.
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
        let n = self.items.len();
        let cur = self.cursor.unwrap_or(0);
        let shift = key.modifiers & keys::modifiers::SHIFT != 0;
        let ctrl = key.modifiers & keys::modifiers::CTRL != 0;

        // Movement keys compute the next cursor row.
        let next = match key.key {
            keys::ARROW_DOWN => Some((cur + 1).min(n - 1)),
            keys::ARROW_UP => Some(cur.saturating_sub(1)),
            keys::HOME => Some(0),
            keys::END => Some(n - 1),
            _ => None,
        };
        if let Some(idx) = next {
            return if shift && self.mode == SelectionMode::Multi {
                self.select_range(idx)
            } else if ctrl {
                // Ctrl+move shifts the cursor without changing the selection.
                self.cursor = Some(idx);
                WidgetOutcome::Changed
            } else {
                self.select_single(idx)
            };
        }

        // Activation keys select / toggle the cursor row. Ctrl+Space toggles the
        // cursor row in/out of a multi-selection; plain Space/Enter single-selects.
        match key.key {
            keys::SPACE if ctrl && self.mode == SelectionMode::Multi => self.toggle_one(cur),
            keys::SPACE | keys::ENTER => self.select_single(cur),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut list = TemplateNode::el("lq-list")
            .attr("role", "listbox")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);
        if self.mode == SelectionMode::Multi {
            list = list.attr("aria-multiselectable", "true");
        }

        for (i, (value, label)) in self.items.iter().enumerate() {
            let sel = self.selected.contains(&i);
            let is_cursor = self.cursor == Some(i) && !self.disabled;
            let row = TemplateNode::el("lq-list-item")
                .key(value)
                .attr("data-part", &Self::item_part(i))
                .attr("data-index", &format!("{i}"))
                .attr("data-value", value)
                .attr("role", "option")
                .attr("aria-selected", if sel { "true" } else { "false" })
                .class_if("selected", sel)
                // :checked = selected row (CSS restyles the selection fill);
                // :focus = cursor row; :hover = hovered row.
                .pseudo_if(PseudoStateFlags::CHECKED, sel)
                .pseudo_if(PseudoStateFlags::FOCUS, is_cursor)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(i) && !self.disabled,
                )
                .child(TemplateNode::text(label));
            list = list.child(row);
        }
        if self.disabled {
            list = list.attr("disabled", "true");
        }
        list
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
