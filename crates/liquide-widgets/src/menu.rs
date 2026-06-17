//! `<lq-menu>` — a vertical menu of activatable items (Group C: C4).
//!
//! A reusable in-app menu widget (NOT the shell's chrome context-menu, which is
//! left untouched). State: an ordered list of [`MenuEntry`] (item / separator) +
//! a highlighted index (the keyboard cursor / hover). Behavior:
//!
//! - **Hover** an item: highlights it (`:hover`); separators + disabled items are
//!   never highlighted.
//! - **Click** an enabled item: emits an `Activate`(item id) Action. Click on a
//!   separator/disabled item is ignored. Hit-tested per-item from the LAID-OUT
//!   item box (`data-part="item-<i>"`) — the recurring menu-geometry-from-CSS
//!   guard: never `index * item_height`.
//! - **Up/Down** move the highlight across ENABLED items only (skipping
//!   separators + disabled), wrapping; **Home/End** jump to first/last enabled;
//!   **Enter** activates the highlighted item; **Esc** emits a `Dismiss` Action.
//! - The highlighted item carries `:focus` (+ `.highlighted`); disabled items
//!   `:disabled`. A submenu parent shows a `.has-submenu` marker (CSS ::after
//!   arrow) and Right opens it (emitted as a `Submenu`(id) Action for the owner).

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when an item is activated (payload: the item id).
pub const ACTIVATE_ACTION: &str = "activate";
/// Emitted when the menu is dismissed via Esc (no payload).
pub const DISMISS_ACTION: &str = "dismiss";
/// Emitted when a submenu parent is opened via Right (payload: the item id).
pub const SUBMENU_ACTION: &str = "submenu";

/// One entry in a menu.
#[derive(Debug, Clone)]
pub enum MenuEntry {
    /// An activatable item.
    Item {
        /// Stable id emitted on activation.
        id: String,
        /// Display label.
        label: String,
        /// Whether the item is disabled (un-activatable, never highlighted).
        disabled: bool,
        /// Whether the item opens a submenu (shows a marker; Right opens).
        has_submenu: bool,
    },
    /// A non-interactive divider.
    Separator,
}

impl MenuEntry {
    /// An enabled item with `id` and `label`.
    pub fn item(id: impl Into<String>, label: impl Into<String>) -> Self {
        MenuEntry::Item {
            id: id.into(),
            label: label.into(),
            disabled: false,
            has_submenu: false,
        }
    }

    /// A disabled item.
    pub fn disabled_item(id: impl Into<String>, label: impl Into<String>) -> Self {
        MenuEntry::Item {
            id: id.into(),
            label: label.into(),
            disabled: true,
            has_submenu: false,
        }
    }

    /// An item that opens a submenu.
    pub fn submenu(id: impl Into<String>, label: impl Into<String>) -> Self {
        MenuEntry::Item {
            id: id.into(),
            label: label.into(),
            disabled: false,
            has_submenu: true,
        }
    }

    /// A separator.
    pub fn separator() -> Self {
        MenuEntry::Separator
    }

    fn is_activatable(&self) -> bool {
        matches!(
            self,
            MenuEntry::Item {
                disabled: false,
                ..
            }
        )
    }
}

/// A vertical menu widget.
#[derive(Debug, Clone, Default)]
pub struct Menu {
    entries: Vec<MenuEntry>,
    /// The highlighted entry index (keyboard cursor / hover), if any.
    highlighted: Option<usize>,
}

impl Menu {
    /// An empty menu.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from a list of entries.
    pub fn with_entries(entries: impl IntoIterator<Item = MenuEntry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            highlighted: None,
        }
    }

    /// Append an entry.
    pub fn entry(mut self, e: MenuEntry) -> Self {
        self.entries.push(e);
        self
    }

    /// The highlighted entry index.
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// The id of the highlighted item, if it is an item.
    pub fn highlighted_id(&self) -> Option<&str> {
        match self.highlighted.and_then(|i| self.entries.get(i)) {
            Some(MenuEntry::Item { id, .. }) => Some(id.as_str()),
            _ => None,
        }
    }

    /// Number of entries (items + separators).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the menu has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn item_part(i: usize) -> String {
        format!("item-{i}")
    }

    /// The first activatable index at or after `from` (wrapping once); `None` if
    /// no activatable items exist.
    fn next_activatable(&self, from: usize, forward: bool) -> Option<usize> {
        let n = self.entries.len();
        if n == 0 {
            return None;
        }
        for step in 1..=n {
            let idx = if forward {
                (from + step) % n
            } else {
                (from + n - step) % n
            };
            if self.entries[idx].is_activatable() {
                return Some(idx);
            }
        }
        None
    }

    fn first_activatable(&self) -> Option<usize> {
        (0..self.entries.len()).find(|&i| self.entries[i].is_activatable())
    }

    fn last_activatable(&self) -> Option<usize> {
        (0..self.entries.len())
            .rev()
            .find(|&i| self.entries[i].is_activatable())
    }

    /// Set the highlight to `idx` (only if it is an activatable item).
    fn highlight(&mut self, idx: Option<usize>) -> WidgetOutcome {
        let idx = idx.filter(|&i| self.entries.get(i).is_some_and(MenuEntry::is_activatable));
        if idx == self.highlighted {
            return WidgetOutcome::Ignored;
        }
        self.highlighted = idx;
        WidgetOutcome::Changed
    }

    /// Activate the item at `idx` if it is enabled.
    fn activate(&mut self, idx: usize) -> WidgetOutcome {
        match self.entries.get(idx) {
            Some(MenuEntry::Item {
                id,
                disabled: false,
                has_submenu,
                ..
            }) => {
                self.highlighted = Some(idx);
                if *has_submenu {
                    WidgetOutcome::action_with(SUBMENU_ACTION, id.clone())
                } else {
                    WidgetOutcome::action_with(ACTIVATE_ACTION, id.clone())
                }
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    /// Which item's LAID-OUT box contains `point` (per-item hit from layout).
    fn item_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.entries.len() {
            if let Some(r) = layout.box_of_part(root, &Self::item_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for Menu {
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
        match &event.kind {
            DomEventKind::MouseLeave => self.highlight(None),
            DomEventKind::MouseMove { x, y } => {
                let hit = self.item_at(root, Point::new(*x, *y), layout);
                self.highlight(hit)
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => match self.item_at(root, Point::new(*x, *y), layout) {
                Some(i) => self.activate(i),
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
        match key.key {
            keys::ARROW_DOWN => {
                let next = match self.highlighted {
                    Some(cur) => self.next_activatable(cur, true),
                    None => self.first_activatable(),
                };
                self.highlight(next)
            }
            keys::ARROW_UP => {
                let next = match self.highlighted {
                    Some(cur) => self.next_activatable(cur, false),
                    None => self.last_activatable(),
                };
                self.highlight(next)
            }
            keys::HOME => self.highlight(self.first_activatable()),
            keys::END => self.highlight(self.last_activatable()),
            keys::ENTER => match self.highlighted {
                Some(i) => self.activate(i),
                None => WidgetOutcome::Ignored,
            },
            keys::ARROW_RIGHT => match self.highlighted {
                Some(i) => match self.entries.get(i) {
                    Some(MenuEntry::Item {
                        id,
                        has_submenu: true,
                        disabled: false,
                        ..
                    }) => WidgetOutcome::action_with(SUBMENU_ACTION, id.clone()),
                    _ => WidgetOutcome::Ignored,
                },
                None => WidgetOutcome::Ignored,
            },
            keys::ESCAPE => WidgetOutcome::action(DISMISS_ACTION),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn render(&self) -> TemplateNode {
        let mut menu = TemplateNode::el("lq-menu")
            .attr("role", "menu")
            .attr(FOCUSABLE_ATTR, "true");

        for (i, entry) in self.entries.iter().enumerate() {
            match entry {
                MenuEntry::Separator => {
                    menu = menu.child(
                        TemplateNode::el("lq-menu-separator")
                            .key(&format!("sep-{i}"))
                            .attr("role", "separator"),
                    );
                }
                MenuEntry::Item {
                    id,
                    label,
                    disabled,
                    has_submenu,
                } => {
                    let hot = self.highlighted == Some(i) && !disabled;
                    let item = TemplateNode::el("lq-menu-item")
                        .key(id)
                        .attr("data-part", &Self::item_part(i))
                        .attr("data-id", id)
                        .attr("data-action", id) // mirror the chrome data-action seam
                        .attr("role", "menuitem")
                        .class_if("highlighted", hot)
                        .class_if("has-submenu", *has_submenu)
                        .pseudo_if(PseudoStateFlags::FOCUS, hot)
                        .pseudo_if(PseudoStateFlags::HOVER, hot)
                        .pseudo_if(PseudoStateFlags::DISABLED, *disabled)
                        .child(
                            TemplateNode::el("lq-menu-label")
                                .attr("data-part", "label")
                                .child(TemplateNode::text(label)),
                        );
                    let item = if *disabled {
                        item.attr("disabled", "true").attr("aria-disabled", "true")
                    } else {
                        item
                    };
                    menu = menu.child(item);
                }
            }
        }
        menu
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
