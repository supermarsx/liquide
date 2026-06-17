//! `<lq-accordion>` — stacked collapsible sections (Group D: D7).
//!
//! State: N sections (title + a `expanded` flag) + a keyboard cursor + a mode
//! (single-open: opening one closes the rest; multi-open: each toggles
//! independently). Behavior:
//!
//! - **Click a header's LAID-OUT box** (`data-part="header-<i>"`) toggles that
//!   section. Hit per-header from layout (the panel below a header has variable
//!   height, so a constant header height would mis-target the next header once a
//!   panel expands — reading the laid-out box is the guard).
//! - **Up/Down** move the keyboard cursor across headers; **Enter/Space** toggle
//!   the cursor section; **Home/End** jump to first/last.
//! - Expanded sections carry `:expanded`(class) + the header `:checked`; the body
//!   is only emitted when expanded (so a collapsed section paints no panel box).
//! - Emits `Toggled`(index) when a section's expanded state flips.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a section is toggled (payload: the 0-based section index).
pub const TOGGLED_ACTION: &str = "toggled";

/// One accordion section.
#[derive(Debug, Clone)]
struct Section {
    title: String,
    body: String,
    expanded: bool,
}

/// A stacked collapsible-section accordion.
#[derive(Debug, Clone)]
pub struct Accordion {
    sections: Vec<Section>,
    /// Whether only one section may be open at a time.
    single_open: bool,
    /// Keyboard cursor over headers.
    cursor: usize,
    /// Hovered header index.
    hovered: Option<usize>,
}

impl Accordion {
    /// An accordion over `(title, body)` sections (multi-open by default).
    pub fn new(sections: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            sections: sections
                .into_iter()
                .map(|(title, body)| Section {
                    title,
                    body,
                    expanded: false,
                })
                .collect(),
            single_open: false,
            cursor: 0,
            hovered: None,
        }
    }

    /// Only one section open at a time (opening one closes the others).
    pub fn single_open(mut self, s: bool) -> Self {
        self.single_open = s;
        self
    }

    /// Start with section `idx` expanded (respects single-open).
    pub fn expand(mut self, idx: usize) -> Self {
        if idx < self.sections.len() {
            if self.single_open {
                for (i, s) in self.sections.iter_mut().enumerate() {
                    s.expanded = i == idx;
                }
            } else {
                self.sections[idx].expanded = true;
            }
        }
        self
    }

    /// Number of sections.
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    /// Whether there are no sections.
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// Whether section `idx` is expanded.
    pub fn is_expanded(&self, idx: usize) -> bool {
        self.sections.get(idx).map(|s| s.expanded).unwrap_or(false)
    }

    /// The indices of all expanded sections.
    pub fn expanded_indices(&self) -> Vec<usize> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| s.expanded)
            .map(|(i, _)| i)
            .collect()
    }

    /// The keyboard cursor.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn header_part(i: usize) -> String {
        format!("header-{i}")
    }

    fn toggle(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.sections.len() {
            return WidgetOutcome::Ignored;
        }
        let now = !self.sections[idx].expanded;
        if self.single_open && now {
            // Opening one closes the rest.
            for (i, s) in self.sections.iter_mut().enumerate() {
                s.expanded = i == idx;
            }
        } else {
            self.sections[idx].expanded = now;
        }
        self.cursor = idx;
        WidgetOutcome::action_with(TOGGLED_ACTION, idx.to_string())
    }

    fn header_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.sections.len() {
            if let Some(r) = layout.box_of_part(root, &Self::header_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for Accordion {
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
                let hit = self.header_at(root, Point::new(*x, *y), layout);
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
            } => match self.header_at(root, Point::new(*x, *y), layout) {
                Some(i) => self.toggle(i),
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
        if self.sections.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let n = self.sections.len();
        match key.key {
            keys::ARROW_DOWN => {
                let next = (self.cursor + 1).min(n - 1);
                if next == self.cursor {
                    return WidgetOutcome::Ignored;
                }
                self.cursor = next;
                WidgetOutcome::Changed
            }
            keys::ARROW_UP => {
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
                if self.cursor == n - 1 {
                    return WidgetOutcome::Ignored;
                }
                self.cursor = n - 1;
                WidgetOutcome::Changed
            }
            keys::ENTER | keys::SPACE => self.toggle(self.cursor),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.sections.is_empty()
    }

    fn render(&self) -> TemplateNode {
        let mut acc = TemplateNode::el("lq-accordion").attr(FOCUSABLE_ATTR, "true");

        for (i, sec) in self.sections.iter().enumerate() {
            let mut section = TemplateNode::el("lq-section")
                .key(&format!("section-{i}"))
                .class_if("expanded", sec.expanded);

            let header = TemplateNode::el("lq-section-header")
                .attr("data-part", &Self::header_part(i))
                .attr("data-index", &format!("{i}"))
                .attr("role", "button")
                .attr("aria-expanded", if sec.expanded { "true" } else { "false" })
                .class_if("expanded", sec.expanded)
                .pseudo_if(PseudoStateFlags::CHECKED, sec.expanded)
                .pseudo_if(PseudoStateFlags::FOCUS, self.cursor == i)
                .pseudo_if(PseudoStateFlags::HOVER, self.hovered == Some(i))
                .child(
                    TemplateNode::el("lq-section-twisty").attr("data-part", "twisty"),
                )
                .child(
                    TemplateNode::el("lq-section-title")
                        .attr("data-part", "title")
                        .child(TemplateNode::text(&sec.title)),
                );
            section = section.child(header);

            // The body panel is only emitted when expanded (collapsed section
            // paints no panel box).
            if sec.expanded {
                section = section.child(
                    TemplateNode::el("lq-section-body")
                        .attr("data-part", "body")
                        .attr("role", "region")
                        .child(TemplateNode::text(&sec.body)),
                );
            }
            acc = acc.child(section);
        }
        acc
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
