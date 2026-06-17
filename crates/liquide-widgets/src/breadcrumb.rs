//! `<lq-breadcrumb>` — a path of crumbs joined by separators (Group D: D3).
//!
//! Each crumb but the last is a clickable link; the last crumb is the current
//! location (non-clickable, `.current`/`aria-current`). Separators are CSS
//! `::after` glyphs on the crumb (no separate hit target). Behavior:
//!
//! - **Click** a non-last crumb's LAID-OUT box (`data-part="crumb-<i>"`) emits a
//!   `Navigate`(index) Action (hit per-crumb from layout, never a constant).
//!   Clicking the current (last) crumb is ignored.
//! - **Left/Right** move a keyboard cursor across the clickable crumbs; **Enter**
//!   navigates to the cursor crumb.
//! - Hovered crumb carries `:hover`; the keyboard-cursor crumb `:focus`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a crumb is activated (payload: the crumb index, as a string).
pub const NAVIGATE_ACTION: &str = "navigate";

/// A breadcrumb trail.
#[derive(Debug, Clone)]
pub struct Breadcrumb {
    /// Crumb labels, root first.
    crumbs: Vec<String>,
    /// Hovered crumb index, if any.
    hovered: Option<usize>,
    /// Keyboard cursor over clickable crumbs.
    cursor: usize,
}

impl Breadcrumb {
    /// A breadcrumb over the given path labels (root first, current last).
    pub fn new(crumbs: impl IntoIterator<Item = String>) -> Self {
        Self {
            crumbs: crumbs.into_iter().collect(),
            hovered: None,
            cursor: 0,
        }
    }

    /// Number of crumbs.
    pub fn len(&self) -> usize {
        self.crumbs.len()
    }

    /// Whether there are no crumbs.
    pub fn is_empty(&self) -> bool {
        self.crumbs.is_empty()
    }

    /// The index of the current (last) crumb, if any.
    pub fn current_index(&self) -> Option<usize> {
        self.crumbs.len().checked_sub(1)
    }

    /// The keyboard cursor crumb index.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn part_name(i: usize) -> String {
        format!("crumb-{i}")
    }

    /// Whether crumb `i` is a clickable link (every crumb except the last).
    fn is_link(&self, i: usize) -> bool {
        i + 1 < self.crumbs.len()
    }

    /// The index of the last clickable crumb (the one before current).
    fn last_link(&self) -> usize {
        self.crumbs.len().saturating_sub(2)
    }

    fn crumb_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.crumbs.len() {
            if let Some(r) = layout.box_of_part(root, &Self::part_name(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn navigate(&self, idx: usize) -> WidgetOutcome {
        if !self.is_link(idx) {
            return WidgetOutcome::Ignored;
        }
        WidgetOutcome::action_with(NAVIGATE_ACTION, idx.to_string())
    }
}

impl WidgetBehavior for Breadcrumb {
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
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                // Only clickable crumbs hover.
                let hit = self
                    .crumb_at(root, Point::new(*x, *y), layout)
                    .filter(|&i| self.is_link(i));
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
            } => match self.crumb_at(root, Point::new(*x, *y), layout) {
                Some(i) => self.navigate(i),
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
        if self.crumbs.len() < 2 {
            return WidgetOutcome::Ignored;
        }
        let last = self.last_link();
        match key.key {
            keys::ARROW_RIGHT => {
                let next = (self.cursor + 1).min(last);
                if next == self.cursor {
                    return WidgetOutcome::Ignored;
                }
                self.cursor = next;
                WidgetOutcome::Changed
            }
            keys::ARROW_LEFT => {
                let next = self.cursor.saturating_sub(1);
                if next == self.cursor {
                    return WidgetOutcome::Ignored;
                }
                self.cursor = next;
                WidgetOutcome::Changed
            }
            keys::HOME => {
                if self.cursor == 0 {
                    return WidgetOutcome::Ignored;
                }
                self.cursor = 0;
                WidgetOutcome::Changed
            }
            keys::END => {
                if self.cursor == last {
                    return WidgetOutcome::Ignored;
                }
                self.cursor = last;
                WidgetOutcome::Changed
            }
            keys::ENTER => self.navigate(self.cursor),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        self.crumbs.len() >= 2
    }

    fn render(&self) -> TemplateNode {
        let mut nav = TemplateNode::el("lq-breadcrumb")
            .attr(FOCUSABLE_ATTR, if self.crumbs.len() >= 2 { "true" } else { "false" })
            .attr("role", "navigation");

        let last = self.crumbs.len().saturating_sub(1);
        for (i, label) in self.crumbs.iter().enumerate() {
            let is_current = i == last;
            let is_link = self.is_link(i);
            let crumb = TemplateNode::el("lq-crumb")
                .key(&format!("crumb-{i}-{label}"))
                .attr("data-part", &Self::part_name(i))
                .attr("data-index", &format!("{i}"))
                // A non-last crumb separator (▸) is a CSS ::after; mark it so CSS
                // can target "not the last crumb".
                .class_if("link", is_link)
                .class_if("current", is_current)
                .attr("aria-current", if is_current { "page" } else { "false" })
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(i) && is_link,
                )
                .pseudo_if(
                    PseudoStateFlags::FOCUS,
                    is_link && self.cursor == i,
                )
                .child(TemplateNode::text(label));
            nav = nav.child(crumb);
        }
        nav
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
