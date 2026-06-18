//! `<lq-drawer>` — a slide-in sheet panel with a scrim (NAV/OVERLAY family).
//!
//! A drawer is an off-canvas panel that slides in from a screen edge
//! (left/right/top/bottom), backed by a dimming `data-part="scrim"` over the rest
//! of the surface. The `data-part="panel"` holds arbitrary content (set via
//! [`Drawer::content`]). Behavior:
//!
//! - **Open / closed** is a state bit. When closed the drawer emits NO scrim and
//!   NO panel box (so a closed drawer hit-tests to nothing — a constant could not
//!   know that). The `.open` class + `data-edge` drive the slide-in CSS.
//! - **Click the scrim** (a click that lands in the scrim box but NOT the panel
//!   box): closes. **Esc**: closes. Both emit `Action`(`close`).
//! - A click inside the panel is swallowed (does not close).
//! - Opening/closing programmatically via [`Drawer::set_open`].
//!
//! The scrim/panel split is hit-tested from the LAID-OUT boxes, so the
//! click-outside-to-close behaviour tracks the real geometry, not a guess.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the drawer closes via scrim-click or Esc.
pub const CLOSE_ACTION: &str = "close";

/// Which edge the drawer slides in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// Slides in from the left.
    Left,
    /// Slides in from the right.
    Right,
    /// Slides in from the top.
    Top,
    /// Slides in from the bottom.
    Bottom,
}

impl Edge {
    fn as_str(self) -> &'static str {
        match self {
            Edge::Left => "left",
            Edge::Right => "right",
            Edge::Top => "top",
            Edge::Bottom => "bottom",
        }
    }
}

/// A slide-in drawer / sheet panel.
#[derive(Debug, Clone)]
pub struct Drawer {
    edge: Edge,
    open: bool,
    title: Option<String>,
    /// Panel body text content (a simple content slot for the gallery).
    content: String,
    /// Whether a click on the scrim closes the drawer (default true).
    dismissable: bool,
}

impl Drawer {
    /// A drawer that slides in from `edge` (closed by default).
    pub fn new(edge: Edge) -> Self {
        Self {
            edge,
            open: false,
            title: None,
            content: String::new(),
            dismissable: true,
        }
    }

    /// Start open.
    pub fn open(mut self, o: bool) -> Self {
        self.open = o;
        self
    }

    /// Set a header title.
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Set the panel body content.
    pub fn content(mut self, c: impl Into<String>) -> Self {
        self.content = c.into();
        self
    }

    /// Whether scrim-click dismisses the drawer.
    pub fn dismissable(mut self, d: bool) -> Self {
        self.dismissable = d;
        self
    }

    /// Whether the drawer is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The edge the drawer slides in from.
    pub fn edge(&self) -> Edge {
        self.edge
    }

    /// Programmatically open/close. Returns `Changed` when the state flipped.
    pub fn set_open(&mut self, open: bool) -> WidgetOutcome {
        if self.open == open {
            return WidgetOutcome::Ignored;
        }
        self.open = open;
        WidgetOutcome::Changed
    }

    fn close(&mut self) -> WidgetOutcome {
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = false;
        WidgetOutcome::action(CLOSE_ACTION)
    }

    fn box_hit(&self, root: NodeId, part: &str, point: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, part)
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }
}

impl WidgetBehavior for Drawer {
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
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        if let DomEventKind::Click {
            button: MouseButton::Left,
            x,
            y,
        } = &event.kind
        {
            let p = Point::new(*x, *y);
            // A click inside the panel is swallowed (does not close).
            if self.box_hit(root, "panel", p, layout) {
                return WidgetOutcome::Ignored;
            }
            // A click on the scrim (but outside the panel) closes when allowed.
            if self.dismissable && self.box_hit(root, "scrim", p, layout) {
                return self.close();
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
        if self.open && key.key == keys::ESCAPE {
            self.close()
        } else {
            WidgetOutcome::Ignored
        }
    }

    fn focusable(&self) -> bool {
        self.open
    }

    fn render(&self) -> TemplateNode {
        let mut root = TemplateNode::el("lq-drawer")
            .attr(FOCUSABLE_ATTR, if self.open { "true" } else { "false" })
            .attr("role", "dialog")
            .attr("data-edge", self.edge.as_str())
            .attr("aria-hidden", if self.open { "false" } else { "true" })
            .class_if("open", self.open)
            .class(self.edge.as_str())
            .pseudo_if(PseudoStateFlags::ACTIVE, self.open);

        if !self.open {
            // A closed drawer paints nothing (no scrim, no panel).
            return root;
        }

        // The scrim covers the whole surface behind the panel.
        root = root.child(
            TemplateNode::el("lq-drawer-scrim")
                .attr("data-part", "scrim")
                .attr("aria-hidden", "true"),
        );

        // The sliding panel.
        let mut panel = TemplateNode::el("lq-drawer-panel")
            .attr("data-part", "panel")
            .attr("data-edge", self.edge.as_str())
            .class(self.edge.as_str())
            .attr("role", "document");

        if let Some(title) = &self.title {
            panel = panel.child(
                TemplateNode::el("lq-drawer-header")
                    .attr("data-part", "header")
                    .child(TemplateNode::text(title)),
            );
        }
        panel = panel.child(
            TemplateNode::el("lq-drawer-body")
                .attr("data-part", "body")
                .child(TemplateNode::text(&self.content)),
        );
        root.child(panel)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
