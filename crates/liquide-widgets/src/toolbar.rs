//! `<lq-toolbar>` — a horizontal/vertical button container (Group B).
//!
//! A toolbar is a flex row (or column) that groups buttons / separators with
//! consistent spacing and handles overflow. It is a **container**: it lays out
//! and styles its items, and delegates interaction to the child buttons (each a
//! [`Button`](crate::button::Button) the owner mounts and wires separately, or a
//! rendered button subtree slotted directly).
//!
//! - Orientation is chosen by [`ToolbarOrientation`] -> a CSS class
//!   (`horizontal` / `vertical`) the theme styles (flex-direction + spacing).
//! - [`separator`](Toolbar::separator) inserts an `<lq-toolbar-sep>` item the CSS
//!   renders as a divider; [`spacer`](Toolbar::spacer) inserts a flexible gap
//!   that pushes following items to the far end.
//! - Items are grouped: consecutive non-separator items form a visual group; the
//!   separators delimit groups. Overflow is handled by the CSS (`overflow`/wrap),
//!   not by truncating the item list.

use liquide_components::template::TemplateNode;
use liquide_dom::NodeId;
use liquide_hit_test::event::{DomEvent, DomEventKind};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::layout_query::LayoutQuery;

/// Toolbar layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolbarOrientation {
    /// A horizontal row of items (the default).
    #[default]
    Horizontal,
    /// A vertical column of items.
    Vertical,
}

impl ToolbarOrientation {
    fn class(self) -> &'static str {
        match self {
            ToolbarOrientation::Horizontal => "horizontal",
            ToolbarOrientation::Vertical => "vertical",
        }
    }
}

/// One toolbar item.
#[derive(Debug, Clone)]
enum Item {
    /// A slotted widget subtree (typically a button).
    Node(TemplateNode),
    /// A divider between groups.
    Separator,
    /// A flexible gap that pushes following items to the far end.
    Spacer,
}

/// A toolbar grouping buttons / separators with consistent spacing.
#[derive(Debug, Clone, Default)]
pub struct Toolbar {
    orientation: ToolbarOrientation,
    items: Vec<Item>,
}

impl Toolbar {
    /// An empty horizontal toolbar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the orientation.
    pub fn orientation(mut self, o: ToolbarOrientation) -> Self {
        self.orientation = o;
        self
    }

    /// A vertical toolbar (shorthand).
    pub fn vertical(mut self) -> Self {
        self.orientation = ToolbarOrientation::Vertical;
        self
    }

    /// Slot an item subtree (usually a button's rendered template).
    pub fn item(mut self, node: TemplateNode) -> Self {
        self.items.push(Item::Node(node));
        self
    }

    /// Insert a separator (group divider).
    pub fn separator(mut self) -> Self {
        self.items.push(Item::Separator);
        self
    }

    /// Insert a flexible spacer (pushes following items to the far end).
    pub fn spacer(mut self) -> Self {
        self.items.push(Item::Spacer);
        self
    }

    /// The number of laid-out items (including separators / spacers).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the toolbar has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The number of separator items.
    pub fn separator_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| matches!(i, Item::Separator))
            .count()
    }
}

impl WidgetBehavior for Toolbar {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Container
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        // Interaction is delegated to the child buttons (mounted separately); the
        // toolbar itself wants nothing.
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
        let mut bar = TemplateNode::el("lq-toolbar")
            .attr("role", "toolbar")
            .attr("data-orientation", self.orientation.class())
            .class(self.orientation.class());

        for item in &self.items {
            bar = match item {
                Item::Node(n) => bar.child(n.clone()),
                Item::Separator => bar.child(
                    TemplateNode::el("lq-toolbar-sep")
                        .attr("data-part", "separator")
                        .attr("role", "separator"),
                ),
                Item::Spacer => bar.child(
                    TemplateNode::el("lq-toolbar-spacer").attr("data-part", "spacer"),
                ),
            };
        }
        bar
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
