//! `<lq-command-palette>` — a fuzzy-finder overlay (NAV/OVERLAY family).
//!
//! The palette is a single subtree: a `data-part="search"` text input showing the
//! live query, and a `data-part="results"` list whose visible rows are
//! `data-part="item-<i>"` (where `<i>` is the command's STABLE original index, so
//! a click always maps back to the true command even when the list is filtered
//! and re-ranked). Behavior:
//!
//! - **Type** (printable / Backspace): filters + ranks the command list. Matching
//!   is a case-insensitive **subsequence** test (every query char appears in the
//!   command title in order), and survivors are ranked by a fuzzy score
//!   (contiguous runs + a word-boundary/prefix bonus), best first.
//! - **Up/Down**: move the highlighted row across the *visible* (ranked) results.
//! - **Enter**: activate the highlighted command → `Action`(command id).
//! - **Esc**: close the palette (clears the query + highlight).
//! - **Click an item**: activate it from its LAID-OUT box (never `i * row_h`).
//!
//! `:open` is the `.open` class on the root + the presence of the results list
//! (a closed palette emits no item boxes — a constant-driven hit test could not
//! know that). The highlighted row carries `:focus`/`.highlighted`; the hovered
//! one `:hover`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::fuzzy;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a command is activated (payload: the command id).
pub const ACTION_NAME: &str = "activate";

/// One command in the palette.
#[derive(Debug, Clone)]
pub struct Command {
    /// Stable id emitted in the action payload.
    pub id: String,
    /// Display title (the text matched against the query).
    pub title: String,
}

impl Command {
    /// A command with an id and a title.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }
}

/// A fuzzy-finder command palette overlay.
#[derive(Debug, Clone)]
pub struct CommandPalette {
    /// All commands, in their original (stable) order.
    commands: Vec<Command>,
    /// The live search query.
    query: String,
    /// Whether the palette is open.
    open: bool,
    /// The highlighted position WITHIN the current visible/ranked list.
    highlighted: usize,
    /// The hovered command's STABLE original index, if any.
    hovered: Option<usize>,
    /// Placeholder shown in the empty search field.
    placeholder: String,
}

impl CommandPalette {
    /// Build a palette over `commands` (closed by default).
    pub fn new(commands: impl IntoIterator<Item = Command>) -> Self {
        Self {
            commands: commands.into_iter().collect(),
            query: String::new(),
            open: false,
            highlighted: 0,
            hovered: None,
            placeholder: "Type a command…".to_string(),
        }
    }

    /// Open the palette initially.
    pub fn open(mut self, o: bool) -> Self {
        self.open = o;
        self
    }

    /// Set the empty-field placeholder.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    /// Whether the palette is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The current query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The highlighted position within the visible/ranked list.
    pub fn highlighted(&self) -> usize {
        self.highlighted
    }

    /// The STABLE original indices of the currently-visible commands, ranked
    /// best-first by the fuzzy score. With an empty query every command is
    /// visible in its original order.
    pub fn visible_indices(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.commands.len()).collect();
        }
        let mut scored: Vec<(usize, i32)> = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(i, c)| fuzzy::score(&self.query, &c.title).map(|s| (i, s)))
            .collect();
        // Higher score first; ties keep original order (stable sort on -score).
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.into_iter().map(|(i, _)| i).collect()
    }

    /// The command id at the highlighted visible position, if any.
    pub fn highlighted_id(&self) -> Option<&str> {
        let vis = self.visible_indices();
        vis.get(self.highlighted)
            .and_then(|&i| self.commands.get(i))
            .map(|c| c.id.as_str())
    }

    fn item_part(i: usize) -> String {
        format!("item-{i}")
    }

    /// Which command (stable index) sits under `point`, from its laid-out box.
    fn item_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for &i in &self.visible_indices() {
            if let Some(r) = layout.box_of_part(root, &Self::item_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Activate the command at the given STABLE index.
    fn activate(&mut self, idx: usize) -> WidgetOutcome {
        match self.commands.get(idx) {
            Some(c) => WidgetOutcome::action_with(ACTION_NAME, c.id.clone()),
            None => WidgetOutcome::Ignored,
        }
    }

    fn close(&mut self) -> WidgetOutcome {
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = false;
        self.query.clear();
        self.highlighted = 0;
        self.hovered = None;
        WidgetOutcome::Changed
    }

    fn edit_query(&mut self, key: u32) -> WidgetOutcome {
        if let Some(c) = keys::printable_char(key) {
            self.query.push(c);
        } else if key == keys::BACKSPACE {
            if self.query.pop().is_none() {
                return WidgetOutcome::Ignored;
            }
        } else {
            return WidgetOutcome::Ignored;
        }
        // The result set just changed; re-anchor the highlight to the top.
        self.highlighted = 0;
        WidgetOutcome::Changed
    }
}

impl WidgetBehavior for CommandPalette {
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
        if !self.open {
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
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        match key.key {
            keys::ESCAPE => self.close(),
            keys::ARROW_DOWN => {
                let n = self.visible_indices().len();
                if n == 0 {
                    return WidgetOutcome::Ignored;
                }
                self.highlighted = (self.highlighted + 1).min(n - 1);
                WidgetOutcome::Changed
            }
            keys::ARROW_UP => {
                self.highlighted = self.highlighted.saturating_sub(1);
                WidgetOutcome::Changed
            }
            keys::ENTER => {
                let vis = self.visible_indices();
                match vis.get(self.highlighted) {
                    Some(&i) => self.activate(i),
                    None => WidgetOutcome::Ignored,
                }
            }
            k => self.edit_query(k),
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn render(&self) -> TemplateNode {
        // Recompute the ranking once, and clamp the highlight against it so the
        // rendered :focus row is always a real, visible row.
        let vis = self.visible_indices();
        let hi = if vis.is_empty() {
            usize::MAX
        } else {
            self.highlighted.min(vis.len() - 1)
        };

        let mut root = TemplateNode::el("lq-command-palette")
            .attr(FOCUSABLE_ATTR, "true")
            .attr("role", "dialog")
            .attr("aria-expanded", if self.open { "true" } else { "false" })
            .class_if("open", self.open)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.open);

        if !self.open {
            return root;
        }

        // Search field (shows the live query, or the placeholder when empty).
        let search = TemplateNode::el("lq-palette-search")
            .attr("data-part", "search")
            .attr("role", "searchbox")
            .class_if("placeholder", self.query.is_empty())
            .child(TemplateNode::text(if self.query.is_empty() {
                &self.placeholder
            } else {
                &self.query
            }));
        root = root.child(search);

        let mut results = TemplateNode::el("lq-palette-results")
            .attr("data-part", "results")
            .attr("role", "listbox");

        if vis.is_empty() {
            results = results.child(
                TemplateNode::el("lq-palette-empty")
                    .attr("data-part", "empty")
                    .child(TemplateNode::text("No matching commands")),
            );
        } else {
            for (pos, &i) in vis.iter().enumerate() {
                let cmd = &self.commands[i];
                let hot = pos == hi;
                let hov = self.hovered == Some(i);
                let item = TemplateNode::el("lq-palette-item")
                    .key(&cmd.id)
                    .attr("data-part", &Self::item_part(i))
                    .attr("data-index", &format!("{i}"))
                    .attr("data-id", &cmd.id)
                    .attr("role", "option")
                    .attr("aria-selected", if hot { "true" } else { "false" })
                    .class_if("highlighted", hot)
                    .pseudo_if(PseudoStateFlags::FOCUS, hot)
                    .pseudo_if(PseudoStateFlags::HOVER, hov)
                    .child(TemplateNode::text(&cmd.title));
                results = results.child(item);
            }
        }
        root.child(results)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
