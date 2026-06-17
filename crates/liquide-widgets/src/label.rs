//! `<lq-label>` / `<lq-link>` — static and navigable text (Group A: A2).
//!
//! - [`Label`] (`<lq-label>`) is inert: pure text, no events, not focusable. It
//!   closes the "static text" requirement and is reused as a sub-part by other
//!   widgets (button label, list-item text).
//! - [`Link`] (`<lq-link>`) is a focusable, clickable navigation control: a click
//!   (inside the laid-out box) or Enter (when focused) emits a `navigate` Action
//!   carrying the link's `href` payload. It carries `:hover`/`:active`/`:focus`
//!   states so CSS restyles it.
//!
//! `:visited` history is not modelled yet (no navigation history store) — left as
//! a documented follow-up.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action name a link emits when activated.
pub const NAVIGATE_ACTION: &str = "navigate";

/// A static, non-interactive text label.
#[derive(Debug, Clone)]
pub struct Label {
    text: String,
}

impl Label {
    /// A label showing `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// The label text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl WidgetBehavior for Label {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Label
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        // A static label asks for nothing and costs nothing.
        Vec::new()
    }

    fn on_dom_event(
        &mut self,
        _root: NodeId,
        _event: &DomEvent,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        WidgetOutcome::Ignored
    }

    fn focusable(&self) -> bool {
        false
    }

    fn render(&self) -> TemplateNode {
        TemplateNode::el("lq-label").child(TemplateNode::text(&self.text))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A focusable, clickable hyperlink. Activating it emits a `navigate` Action with
/// the `href` payload.
#[derive(Debug, Clone)]
pub struct Link {
    text: String,
    href: String,
    hovered: bool,
    pressed: bool,
    navigations: u32,
}

impl Link {
    /// A link labelled `text` pointing at `href`.
    pub fn new(text: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            href: href.into(),
            hovered: false,
            pressed: false,
            navigations: 0,
        }
    }

    /// The link target.
    pub fn href(&self) -> &str {
        &self.href
    }

    /// How many times the link has been activated.
    pub fn navigations(&self) -> u32 {
        self.navigations
    }

    /// Whether the link is hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    fn activate(&mut self) -> WidgetOutcome {
        self.navigations += 1;
        WidgetOutcome::action_with(NAVIGATE_ACTION, self.href.clone())
    }
}

impl WidgetBehavior for Link {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Label
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
        if key.key == keys::ENTER {
            return self.activate();
        }
        WidgetOutcome::Ignored
    }

    fn focusable(&self) -> bool {
        true
    }

    fn render(&self) -> TemplateNode {
        TemplateNode::el("lq-link")
            .attr("data-action", NAVIGATE_ACTION)
            .attr("data-href", &self.href)
            .attr(FOCUSABLE_ATTR, "true")
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.pressed)
            .child(TemplateNode::text(&self.text))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
