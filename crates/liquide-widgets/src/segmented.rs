//! `<lq-segmented>` — a horizontal exclusive segment group (Group D: D2).
//!
//! Like a radio group but button-styled: a row of segments where exactly one is
//! selected at a time, enforced in the behavior (single `selected` field — the
//! same single-source-of-truth construction as [`RadioGroup`](crate::toggle::RadioGroup)).
//! Behavior:
//!
//! - **Click** a segment's LAID-OUT box (`data-part="seg-<i>"`) selects it (hit
//!   per-segment from layout, never `index * seg_width`).
//! - **Left/Right** (and Up/Down) move the selection across segments, wrapping,
//!   when focused.
//! - The selected segment carries `:checked`/`.selected`; hover `:hover`.
//! - Emits `Changed`(value) when the selection changes.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the selected segment changes (payload: the segment value).
pub const CHANGED_ACTION: &str = "changed";

/// A horizontal exclusive segment group.
#[derive(Debug, Clone)]
pub struct Segmented {
    /// (value, label) segments in order.
    segments: Vec<(String, String)>,
    /// The selected segment index.
    selected: usize,
    /// The hovered segment index, if any.
    hovered: Option<usize>,
    disabled: bool,
}

impl Segmented {
    /// A segment group over `(value, label)` pairs; the first is selected.
    pub fn new(segments: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            segments: segments.into_iter().collect(),
            selected: 0,
            hovered: None,
            disabled: false,
        }
    }

    /// Pre-select a segment by index.
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.segments.len() {
            self.selected = idx;
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The selected segment index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The selected segment value.
    pub fn selected_value(&self) -> Option<&str> {
        self.segments.get(self.selected).map(|(v, _)| v.as_str())
    }

    /// Number of segments.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Whether there are no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    fn part_name(i: usize) -> String {
        format!("seg-{i}")
    }

    fn set_selected(&mut self, idx: usize) -> WidgetOutcome {
        if self.disabled || idx >= self.segments.len() || idx == self.selected {
            return WidgetOutcome::Ignored;
        }
        self.selected = idx;
        WidgetOutcome::action_with(CHANGED_ACTION, self.segments[idx].0.clone())
    }

    fn seg_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.segments.len() {
            if let Some(r) = layout.box_of_part(root, &Self::part_name(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for Segmented {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Toggle
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
                let hit = self.seg_at(root, Point::new(*x, *y), layout);
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
            } => match self.seg_at(root, Point::new(*x, *y), layout) {
                Some(i) => self.set_selected(i),
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
        if self.disabled || self.segments.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let n = self.segments.len();
        let next = match key.key {
            keys::ARROW_RIGHT | keys::ARROW_DOWN => (self.selected + 1) % n,
            keys::ARROW_LEFT | keys::ARROW_UP => (self.selected + n - 1) % n,
            keys::HOME => 0,
            keys::END => n - 1,
            _ => return WidgetOutcome::Ignored,
        };
        self.set_selected(next)
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut group = TemplateNode::el("lq-segmented")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "radiogroup")
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for (i, (value, label)) in self.segments.iter().enumerate() {
            let sel = i == self.selected;
            let seg = TemplateNode::el("lq-segment")
                .key(value)
                .attr("data-part", &Self::part_name(i))
                .attr("data-value", value)
                .attr("role", "radio")
                .attr("aria-checked", if sel { "true" } else { "false" })
                .class_if("selected", sel)
                .pseudo_if(PseudoStateFlags::CHECKED, sel)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(i) && !self.disabled,
                )
                .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
                .child(TemplateNode::text(label));
            group = group.child(seg);
        }
        if self.disabled {
            group = group.attr("disabled", "true");
        }
        group
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
