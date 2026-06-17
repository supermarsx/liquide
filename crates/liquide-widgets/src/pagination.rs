//! `<lq-pagination>` — prev/next + numbered page buttons with ellipses (Group D: D4).
//!
//! State: a total page count + a current page (0-based internally, 1-based in the
//! visible labels). The rendered control is a row of `data-part`-tagged buttons:
//! a `prev` button, a windowed set of `page-<n>` number buttons (with `ellipsis`
//! gaps when the range is long), and a `next` button. Behavior:
//!
//! - **Click** a page button's LAID-OUT box jumps to that page; **prev**/**next**
//!   step by one (disabled + un-clickable at the ends). All hit-tested per-button
//!   from layout (`data-part`), never an index over a constant button width.
//! - **Left/Right** step the page when focused; **Home/End** jump to first/last.
//! - The current page button carries `:checked`/`.current`; the disabled
//!   prev/next ends carry `:disabled`. Ellipsis gaps are inert.
//! - Emits `Changed`(page) (the new 0-based page index) when the page changes.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the current page changes (payload: the 0-based page index).
pub const CHANGED_ACTION: &str = "changed";

/// A rendered slot in the page row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    /// A clickable page number (0-based page index).
    Page(usize),
    /// A non-interactive "…" gap.
    Ellipsis,
}

/// A numbered pagination control.
#[derive(Debug, Clone)]
pub struct Pagination {
    /// Total number of pages (>= 1).
    total: usize,
    /// The current page (0-based).
    current: usize,
    /// How many page numbers to show around the current page (sibling count).
    siblings: usize,
    /// Hovered page index, if any.
    hovered: Option<usize>,
}

impl Pagination {
    /// A control over `total` pages (clamped to >= 1) starting on page 0.
    pub fn new(total: usize) -> Self {
        Self {
            total: total.max(1),
            current: 0,
            siblings: 1,
            hovered: None,
        }
    }

    /// Set the current page (0-based, clamped).
    pub fn page(mut self, page: usize) -> Self {
        self.current = page.min(self.total - 1);
        self
    }

    /// Set the sibling count (page numbers either side of the current page).
    pub fn siblings(mut self, n: usize) -> Self {
        self.siblings = n;
        self
    }

    /// The current page (0-based).
    pub fn current_page(&self) -> usize {
        self.current
    }

    /// The total page count.
    pub fn total_pages(&self) -> usize {
        self.total
    }

    fn page_part(n: usize) -> String {
        format!("page-{n}")
    }

    /// Compute the visible slot row (first, …, window, …, last) — the page
    /// numbers actually rendered. Always includes page 0 and the last page.
    fn slots(&self) -> Vec<Slot> {
        let n = self.total;
        // Small page counts: show all.
        // Window size = current ± siblings, plus the two ends, plus two ellipses.
        let max_numbers = self.siblings * 2 + 5;
        if n <= max_numbers {
            return (0..n).map(Slot::Page).collect();
        }

        let mut out = Vec::new();
        let left = self.current.saturating_sub(self.siblings);
        let right = (self.current + self.siblings).min(n - 1);

        // Always page 0.
        out.push(Slot::Page(0));
        // Left ellipsis (or page 1 if adjacent).
        if left > 2 {
            out.push(Slot::Ellipsis);
        } else {
            for p in 1..left {
                out.push(Slot::Page(p));
            }
        }
        // The window.
        let win_lo = left.max(1);
        let win_hi = right.min(n - 2);
        for p in win_lo..=win_hi {
            out.push(Slot::Page(p));
        }
        // Right ellipsis (or the pages up to the last).
        if right < n - 3 {
            out.push(Slot::Ellipsis);
        } else {
            for p in (right + 1)..(n - 1) {
                out.push(Slot::Page(p));
            }
        }
        // Always the last page.
        out.push(Slot::Page(n - 1));
        out
    }

    fn set_page(&mut self, page: usize) -> WidgetOutcome {
        let p = page.min(self.total - 1);
        if p == self.current {
            return WidgetOutcome::Ignored;
        }
        self.current = p;
        WidgetOutcome::action_with(CHANGED_ACTION, p.to_string())
    }

    fn prev(&mut self) -> WidgetOutcome {
        if self.current == 0 {
            return WidgetOutcome::Ignored;
        }
        self.set_page(self.current - 1)
    }

    fn next(&mut self) -> WidgetOutcome {
        if self.current + 1 >= self.total {
            return WidgetOutcome::Ignored;
        }
        self.set_page(self.current + 1)
    }

    /// Which actionable button's LAID-OUT box contains the point. Returns one of
    /// the part names ("prev"/"next"/"page-<n>") that was hit, ignoring ellipses.
    fn button_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<Hit> {
        if let Some(r) = layout.box_of_part(root, "prev") {
            if r.contains(point) {
                return Some(Hit::Prev);
            }
        }
        if let Some(r) = layout.box_of_part(root, "next") {
            if r.contains(point) {
                return Some(Hit::Next);
            }
        }
        for slot in self.slots() {
            if let Slot::Page(p) = slot {
                if let Some(r) = layout.box_of_part(root, &Self::page_part(p)) {
                    if r.contains(point) {
                        return Some(Hit::Page(p));
                    }
                }
            }
        }
        None
    }
}

/// A resolved hit target.
enum Hit {
    Prev,
    Next,
    Page(usize),
}

impl WidgetBehavior for Pagination {
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
                let hit = match self.button_at(root, Point::new(*x, *y), layout) {
                    Some(Hit::Page(p)) => Some(p),
                    _ => None,
                };
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
            } => match self.button_at(root, Point::new(*x, *y), layout) {
                Some(Hit::Prev) => self.prev(),
                Some(Hit::Next) => self.next(),
                Some(Hit::Page(p)) => self.set_page(p),
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
        match key.key {
            keys::ARROW_LEFT => self.prev(),
            keys::ARROW_RIGHT => self.next(),
            keys::HOME => self.set_page(0),
            keys::END => self.set_page(self.total - 1),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn render(&self) -> TemplateNode {
        let at_start = self.current == 0;
        let at_end = self.current + 1 >= self.total;

        let mut nav = TemplateNode::el("lq-pagination")
            .attr(FOCUSABLE_ATTR, "true")
            .attr("role", "navigation");

        // Prev button.
        nav = nav.child(
            TemplateNode::el("lq-page-btn")
                .key("prev")
                .attr("data-part", "prev")
                .attr("data-action", "prev")
                .class("prev")
                .class_if("disabled", at_start)
                .pseudo_if(PseudoStateFlags::DISABLED, at_start)
                .child(TemplateNode::text("‹")),
        );

        // Page-number buttons (+ ellipses).
        for (si, slot) in self.slots().into_iter().enumerate() {
            match slot {
                Slot::Page(p) => {
                    let cur = p == self.current;
                    nav = nav.child(
                        TemplateNode::el("lq-page-btn")
                            .key(&format!("page-{p}"))
                            .attr("data-part", &Self::page_part(p))
                            .attr("data-page", &format!("{p}"))
                            .class("page")
                            .class_if("current", cur)
                            .attr("aria-current", if cur { "page" } else { "false" })
                            .pseudo_if(PseudoStateFlags::CHECKED, cur)
                            .pseudo_if(
                                PseudoStateFlags::HOVER,
                                self.hovered == Some(p) && !cur,
                            )
                            // 1-based label.
                            .child(TemplateNode::text(&(p + 1).to_string())),
                    );
                }
                Slot::Ellipsis => {
                    nav = nav.child(
                        TemplateNode::el("lq-page-ellipsis")
                            .key(&format!("ellipsis-{si}"))
                            .attr("data-part", "ellipsis")
                            .child(TemplateNode::text("…")),
                    );
                }
            }
        }

        // Next button.
        nav = nav.child(
            TemplateNode::el("lq-page-btn")
                .key("next")
                .attr("data-part", "next")
                .attr("data-action", "next")
                .class("next")
                .class_if("disabled", at_end)
                .pseudo_if(PseudoStateFlags::DISABLED, at_end)
                .child(TemplateNode::text("›")),
        );

        nav
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
