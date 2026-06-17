//! `ReferenceBox` — the S0 infrastructure-validation widget (NOT a Group-A widget).
//!
//! This minimal `<lq-box>` exists only to prove the shared infrastructure works
//! end-to-end through the REAL pipeline: it is rendered as a styled custom
//! element, a click on its laid-out CSS box fires a [`WidgetOutcome::Action`],
//! and pointer interaction flips its interactive pseudo-states (`:hover`,
//! `:active`) so CSS restyles it. The gallery harness drives all of this through
//! style -> layout -> paint, asserting against the rasterized pixels and the
//! emitted actions — no fake-green.
//!
//! It deliberately does the WRONG thing if you try to hardcode geometry: its
//! click handling derives the press location's relevance from the laid-out box
//! via [`LayoutQuery`], not a constant, and records the box width it saw so a
//! test can assert the geometry came from layout (and would fail if a constant
//! were substituted).

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::layout_query::LayoutQuery;

/// A reference widget that emits an action on click and tracks pointer state.
#[derive(Debug, Default)]
pub struct ReferenceBox {
    /// The action name emitted on click.
    action: String,
    /// Whether the pointer is currently pressing the box (`:active`).
    pressed: bool,
    /// Whether the pointer is hovering the box (`:hover`).
    hovered: bool,
    /// Number of completed clicks (proof the action fired).
    clicks: u32,
    /// The width (px) of the box as read from the LAID-OUT layout box on the
    /// last click — `None` until the first geometry-derived click. A test asserts
    /// this matches the CSS-driven width, which a hardcoded-constant
    /// implementation could not produce.
    last_seen_box_width: Option<f32>,
}

impl ReferenceBox {
    /// Create a reference box that emits `action` on click.
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            ..Default::default()
        }
    }

    /// Completed click count.
    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    /// Whether the box is in its `:active` (pressed) state.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Whether the box is in its `:hover` state.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// The laid-out box width seen at the last click (geometry-from-layout proof).
    pub fn last_seen_box_width(&self) -> Option<f32> {
        self.last_seen_box_width
    }
}

impl WidgetBehavior for ReferenceBox {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Reference
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseEnter,
            DomEventKind::MouseLeave,
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
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
            DomEventKind::MouseEnter => {
                if self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = true;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseLeave => {
                if !self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = false;
                self.pressed = false;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = true;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if !self.pressed {
                    return WidgetOutcome::Ignored;
                }
                self.pressed = false;
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                // Read the hit geometry from the LAID-OUT box of THIS WIDGET's
                // root (never a constant; never the hit leaf which may be a text
                // sub-node). We only count the click when the click point falls
                // inside the widget's layout box — proving geometry is
                // layout-derived.
                let inside = layout
                    .box_of(root)
                    .map(|r| {
                        self.last_seen_box_width = Some(r.width);
                        r.contains(liquide_layout::geometry::Point::new(*x, *y))
                    })
                    .unwrap_or(false);
                if !inside {
                    return WidgetOutcome::Ignored;
                }
                self.clicks += 1;
                WidgetOutcome::action(self.action.clone())
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        _key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        WidgetOutcome::Ignored
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn render(&self) -> TemplateNode {
        TemplateNode::el("lq-box")
            .attr("data-action", &self.action)
            .attr(FOCUSABLE_ATTR, "true")
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.pressed)
            .child(TemplateNode::text("reference"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_carries_action_and_pseudo_states() {
        let mut b = ReferenceBox::new("ping");
        let node = b.render();
        assert_eq!(node.tag, "lq-box");
        assert!(node
            .attrs
            .iter()
            .any(|(k, v)| k == "data-action" && v == "ping"));
        assert!(!node.pseudo_states.contains(PseudoStateFlags::HOVER));

        b.hovered = true;
        b.pressed = true;
        let node = b.render();
        assert!(node.pseudo_states.contains(PseudoStateFlags::HOVER));
        assert!(node.pseudo_states.contains(PseudoStateFlags::ACTIVE));
    }
}
