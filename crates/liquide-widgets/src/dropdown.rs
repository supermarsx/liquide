//! `<lq-dropdown>` / `<lq-combobox>` — a closed button that opens a popup option
//! list (Group D: D1).
//!
//! The dropdown is a single widget subtree: a `data-part="button"` trigger
//! showing the current selection, and — when open — a `data-part="popup"`
//! option list whose rows are `data-part="option-<i>"`. The option rows reuse
//! the menu/list popup pattern (per-row hit-test from the LAID-OUT box, never
//! `index * row_height`). Behavior:
//!
//! - **Click the button**: toggles the popup open/closed.
//! - **Click an option** (popup open): selects it + closes; emits `Changed`(value).
//! - **Click elsewhere while open** (a click that hits neither button nor any
//!   option box): closes without changing the selection.
//! - **Keyboard** (focused): Down/Up move the highlight (opening the popup if
//!   closed), Enter selects the highlight (or opens if closed), Esc closes.
//! - A **combobox** ([`Dropdown::combobox`]) adds a text-filter input
//!   (`data-part="filter"`): typed characters narrow the visible options by a
//!   case-insensitive substring match; the option indices stay STABLE (each
//!   visible row keeps its real option index in `data-index` + its
//!   `data-part="option-<i>"`), so selection always maps back to the true value.
//!
//! `:open` is expressed as the `.open` class on the root + the popup's presence
//! (the popup element is only emitted when open, so a closed dropdown paints no
//! option boxes — a constant-driven hit test could not know that). The
//! highlighted option carries `:focus`/`.highlighted`; the hovered one `:hover`;
//! the selected one `:checked`/`.selected`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the selection changes (payload: the selected option value).
pub const CHANGED_ACTION: &str = "changed";

/// A select / combobox widget.
#[derive(Debug, Clone)]
pub struct Dropdown {
    /// (value, label) options in order.
    options: Vec<(String, String)>,
    /// The selected option index, if any.
    selected: Option<usize>,
    /// Whether the popup is open.
    open: bool,
    /// The highlighted option index (keyboard cursor / hover) while open.
    highlighted: Option<usize>,
    /// The hovered option index (mouse).
    hovered: Option<usize>,
    /// Whether this is a combobox (has a filter input).
    combobox: bool,
    /// The current filter text (combobox only).
    filter: String,
    /// Placeholder shown when nothing is selected.
    placeholder: String,
    disabled: bool,
}

impl Dropdown {
    /// A plain select over `(value, label)` options.
    pub fn new(options: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            options: options.into_iter().collect(),
            selected: None,
            open: false,
            highlighted: None,
            hovered: None,
            combobox: false,
            filter: String::new(),
            placeholder: "Select…".to_string(),
            disabled: false,
        }
    }

    /// A combobox (select + text filter).
    pub fn combobox(options: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut d = Self::new(options);
        d.combobox = true;
        d
    }

    /// Set the placeholder shown when nothing is selected.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    /// Pre-select an option by index.
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.options.len() {
            self.selected = Some(idx);
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Whether the popup is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The selected option index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// The selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected
            .and_then(|i| self.options.get(i))
            .map(|(v, _)| v.as_str())
    }

    /// The highlighted option index (while open).
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// The current filter text (combobox).
    pub fn filter_text(&self) -> &str {
        &self.filter
    }

    fn option_part(i: usize) -> String {
        format!("option-{i}")
    }

    /// Whether option `i` is currently visible under the filter.
    fn visible(&self, i: usize) -> bool {
        if !self.combobox || self.filter.is_empty() {
            return true;
        }
        let needle = self.filter.to_lowercase();
        self.options
            .get(i)
            .map(|(_, label)| label.to_lowercase().contains(&needle))
            .unwrap_or(false)
    }

    /// The visible option indices, in order.
    fn visible_indices(&self) -> Vec<usize> {
        (0..self.options.len()).filter(|&i| self.visible(i)).collect()
    }

    fn open_popup(&mut self) -> WidgetOutcome {
        if self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = true;
        // Highlight the selected option, else the first visible one.
        self.highlighted = self
            .selected
            .filter(|&i| self.visible(i))
            .or_else(|| self.visible_indices().first().copied());
        WidgetOutcome::Changed
    }

    fn close_popup(&mut self) -> WidgetOutcome {
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = false;
        self.highlighted = None;
        self.hovered = None;
        WidgetOutcome::Changed
    }

    /// Commit a selection by option index and close.
    fn choose(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.options.len() {
            return WidgetOutcome::Ignored;
        }
        let changed = self.selected != Some(idx);
        self.selected = Some(idx);
        self.open = false;
        self.highlighted = None;
        self.hovered = None;
        // Clear the filter so the closed combobox shows the selection, not the
        // search text.
        self.filter.clear();
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, self.options[idx].0.clone())
        } else {
            WidgetOutcome::Changed
        }
    }

    /// Which option's LAID-OUT box contains the point (visible options only).
    fn option_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in self.visible_indices() {
            if let Some(r) = layout.box_of_part(root, &Self::option_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn button_contains(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, "button")
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }

    /// Move the highlight to the next/prev visible option.
    fn move_highlight(&mut self, forward: bool) -> WidgetOutcome {
        let vis = self.visible_indices();
        if vis.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let cur_pos = self
            .highlighted
            .and_then(|h| vis.iter().position(|&i| i == h));
        let next_pos = match cur_pos {
            Some(p) if forward => (p + 1).min(vis.len() - 1),
            Some(p) => p.saturating_sub(1),
            None if forward => 0,
            None => vis.len() - 1,
        };
        self.highlighted = Some(vis[next_pos]);
        WidgetOutcome::Changed
    }

    /// Apply a printable character or Backspace to the filter (combobox).
    fn edit_filter(&mut self, key: u32) -> WidgetOutcome {
        if let Some(c) = keys::printable_char(key) {
            self.filter.push(c);
        } else if key == keys::BACKSPACE {
            if self.filter.pop().is_none() {
                return WidgetOutcome::Ignored;
            }
        } else {
            return WidgetOutcome::Ignored;
        }
        // Re-anchor the highlight to the first still-visible option.
        if self.highlighted.map(|h| !self.visible(h)).unwrap_or(true) {
            self.highlighted = self.visible_indices().first().copied();
        }
        WidgetOutcome::Changed
    }
}

impl WidgetBehavior for Dropdown {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
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
                if !self.open {
                    return WidgetOutcome::Ignored;
                }
                let hit = self.option_at(root, Point::new(*x, *y), layout);
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
            } => {
                let p = Point::new(*x, *y);
                if self.open {
                    // Click an option -> choose; click the button -> close;
                    // click anywhere else -> close (dismiss).
                    if let Some(i) = self.option_at(root, p, layout) {
                        self.choose(i)
                    } else {
                        self.close_popup()
                    }
                } else if self.button_contains(root, p, layout) {
                    self.open_popup()
                } else {
                    WidgetOutcome::Ignored
                }
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        match key.key {
            keys::ARROW_DOWN => {
                if !self.open {
                    self.open_popup()
                } else {
                    self.move_highlight(true)
                }
            }
            keys::ARROW_UP => {
                if !self.open {
                    self.open_popup()
                } else {
                    self.move_highlight(false)
                }
            }
            keys::ENTER => {
                if !self.open {
                    self.open_popup()
                } else if let Some(i) = self.highlighted {
                    self.choose(i)
                } else {
                    WidgetOutcome::Ignored
                }
            }
            keys::ESCAPE => self.close_popup(),
            k if self.combobox && self.open => self.edit_filter(k),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let tag = if self.combobox {
            "lq-combobox"
        } else {
            "lq-dropdown"
        };
        let mut root = TemplateNode::el(tag)
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "combobox")
            .attr("aria-expanded", if self.open { "true" } else { "false" })
            .class_if("open", self.open)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.open)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        // The closed-state trigger button shows the current selection (or, for an
        // open combobox, the live filter text), else the placeholder.
        let button_text = if self.combobox && self.open && !self.filter.is_empty() {
            self.filter.clone()
        } else {
            match self.selected_value().and_then(|v| {
                self.options
                    .iter()
                    .find(|(val, _)| val == v)
                    .map(|(_, l)| l.clone())
            }) {
                Some(label) => label,
                None => self.placeholder.clone(),
            }
        };

        let button = TemplateNode::el("lq-dropdown-button")
            .attr("data-part", "button")
            .class_if("placeholder", self.selected.is_none() && self.filter.is_empty())
            .child(
                TemplateNode::el("lq-dropdown-value")
                    .attr("data-part", "value")
                    .child(TemplateNode::text(&button_text)),
            )
            .child(TemplateNode::el("lq-dropdown-arrow").attr("data-part", "arrow"));
        root = root.child(button);

        if self.open {
            let mut popup = TemplateNode::el("lq-popup")
                .attr("data-part", "popup")
                .attr("role", "listbox");

            if self.combobox {
                popup = popup.child(
                    TemplateNode::el("lq-dropdown-filter")
                        .attr("data-part", "filter")
                        .child(TemplateNode::text(&self.filter)),
                );
            }

            for i in self.visible_indices() {
                let (value, label) = &self.options[i];
                let sel = self.selected == Some(i);
                let hot = self.highlighted == Some(i);
                let hov = self.hovered == Some(i);
                let opt = TemplateNode::el("lq-option")
                    .key(value)
                    .attr("data-part", &Self::option_part(i))
                    .attr("data-index", &format!("{i}"))
                    .attr("data-value", value)
                    .attr("role", "option")
                    .attr("aria-selected", if sel { "true" } else { "false" })
                    .class_if("selected", sel)
                    .class_if("highlighted", hot)
                    .pseudo_if(PseudoStateFlags::CHECKED, sel)
                    .pseudo_if(PseudoStateFlags::FOCUS, hot)
                    .pseudo_if(PseudoStateFlags::HOVER, hov)
                    .child(TemplateNode::text(label));
                popup = popup.child(opt);
            }
            root = root.child(popup);
        }

        if self.disabled {
            root = root.attr("disabled", "true");
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
