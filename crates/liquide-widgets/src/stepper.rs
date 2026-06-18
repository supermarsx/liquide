//! `<lq-stepper>` — an ordered wizard with a progress indicator (NAV/OVERLAY).
//!
//! The stepper renders a header strip of `data-part="step-<i>"` markers (one per
//! step, each a numbered/labelled node) plus a `data-part="back"` and
//! `data-part="next"` control. Each step is `.completed` / `.current` /
//! `.upcoming`, and the marker for a *reachable* step is clickable.
//!
//! Reachability model (the classic wizard rule): you may jump **back** to any
//! completed step or the current one, and **forward** only to the immediately
//! next step (you can't skip ahead past unvisited steps). `Next` advances one
//! step (marking the left one completed); `Back` retreats one. Every transition
//! emits `Action`(`changed`) with the new step index as payload.
//!
//! Hit-testing reads each marker/control box from layout, never `i * marker_w`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the active step changes (payload: the new step index).
pub const CHANGED_ACTION: &str = "changed";

/// A wizard / stepper control.
#[derive(Debug, Clone)]
pub struct Stepper {
    /// Step titles, in order.
    steps: Vec<String>,
    /// The current (active) step index.
    current: usize,
    disabled: bool,
}

impl Stepper {
    /// Build a stepper over `steps` (starting at step 0).
    pub fn new(steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            steps: steps.into_iter().map(Into::into).collect(),
            current: 0,
            disabled: false,
        }
    }

    /// Start at a given step (and mark everything up to it reachable).
    pub fn start_at(mut self, idx: usize) -> Self {
        if idx < self.steps.len() {
            self.current = idx;
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The current step index.
    pub fn current(&self) -> usize {
        self.current
    }

    /// The number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether there are no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Whether step `i` may be navigated to right now: any step at or before the
    /// current one (you can always step back to a visited step), or the single
    /// immediately-next step (you can advance one, but not skip ahead).
    pub fn is_reachable(&self, i: usize) -> bool {
        i < self.steps.len() && i <= self.current + 1
    }

    fn step_part(i: usize) -> String {
        format!("step-{i}")
    }

    fn goto(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.steps.len() || idx == self.current || !self.is_reachable(idx) {
            return WidgetOutcome::Ignored;
        }
        self.current = idx;
        WidgetOutcome::action_with(CHANGED_ACTION, idx.to_string())
    }

    fn next(&mut self) -> WidgetOutcome {
        if self.current + 1 >= self.steps.len() {
            return WidgetOutcome::Ignored;
        }
        self.goto(self.current + 1)
    }

    fn back(&mut self) -> WidgetOutcome {
        if self.current == 0 {
            return WidgetOutcome::Ignored;
        }
        self.goto(self.current - 1)
    }

    /// Which reachable step marker sits under `point`, from its laid-out box.
    fn step_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.steps.len() {
            if let Some(r) = layout.box_of_part(root, &Self::step_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn control_hit(&self, root: NodeId, part: &str, point: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, part)
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }
}

impl WidgetBehavior for Stepper {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        }]
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
        if let DomEventKind::Click {
            button: MouseButton::Left,
            x,
            y,
        } = &event.kind
        {
            let p = Point::new(*x, *y);
            if self.control_hit(root, "next", p, layout) {
                return self.next();
            }
            if self.control_hit(root, "back", p, layout) {
                return self.back();
            }
            if let Some(i) = self.step_at(root, p, layout) {
                return self.goto(i);
            }
        }
        WidgetOutcome::Ignored
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
            keys::ARROW_RIGHT => self.next(),
            keys::ARROW_LEFT => self.back(),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut root = TemplateNode::el("lq-stepper")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "group")
            .attr("aria-valuenow", &self.current.to_string())
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        let mut header = TemplateNode::el("lq-stepper-head").attr("data-part", "head");
        for (i, title) in self.steps.iter().enumerate() {
            let completed = i < self.current;
            let current = i == self.current;
            let reachable = self.is_reachable(i) && !self.disabled;
            let marker = TemplateNode::el("lq-step")
                .key(&format!("step-{i}"))
                .attr("data-part", &Self::step_part(i))
                .attr("data-index", &format!("{i}"))
                .attr("role", "tab")
                .attr("aria-selected", if current { "true" } else { "false" })
                .class_if("completed", completed)
                .class_if("current", current)
                .class_if("upcoming", !completed && !current)
                .class_if("reachable", reachable)
                .pseudo_if(PseudoStateFlags::CHECKED, completed)
                .pseudo_if(PseudoStateFlags::FOCUS, current)
                .pseudo_if(PseudoStateFlags::DISABLED, !reachable)
                .child(
                    TemplateNode::el("lq-step-marker")
                        .attr("data-part", "marker")
                        .child(TemplateNode::text(&(i + 1).to_string())),
                )
                .child(
                    TemplateNode::el("lq-step-label")
                        .attr("data-part", "label")
                        .child(TemplateNode::text(title)),
                );
            header = header.child(marker);
            // A connector between steps (last step omits it).
            if i + 1 < self.steps.len() {
                header = header.child(
                    TemplateNode::el("lq-step-connector")
                        .key(&format!("conn-{i}"))
                        .class_if("filled", i < self.current),
                );
            }
        }
        root = root.child(header);

        // Footer controls.
        let footer = TemplateNode::el("lq-stepper-foot")
            .attr("data-part", "foot")
            .child(
                TemplateNode::el("lq-step-back")
                    .attr("data-part", "back")
                    .attr("role", "button")
                    .class_if("disabled", self.current == 0)
                    .pseudo_if(PseudoStateFlags::DISABLED, self.current == 0)
                    .child(TemplateNode::text("Back")),
            )
            .child(
                TemplateNode::el("lq-step-next")
                    .attr("data-part", "next")
                    .attr("role", "button")
                    .class_if("disabled", self.current + 1 >= self.steps.len())
                    .pseudo_if(
                        PseudoStateFlags::DISABLED,
                        self.current + 1 >= self.steps.len(),
                    )
                    .child(TemplateNode::text("Next")),
            );
        root = root.child(footer);

        if self.disabled {
            root = root.attr("disabled", "true");
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
