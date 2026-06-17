//! `<lq-tabs>` — a tab strip + panels (Group B).
//!
//! State: N (label, content) tabs and a selected index. Behavior:
//! - **Click** a tab: the tab whose LAID-OUT box (`data-part="tab-<i>"`) contains
//!   the point becomes active — hit-tested per-tab from layout, never an index
//!   computed from a constant tab width.
//! - **Left/Up** select the previous tab, **Right/Down** the next (wrapping);
//!   **Home/End** jump to first/last (keyboard a11y, when the tablist is focused).
//! - Only the **active panel** is rendered/shown (the inactive panels are not
//!   emitted, so the layout/paint only sees the visible one). The active tab
//!   carries `:checked` + `aria-selected="true"` for CSS to restyle.
//! - Emits a `Changed`(index) Action whenever the active tab changes.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action a tabs widget emits when the active tab changes.
pub const CHANGED_ACTION: &str = "changed";

/// One tab: a label shown in the strip + its panel content.
#[derive(Debug, Clone)]
struct Tab {
    label: String,
    content: Vec<TemplateNode>,
}

/// A tabbed container.
#[derive(Debug, Clone, Default)]
pub struct Tabs {
    tabs: Vec<Tab>,
    selected: usize,
    hovered: Option<usize>,
    disabled: bool,
}

impl Tabs {
    /// An empty tabs widget.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a tab with `label` and a single text panel.
    pub fn tab(mut self, label: impl Into<String>, content: &str) -> Self {
        self.tabs.push(Tab {
            label: label.into(),
            content: vec![TemplateNode::text(content)],
        });
        self
    }

    /// Append a tab with `label` and arbitrary panel subtrees.
    pub fn tab_with(
        mut self,
        label: impl Into<String>,
        content: impl IntoIterator<Item = TemplateNode>,
    ) -> Self {
        self.tabs.push(Tab {
            label: label.into(),
            content: content.into_iter().collect(),
        });
        self
    }

    /// Select an initial tab by index.
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.tabs.len() {
            self.selected = idx;
        }
        self
    }

    /// Mark the whole widget disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The active tab index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The active tab label.
    pub fn selected_label(&self) -> Option<&str> {
        self.tabs.get(self.selected).map(|t| t.label.as_str())
    }

    /// Number of tabs.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Whether there are no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    fn tab_part(i: usize) -> String {
        format!("tab-{i}")
    }

    fn set_selected(&mut self, idx: usize) -> WidgetOutcome {
        if self.disabled || idx >= self.tabs.len() || idx == self.selected {
            return WidgetOutcome::Ignored;
        }
        self.selected = idx;
        WidgetOutcome::action_with(CHANGED_ACTION, format!("{idx}"))
    }
}

impl WidgetBehavior for Tabs {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Container
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
            DomEventKind::MouseMove { x, y } | DomEventKind::Click { x, y, .. } => {
                // Which tab's LAID-OUT box contains the point? located by
                // data-part per tab — never an index over a constant tab width.
                let p = Point::new(*x, *y);
                let mut hit = None;
                for i in 0..self.tabs.len() {
                    if let Some(r) = layout.box_of_part(root, &Self::tab_part(i)) {
                        if r.contains(p) {
                            hit = Some(i);
                            break;
                        }
                    }
                }
                if matches!(event.kind, DomEventKind::Click { .. }) {
                    match hit {
                        Some(i) => self.set_selected(i),
                        None => WidgetOutcome::Ignored,
                    }
                } else if hit == self.hovered {
                    WidgetOutcome::Ignored
                } else {
                    self.hovered = hit;
                    WidgetOutcome::Changed
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
        if self.disabled || self.tabs.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let n = self.tabs.len();
        let next = match key.key {
            keys::ARROW_RIGHT | keys::ARROW_DOWN => (self.selected + 1) % n,
            keys::ARROW_LEFT | keys::ARROW_UP => (self.selected + n - 1) % n,
            keys::HOME => 0,
            keys::END => n - 1,
            _ => return WidgetOutcome::Ignored,
        };
        self.set_selected(next)
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut strip = TemplateNode::el("lq-tablist")
            .attr("role", "tablist")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for (i, tab) in self.tabs.iter().enumerate() {
            let active = i == self.selected;
            strip = strip.child(
                TemplateNode::el("lq-tab")
                    .key(&format!("tab-{i}"))
                    .attr("data-part", &Self::tab_part(i))
                    .attr("data-index", &format!("{i}"))
                    .attr("role", "tab")
                    .attr("aria-selected", if active { "true" } else { "false" })
                    .pseudo_if(PseudoStateFlags::CHECKED, active)
                    .pseudo_if(
                        PseudoStateFlags::HOVER,
                        self.hovered == Some(i) && !self.disabled,
                    )
                    .child(TemplateNode::text(&tab.label)),
            );
        }

        // Only the ACTIVE panel is emitted, so the rendered/laid-out tree shows
        // exactly one panel — the show/hide is structural, not a CSS toggle.
        let mut panel = TemplateNode::el("lq-tabpanel")
            .attr("data-part", "panel")
            .attr("role", "tabpanel");
        if let Some(active) = self.tabs.get(self.selected) {
            panel = panel.children(active.content.clone());
        }

        TemplateNode::el("lq-tabs")
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(strip)
            .child(panel)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
