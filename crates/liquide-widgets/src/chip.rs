//! `<lq-chip>` — a compact tag, optionally removable and/or selectable (Group D: D5).
//!
//! State: a label, optional selected flag, optional removable flag. Behavior:
//!
//! - **Click the remove (×) box** (`data-part="remove"`, removable chips only):
//!   emits a `Remove` Action. Hit-tested against the LAID-OUT × box, never a
//!   constant offset — clicking the body next to the × must NOT remove.
//! - **Click the chip body** (selectable chips): toggles `:selected`/`.selected`
//!   and emits a `Changed`(true/false) Action.
//! - `:hover` on the whole chip; the × box has its own `:hover` via CSS.
//! - A plain (non-selectable, non-removable) chip is an inert display tag.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a removable chip's × is clicked (no payload).
pub const REMOVE_ACTION: &str = "remove";
/// Emitted when a selectable chip's selected state toggles (payload: true/false).
pub const CHANGED_ACTION: &str = "changed";

/// A compact tag chip.
#[derive(Debug, Clone)]
pub struct Chip {
    label: String,
    selectable: bool,
    selected: bool,
    removable: bool,
    hovered: bool,
    disabled: bool,
}

impl Chip {
    /// A plain display chip.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            selectable: false,
            selected: false,
            removable: false,
            hovered: false,
            disabled: false,
        }
    }

    /// Make the chip removable (renders a × remove affordance).
    pub fn removable(mut self, r: bool) -> Self {
        self.removable = r;
        self
    }

    /// Make the chip selectable (clicking the body toggles selection).
    pub fn selectable(mut self, s: bool) -> Self {
        self.selectable = s;
        self
    }

    /// Start selected (implies selectable).
    pub fn selected(mut self, s: bool) -> Self {
        self.selected = s;
        if s {
            self.selectable = true;
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Whether currently selected.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Whether removable.
    pub fn is_removable(&self) -> bool {
        self.removable
    }

    fn toggle_selected(&mut self) -> WidgetOutcome {
        if !self.selectable {
            return WidgetOutcome::Ignored;
        }
        self.selected = !self.selected;
        WidgetOutcome::action_with(CHANGED_ACTION, if self.selected { "true" } else { "false" })
    }
}

impl WidgetBehavior for Chip {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseEnter,
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
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                // The × box wins if the click lands inside its LAID-OUT box.
                if self.removable {
                    if let Some(r) = layout.box_of_part(root, "remove") {
                        if r.contains(p) {
                            return WidgetOutcome::action(REMOVE_ACTION);
                        }
                    }
                }
                // Otherwise a click on the chip body toggles selection (if
                // selectable). Confirm the click is inside the chip box at all.
                let inside = layout
                    .box_of(root)
                    .map(|r| r.contains(p))
                    .unwrap_or(false);
                if inside {
                    self.toggle_selected()
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
            // Space/Enter toggles selection on a selectable chip.
            keys::SPACE | keys::ENTER if self.selectable => self.toggle_selected(),
            // Delete/Backspace removes a removable chip.
            keys::DELETE | keys::BACKSPACE if self.removable => {
                WidgetOutcome::action(REMOVE_ACTION)
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled && (self.selectable || self.removable)
    }

    fn render(&self) -> TemplateNode {
        let interactive = self.selectable || self.removable;
        let mut chip = TemplateNode::el("lq-chip")
            .attr(
                FOCUSABLE_ATTR,
                if interactive && !self.disabled { "true" } else { "false" },
            )
            .class_if("selectable", self.selectable)
            .class_if("removable", self.removable)
            .class_if("selected", self.selected)
            .pseudo_if(PseudoStateFlags::CHECKED, self.selected)
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-chip-label")
                    .attr("data-part", "label")
                    .child(TemplateNode::text(&self.label)),
            );

        if self.removable {
            chip = chip.child(
                TemplateNode::el("lq-chip-remove")
                    .attr("data-part", "remove")
                    .attr("role", "button")
                    .attr("aria-label", "Remove")
                    .child(TemplateNode::text("\u{00D7}")), // ×
            );
        }
        if self.disabled {
            chip = chip.attr("disabled", "true");
        }
        chip
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
