//! `<lq-button>` — a clickable command control (Group A: A1).
//!
//! Behavior:
//! - **Click** on the laid-out box -> [`WidgetOutcome::Action`] carrying the
//!   button's `data-action` name (the chrome `data-action` convention).
//! - **MouseDown/MouseUp** drive the `:active` pseudo-state; **MouseEnter/Leave**
//!   drive `:hover`; both restyle pixels via CSS.
//! - **Disabled** buttons swallow clicks (no action), carry `:disabled`, and drop
//!   out of the focus ring (`data-focusable="false"`).
//! - **Keyboard**: when focused, **Enter** or **Space** activate (same Action a
//!   click produces).
//!
//! Variants (`primary` / `danger` / `ghost` / `icon`) are pure CSS classes the
//! caller supplies; the behavior is identical.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// A push button. Emits its `action` on click / Enter / Space.
#[derive(Debug, Clone)]
pub struct Button {
    label: String,
    action: String,
    /// Extra CSS class (e.g. `"primary"`, `"danger"`, `"ghost"`) or empty.
    variant: String,
    /// Optional icon name drawn BEFORE the label; `None` = label-only.
    icon: Option<String>,
    disabled: bool,
    pressed: bool,
    hovered: bool,
    activations: u32,
}

impl Button {
    /// A button labelled `label` that emits `action`.
    pub fn new(label: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action: action.into(),
            variant: String::new(),
            icon: None,
            disabled: false,
            pressed: false,
            hovered: false,
            activations: 0,
        }
    }

    /// Set a CSS variant class (`primary` / `danger` / `ghost` / `icon`).
    pub fn variant(mut self, v: impl Into<String>) -> Self {
        self.variant = v.into();
        self
    }

    /// Set an icon name drawn BEFORE the label (resolves through the shared icon
    /// name-map at paint time). An empty name leaves the button icon-less.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        let name = icon.into();
        self.icon = if name.is_empty() { None } else { Some(name) };
        self
    }

    /// Mark the button disabled (swallows clicks, drops from focus ring).
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// How many times the button has activated (click or keyboard).
    pub fn activations(&self) -> u32 {
        self.activations
    }

    /// Whether the button is in its `:active` (pressed) state.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Whether the button is in its `:hover` state.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Whether the button is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    fn activate(&mut self) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        self.activations += 1;
        WidgetOutcome::action(self.action.clone())
    }
}

impl WidgetBehavior for Button {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Button
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
                if !self.hovered && !self.pressed {
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
                if self.disabled || self.pressed {
                    return WidgetOutcome::Ignored;
                }
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
                // Hit-test against the LAID-OUT box (never a constant): only a
                // click landing inside the button's real box activates it.
                let inside = layout
                    .box_of(root)
                    .map(|r| r.contains(liquide_layout::geometry::Point::new(*x, *y)))
                    .unwrap_or(false);
                if !inside {
                    return WidgetOutcome::Ignored;
                }
                self.activate()
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
        // Enter or Space activate a focused button.
        if key.key == keys::ENTER || key.key == keys::SPACE {
            return self.activate();
        }
        WidgetOutcome::Ignored
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut node = TemplateNode::el("lq-button")
            .attr("data-action", &self.action)
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .class_if(&self.variant, !self.variant.is_empty())
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered && !self.disabled)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.pressed && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);
        // A button WITH an icon emits a dedicated `lq-button-icon` leaf carrying
        // `data-icon` BEFORE the label, so the paint path draws the glyph (the
        // name-map resolves it to a non-zero IconId). The whole button stays the
        // click target — the icon is a small inline child that never steals the
        // hit. Icon-less buttons render label-only, exactly as before (no leaf).
        if let Some(icon) = &self.icon {
            node = node.child(TemplateNode::el("lq-button-icon").attr("data-icon", icon));
        }
        node = node.child(
            TemplateNode::el("lq-label")
                .attr("data-part", "label")
                .child(TemplateNode::text(&self.label)),
        );
        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
