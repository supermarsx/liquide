//! `<lq-split-button>` — a primary action button + a caret dropdown (COMP-4).
//!
//! A composite of a primary action zone and a caret zone that opens a menu of
//! secondary actions (the menu/popup pattern, reused from the dropdown). Behavior:
//!
//! - **Click the primary zone** (`data-part="primary"`): emits
//!   `Action`("primary") — actually the primary action id.
//! - **Click the caret zone** (`data-part="caret"`): toggles the secondary menu
//!   open/closed.
//! - **Click a menu item** (menu open, `data-part="item-<i>"`): emits an
//!   `Action`(item id) and closes.
//! - **Click elsewhere while open**: closes (dismiss).
//! - **Keyboard** (focused): Enter/Space fire the primary action; Down opens the
//!   menu (or moves the highlight); Up moves the highlight; Enter (open) fires the
//!   highlighted item; Esc closes.
//!
//! ## Geometry from layout
//!
//! Whether a click is a PRIMARY action or a CARET toggle is decided by which
//! laid-out box (`data-part="primary"` vs `"caret"`) contains the point — never by
//! a constant split fraction. Menu rows hit-test against their laid-out boxes too.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// A split-button widget.
#[derive(Debug, Clone)]
pub struct SplitButton {
    /// (id, label) of the primary action.
    primary: (String, String),
    /// (id, label) secondary menu actions, in order.
    items: Vec<(String, String)>,
    /// Whether the secondary menu is open.
    open: bool,
    /// Highlighted menu item index (keyboard cursor / hover) while open.
    highlighted: Option<usize>,
    /// Hovered menu item (mouse).
    hovered: Option<usize>,
    disabled: bool,
}

impl SplitButton {
    /// A split button with a primary `(id, label)` action and secondary items.
    pub fn new(
        primary_id: impl Into<String>,
        primary_label: impl Into<String>,
        items: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        Self {
            primary: (primary_id.into(), primary_label.into()),
            items: items.into_iter().collect(),
            open: false,
            highlighted: None,
            hovered: None,
            disabled: false,
        }
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Whether the menu is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The primary action id.
    pub fn primary_id(&self) -> &str {
        &self.primary.0
    }

    /// The highlighted menu item index (while open).
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// Number of secondary items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    fn item_part(i: usize) -> String {
        format!("item-{i}")
    }

    fn part_contains(&self, root: NodeId, part: &str, p: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, part)
            .map(|r| r.contains(p))
            .unwrap_or(false)
    }

    fn item_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.items.len() {
            if let Some(r) = layout.box_of_part(root, &Self::item_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn open_menu(&mut self) -> WidgetOutcome {
        if self.open || self.items.is_empty() {
            return WidgetOutcome::Ignored;
        }
        self.open = true;
        self.highlighted = Some(0);
        WidgetOutcome::Changed
    }

    fn close_menu(&mut self) -> WidgetOutcome {
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = false;
        self.highlighted = None;
        self.hovered = None;
        WidgetOutcome::Changed
    }

    fn fire_primary(&self) -> WidgetOutcome {
        WidgetOutcome::action(self.primary.0.clone())
    }

    fn choose_item(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.items.len() {
            return WidgetOutcome::Ignored;
        }
        let id = self.items[idx].0.clone();
        self.open = false;
        self.highlighted = None;
        self.hovered = None;
        WidgetOutcome::action(id)
    }

    fn move_highlight(&mut self, forward: bool) -> WidgetOutcome {
        if self.items.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let n = self.items.len();
        let next = match self.highlighted {
            Some(h) if forward => (h + 1).min(n - 1),
            Some(h) => h.saturating_sub(1),
            None if forward => 0,
            None => n - 1,
        };
        self.highlighted = Some(next);
        WidgetOutcome::Changed
    }
}

impl WidgetBehavior for SplitButton {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Button
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
                if !self.open {
                    return WidgetOutcome::Ignored;
                }
                let hit = self.item_at(root, Point::new(*x, *y), layout);
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
            } => {
                let p = Point::new(*x, *y);
                if self.open {
                    // Click a menu item -> fire; caret -> close; else dismiss.
                    if let Some(i) = self.item_at(root, p, layout) {
                        return self.choose_item(i);
                    }
                    if self.part_contains(root, "caret", p, layout) {
                        return self.close_menu();
                    }
                    return self.close_menu();
                }
                // Closed: primary zone fires, caret zone opens.
                if self.part_contains(root, "primary", p, layout) {
                    return self.fire_primary();
                }
                if self.part_contains(root, "caret", p, layout) {
                    return self.open_menu();
                }
                WidgetOutcome::Ignored
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
            keys::ENTER | keys::SPACE => {
                if self.open {
                    if let Some(i) = self.highlighted {
                        return self.choose_item(i);
                    }
                    WidgetOutcome::Ignored
                } else {
                    self.fire_primary()
                }
            }
            keys::ARROW_DOWN => {
                if !self.open {
                    self.open_menu()
                } else {
                    self.move_highlight(true)
                }
            }
            keys::ARROW_UP => {
                if !self.open {
                    WidgetOutcome::Ignored
                } else {
                    self.move_highlight(false)
                }
            }
            keys::ESCAPE => self.close_menu(),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut root = TemplateNode::el("lq-split-button")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "group")
            .class_if("open", self.open)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        // Primary action zone.
        root = root.child(
            TemplateNode::el("lq-split-primary")
                .attr("data-part", "primary")
                .attr("role", "button")
                .attr("data-action", &self.primary.0)
                .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
                .child(TemplateNode::text(&self.primary.1)),
        );

        // Caret zone.
        root = root.child(
            TemplateNode::el("lq-split-caret")
                .attr("data-part", "caret")
                .attr("role", "button")
                .attr("aria-label", "More actions")
                .attr("aria-expanded", if self.open { "true" } else { "false" })
                .pseudo_if(PseudoStateFlags::ACTIVE, self.open)
                .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
                .child(TemplateNode::text("\u{25BE}")), // ▾
        );

        if self.open {
            let mut popup = TemplateNode::el("lq-popup")
                .attr("data-part", "popup")
                .attr("role", "menu");
            for (i, (id, label)) in self.items.iter().enumerate() {
                let hot = self.highlighted == Some(i);
                let hov = self.hovered == Some(i);
                popup = popup.child(
                    TemplateNode::el("lq-menu-item")
                        .key(id)
                        .attr("data-part", &Self::item_part(i))
                        .attr("data-action", id)
                        .attr("role", "menuitem")
                        .class_if("highlighted", hot)
                        .pseudo_if(PseudoStateFlags::FOCUS, hot)
                        .pseudo_if(PseudoStateFlags::HOVER, hov)
                        .child(TemplateNode::text(label)),
                );
            }
            root = root.child(popup);
        }

        if self.disabled {
            root = root.attr("disabled", "true");
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
