//! `<lq-popover>` — an anchored floating panel relative to a trigger
//! (NAV/OVERLAY family).
//!
//! A popover has a `data-part="trigger"` (always present) and, while open, a
//! `data-part="panel"` floated relative to the trigger by a placement
//! (top/bottom/left/right). Behavior:
//!
//! - **Click the trigger**: toggles the panel open/closed.
//! - **Click outside** the trigger AND the panel: closes (`Action`(`close`)).
//! - **Esc**: closes.
//! - The panel's position is **derived from the trigger's LAID-OUT box** at
//!   render time: [`Popover::panel_offset`] returns the (dx, dy) the panel is
//!   translated by for a given placement, computed from the trigger rect — never
//!   a hardcoded position. The render emits that as an inline `transform:
//!   translate(...)` on the panel so paint + hit-test agree.
//!
//! Because the offset is a pure function of the trigger box + placement, a test
//! can resize the trigger and assert the panel lands at the geometrically-correct
//! spot for that placement (a constant could not track it).

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::{Point, Rect};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the popover closes via outside-click or Esc.
pub const CLOSE_ACTION: &str = "close";

/// Where the panel floats relative to the trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Above the trigger.
    Top,
    /// Below the trigger.
    Bottom,
    /// Left of the trigger.
    Left,
    /// Right of the trigger.
    Right,
}

impl Placement {
    fn as_str(self) -> &'static str {
        match self {
            Placement::Top => "top",
            Placement::Bottom => "bottom",
            Placement::Left => "left",
            Placement::Right => "right",
        }
    }
}

/// Gap (px) between the trigger and the floated panel.
const GAP: f32 = 6.0;

/// An anchored floating popover.
#[derive(Debug, Clone)]
pub struct Popover {
    placement: Placement,
    open: bool,
    label: String,
    content: String,
    /// The measured (dx, dy) px offset the panel takes from the trigger's
    /// top-left, resolved by [`reposition`](Self::reposition) once the trigger +
    /// panel boxes are laid out. `None` = not yet measured (panel renders at the
    /// trigger origin until the first reposition pass).
    offset: Option<(f32, f32)>,
}

impl Popover {
    /// A popover whose trigger shows `label`, panel placed by `placement`.
    pub fn new(label: impl Into<String>, placement: Placement) -> Self {
        Self {
            placement,
            open: false,
            label: label.into(),
            content: String::new(),
            offset: None,
        }
    }

    /// Start open.
    pub fn open(mut self, o: bool) -> Self {
        self.open = o;
        self
    }

    /// Set the panel content.
    pub fn content(mut self, c: impl Into<String>) -> Self {
        self.content = c.into();
        self
    }

    /// Whether the panel is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The placement.
    pub fn placement(&self) -> Placement {
        self.placement
    }

    /// The (dx, dy) translation the panel takes relative to its own static
    /// (trigger-origin-anchored) position, for `placement`, given the trigger's
    /// laid-out rect and the panel's laid-out size.
    ///
    /// The panel is authored `position:absolute; left:0; top:0` inside the
    /// popover (so its static origin is the trigger's top-left); this offset moves
    /// it to the placement edge with a [`GAP`]. Centering along the cross axis
    /// uses the trigger/panel extents — all from layout, no constants.
    pub fn panel_offset(placement: Placement, trigger: Rect, panel: Rect) -> (f32, f32) {
        let (tw, th) = (trigger.width, trigger.height);
        let (pw, ph) = (panel.width, panel.height);
        match placement {
            Placement::Bottom => ((tw - pw) / 2.0, th + GAP),
            Placement::Top => ((tw - pw) / 2.0, -(ph + GAP)),
            Placement::Right => (tw + GAP, (th - ph) / 2.0),
            Placement::Left => (-(pw + GAP), (th - ph) / 2.0),
        }
    }

    /// The currently-measured panel offset (px), if [`reposition`](Self::reposition)
    /// has run.
    pub fn offset(&self) -> Option<(f32, f32)> {
        self.offset
    }

    /// Resolve the panel's placement position from the laid-out trigger + panel
    /// boxes (the second pass of the standard measure-then-place flow).
    ///
    /// The panel is `position:absolute`, which in this engine resolves against the
    /// viewport origin (an inline-block ancestor does not establish a containing
    /// block), so the stored offset is the panel's ABSOLUTE screen `left`/`top` =
    /// the trigger's screen origin plus the trigger-relative
    /// [`panel_offset`](Self::panel_offset). The next
    /// [`render`](WidgetBehavior::render) emits those as inline px `left`/`top`.
    /// Returns `Changed` when the position moved enough to need a re-render.
    pub fn reposition(&mut self, trigger: Rect, panel: Rect) -> WidgetOutcome {
        let (dx, dy) = Self::panel_offset(self.placement, trigger, panel);
        let abs = (trigger.x + dx, trigger.y + dy);
        let moved = match self.offset {
            Some((x, y)) => (x - abs.0).abs() > 0.5 || (y - abs.1).abs() > 0.5,
            None => true,
        };
        self.offset = Some(abs);
        if moved {
            WidgetOutcome::Changed
        } else {
            WidgetOutcome::Ignored
        }
    }

    fn box_hit(&self, root: NodeId, part: &str, point: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, part)
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }

    fn toggle(&mut self) -> WidgetOutcome {
        self.open = !self.open;
        WidgetOutcome::Changed
    }

    fn close(&mut self) -> WidgetOutcome {
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = false;
        WidgetOutcome::action(CLOSE_ACTION)
    }
}

impl WidgetBehavior for Popover {
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
        if let DomEventKind::Click {
            button: MouseButton::Left,
            x,
            y,
        } = &event.kind
        {
            let p = Point::new(*x, *y);
            if self.box_hit(root, "trigger", p, layout) {
                return self.toggle();
            }
            if self.open {
                // A click inside the panel is swallowed; anywhere else closes.
                if self.box_hit(root, "panel", p, layout) {
                    return WidgetOutcome::Ignored;
                }
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
        true
    }

    fn render(&self) -> TemplateNode {
        let mut root = TemplateNode::el("lq-popover")
            .attr(FOCUSABLE_ATTR, "true")
            .attr("data-placement", self.placement.as_str())
            .attr("aria-expanded", if self.open { "true" } else { "false" })
            .class_if("open", self.open)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.open);

        root = root.child(
            TemplateNode::el("lq-popover-trigger")
                .attr("data-part", "trigger")
                .attr("role", "button")
                .pseudo_if(PseudoStateFlags::ACTIVE, self.open)
                .child(TemplateNode::text(&self.label)),
        );

        if self.open {
            // The panel is absolutely anchored at the trigger's top-left
            // (position:absolute; left:0; top:0 inside the relative popover) and
            // shifted to the placement edge by an inline px MARGIN computed from
            // the laid-out trigger + panel boxes (set via `reposition`). A px
            // margin shifts the LAYOUT box (unlike a transform), so `box_of_part`
            // reports the real placed position and the hit-test agrees with paint.
            // Percent offsets are NOT used (the engine does not resolve % left/top
            // on absolute children against the parent's real size).
            let mut panel = TemplateNode::el("lq-popover-panel")
                .attr("data-part", "panel")
                .attr("data-placement", self.placement.as_str())
                .class(self.placement.as_str())
                .attr("role", "dialog");
            if let Some((dx, dy)) = self.offset {
                // Absolute panel anchored at the trigger origin; an explicit px
                // left/top (NOT margin, NOT percent — neither shifts an absolute
                // box here) places it at the geometric placement so `box_of_part`
                // reports the real position.
                panel = panel
                    .style("left", &format!("{dx}px"))
                    .style("top", &format!("{dy}px"));
            }
            panel = panel.child(TemplateNode::text(&self.content));
            root = root.child(panel);
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
