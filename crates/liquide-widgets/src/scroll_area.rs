//! `<lq-scroll-area>` — a scrollable viewport (Group B).
//!
//! ## Scroll-offset / overflow finding (the CONFIRM-1 gate)
//!
//! The layout engine **does** support overflow: a box with `overflow: auto|scroll`
//! and content larger than its box becomes a scroll container
//! (`liquide-layout/src/block.rs` computes `scroll_size`), the painter clips to
//! the viewport (`overflow: hidden|auto|scroll` -> `PushClip`) and translates
//! children by the box's `scroll_offset` (`liquide-paint/src/painter/mod.rs`).
//!
//! BUT the engine's `scroll_offset` is a **transient layout-tree field**, reset to
//! `(0,0)` on every layout pass and **not persisted in the DOM**. Driving it would
//! need a pipeline API (`set_scroll_offset` on the live tree, re-paint without a
//! relayout) that lives in `liquide-session`/`liquide-shell` — outside this crate's
//! lock. So this widget implements scrolling **within the lock**: it tracks the
//! scroll offset in WIDGET STATE and translates the content element via an inline
//! NEGATIVE margin, while the viewport clips with `overflow: hidden`. The clip +
//! the translated content together produce a real scrolled, clipped viewport
//! through the unmodified pipeline.
//!
//! The scrollbar thumb's size/position are derived from the LAID-OUT viewport and
//! content boxes (`LayoutQuery`), never constants: thumb fraction = viewport /
//! content, thumb offset = (scroll / scrollable-range) along the laid-out track.
//!
//! Behavior:
//! - **Wheel** (`Scroll` event): adds the delta to the offset (clamped to
//!   `[0, content - viewport]`); the content translates + the thumb moves.
//! - **Drag the vthumb** (`data-part="vthumb"`): maps the thumb's travel along the
//!   laid-out track back to a scroll offset.
//! - **Keyboard** (when focused): PageUp/PageDown by a viewport-ish page, Home/End
//!   to the ends, arrows by a line step.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::{Point, Rect};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action emitted when the scroll offset changes.
pub const SCROLLED_ACTION: &str = "scrolled";

/// A line-step (px) for arrow-key scrolling.
const LINE_STEP: f32 = 24.0;

/// A vertically scrollable viewport.
#[derive(Debug, Clone)]
pub struct ScrollArea {
    /// Slotted content subtrees (the tall content).
    content: Vec<TemplateNode>,
    /// Current vertical scroll offset (px from the top), always clamped.
    scroll_y: f32,
    /// Whether a thumb drag is in progress.
    dragging: bool,
    /// Pointer y at drag start (screen space).
    drag_start_y: f32,
    /// Scroll offset at drag start.
    drag_start_scroll: f32,
    hovered: bool,
    /// Cached thumb size fraction (viewport/content), updated from the LAID-OUT
    /// boxes whenever an event is processed, so `render` can emit the thumb's
    /// height as a percentage of the real track. `1.0` until the first layout is
    /// observed (full-height thumb -> always paints).
    thumb_frac: f32,
    /// Cached thumb top fraction (scroll/scrollable-range), from the laid-out
    /// boxes — drives the thumb's `top` percentage in `render`.
    thumb_top_frac: f32,
    /// Cached laid-out track height (px), so `render` can size/offset the thumb in
    /// PIXELS (CSS percentage margins resolve against width, not height, so a
    /// percentage top-margin would not move the thumb vertically). `0.0` until the
    /// first layout is observed.
    track_h: f32,
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self {
            content: Vec::new(),
            scroll_y: 0.0,
            dragging: false,
            drag_start_y: 0.0,
            drag_start_scroll: 0.0,
            hovered: false,
            thumb_frac: 1.0,
            thumb_top_frac: 0.0,
            track_h: 0.0,
        }
    }
}

impl ScrollArea {
    /// An empty scroll area.
    pub fn new() -> Self {
        Self::default()
    }

    /// Refresh the cached thumb fractions + track height from the laid-out
    /// viewport / content / track boxes so the next `render` sizes and positions
    /// the thumb in PIXELS off the real layout.
    fn refresh_thumb_cache(&mut self, root: NodeId, viewport: Rect, content: Rect, layout: &LayoutQuery) {
        self.thumb_frac = self.thumb_fraction(viewport, content);
        self.thumb_top_frac = self.thumb_offset_fraction(viewport, content);
        if let Some(track) = layout.box_of_part(root, "vtrack") {
            self.track_h = track.height;
        }
    }

    /// Slot a content child subtree.
    pub fn child(mut self, child: TemplateNode) -> Self {
        self.content.push(child);
        self
    }

    /// Slot a plain-text content child.
    pub fn text(self, text: &str) -> Self {
        self.child(TemplateNode::text(text))
    }

    /// The current vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// Whether a thumb drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// The maximum scroll offset given the laid-out viewport + content heights:
    /// `max(0, content_h - viewport_h)`. Derived from layout, not a constant.
    fn max_scroll(viewport: Rect, content: Rect) -> f32 {
        (content.height - viewport.height).max(0.0)
    }

    /// Read the laid-out viewport + content boxes (by data-part) from layout.
    fn boxes(root: NodeId, layout: &LayoutQuery) -> Option<(Rect, Rect)> {
        let viewport = layout.box_of_part(root, "viewport")?;
        let content = layout.box_of_part(root, "content")?;
        Some((viewport, content))
    }

    /// Clamp + apply a new scroll offset; returns the outcome (Action on change).
    fn apply_scroll(&mut self, new_y: f32, max: f32) -> WidgetOutcome {
        let clamped = new_y.clamp(0.0, max);
        if (clamped - self.scroll_y).abs() < f32::EPSILON {
            return WidgetOutcome::Ignored;
        }
        self.scroll_y = clamped;
        WidgetOutcome::action_with(SCROLLED_ACTION, format!("{clamped}"))
    }

    /// The thumb height fraction (viewport / content), in `(0, 1]`. Drives the CSS
    /// thumb size off the LAID-OUT boxes.
    pub fn thumb_fraction(&self, viewport: Rect, content: Rect) -> f32 {
        if content.height <= 0.0 {
            return 1.0;
        }
        (viewport.height / content.height).clamp(0.05, 1.0)
    }

    /// The thumb top fraction (scroll / scrollable-range), in `[0, 1]`.
    pub fn thumb_offset_fraction(&self, viewport: Rect, content: Rect) -> f32 {
        let max = Self::max_scroll(viewport, content);
        if max <= 0.0 {
            return 0.0;
        }
        (self.scroll_y / max).clamp(0.0, 1.0)
    }
}

impl WidgetBehavior for ScrollArea {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Container
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseEnter,
            DomEventKind::MouseLeave,
            DomEventKind::Scroll { dx: 0.0, dy: 0.0 },
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
            DomEventKind::MouseUp {
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
        let Some((viewport, content)) = Self::boxes(root, layout) else {
            return WidgetOutcome::Ignored;
        };
        let max = Self::max_scroll(viewport, content);

        let outcome = match &event.kind {
            DomEventKind::MouseEnter => {
                if self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = true;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseLeave => {
                if !self.hovered && !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = false;
                WidgetOutcome::Changed
            }
            DomEventKind::Scroll { dy, .. } => {
                // Wheel scrolls by the delta; the content translates + thumb moves.
                self.apply_scroll(self.scroll_y + *dy, max)
            }
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x,
                y,
            } => {
                // Press on the thumb begins a drag; press on the track jumps a
                // page toward the press; press in the content does nothing.
                let p = Point::new(*x, *y);
                if let Some(thumb) = layout.box_of_part(root, "vthumb") {
                    if thumb.contains(p) {
                        self.dragging = true;
                        self.drag_start_y = *y;
                        self.drag_start_scroll = self.scroll_y;
                        return WidgetOutcome::Changed;
                    }
                }
                if let Some(track) = layout.box_of_part(root, "vtrack") {
                    if track.contains(p) {
                        // Page toward the click position.
                        let page = viewport.height.max(LINE_STEP);
                        let dir = if *y < track.y + track.height / 2.0 {
                            -1.0
                        } else {
                            1.0
                        };
                        return self.apply_scroll(self.scroll_y + dir * page, max);
                    }
                }
                WidgetOutcome::Ignored
            }
            DomEventKind::MouseMove { y, .. } => {
                if !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                // Map the thumb's pixel travel along the LAID-OUT track back to a
                // scroll offset: dragging the thumb across the whole track travel
                // scrolls the whole scrollable range.
                let Some(track) = layout.box_of_part(root, "vtrack") else {
                    return WidgetOutcome::Ignored;
                };
                let thumb_h = self.thumb_fraction(viewport, content) * track.height;
                let travel = (track.height - thumb_h).max(1.0);
                let dy = *y - self.drag_start_y;
                let scroll_per_px = if travel > 0.0 { max / travel } else { 0.0 };
                let new_scroll = self.drag_start_scroll + dy * scroll_per_px;
                self.apply_scroll(new_scroll, max)
            }
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                self.dragging = false;
                WidgetOutcome::Changed
            }
            _ => WidgetOutcome::Ignored,
        };
        // Recompute the thumb size/position from the laid-out boxes + the (now
        // updated) scroll offset, so the re-render positions the thumb correctly.
        self.refresh_thumb_cache(root, viewport, content, layout);
        outcome
    }

    fn on_keyboard(
        &mut self,
        root: NodeId,
        key: KeyInput,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        let Some((viewport, content)) = Self::boxes(root, layout) else {
            return WidgetOutcome::Ignored;
        };
        let max = Self::max_scroll(viewport, content);
        let page = viewport.height.max(LINE_STEP);
        let outcome = match key.key {
            keys::ARROW_DOWN => self.apply_scroll(self.scroll_y + LINE_STEP, max),
            keys::ARROW_UP => self.apply_scroll(self.scroll_y - LINE_STEP, max),
            keys::PAGE_DOWN => self.apply_scroll(self.scroll_y + page, max),
            keys::PAGE_UP => self.apply_scroll(self.scroll_y - page, max),
            keys::HOME => self.apply_scroll(0.0, max),
            keys::END => self.apply_scroll(max, max),
            _ => WidgetOutcome::Ignored,
        };
        self.refresh_thumb_cache(root, viewport, content, layout);
        outcome
    }

    fn focusable(&self) -> bool {
        true
    }

    fn render(&self) -> TemplateNode {
        // The content is translated UP by the scroll offset via a negative
        // top-margin; the viewport clips with overflow:hidden (CSS). This is the
        // in-lock scroll: real clip + real translate through the pipeline.
        let content = TemplateNode::el("lq-scroll-content")
            .attr("data-part", "content")
            .style("margin-top", &format!("{}px", -self.scroll_y))
            .children(self.content.clone());

        let viewport = TemplateNode::el("lq-scroll-viewport")
            .attr("data-part", "viewport")
            .child(content);

        // The scrollbar: a track containing a thumb whose size/position are sized
        // in PIXELS off the LAID-OUT track height (cached during event handling in
        // `refresh_thumb_cache`). Pixels — not percentages — because CSS percentage
        // margins/heights resolve against the containing-block WIDTH, so a
        // percentage top-margin would not move the thumb vertically. The thumb
        // height = track_h * (viewport/content); its top offset = (track_h -
        // thumb_h) * (scroll/range). CSS owns the track height; the laid-out
        // viewport/content own the fractions. Before the first event `track_h` is 0
        // so the thumb falls back to the CSS default (full-height -> always paints).
        let thumb = if self.track_h > 0.0 {
            let thumb_h = (self.thumb_frac * self.track_h).clamp(8.0, self.track_h);
            let travel = (self.track_h - thumb_h).max(0.0);
            let thumb_top = (self.thumb_top_frac * travel).clamp(0.0, travel);
            TemplateNode::el("lq-scroll-thumb")
                .attr("data-part", "vthumb")
                .style("height", &format!("{thumb_h}px"))
                .style("margin-top", &format!("{thumb_top}px"))
                .pseudo_if(PseudoStateFlags::ACTIVE, self.dragging)
        } else {
            TemplateNode::el("lq-scroll-thumb")
                .attr("data-part", "vthumb")
                .pseudo_if(PseudoStateFlags::ACTIVE, self.dragging)
        };

        let track = TemplateNode::el("lq-scroll-track")
            .attr("data-part", "vtrack")
            .child(thumb);

        TemplateNode::el("lq-scroll-area")
            .attr(FOCUSABLE_ATTR, "true")
            .attr("data-scroll-y", &format!("{}", self.scroll_y))
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.dragging)
            .child(viewport)
            .child(track)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
