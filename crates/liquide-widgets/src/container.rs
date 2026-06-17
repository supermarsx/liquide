//! `<lq-panel>` / `<lq-card>` / `<lq-group-box>` — styled containers (Group B).
//!
//! These are **static** containers: they have no own interaction beyond rendering
//! their children, so their [`WidgetBehavior`] wants no events and is not
//! focusable. They exist as widgets (rather than raw `<div>`s) so they get a
//! stable `<lq-*>` tag the theme styles, a header/body/footer structure for
//! cards, and a labelled bordered region for group boxes — all CSS-themeable.
//!
//! - [`Panel`] (`<lq-panel>`) — the simplest styled box (bg / border / radius /
//!   shadow / padding) that slots arbitrary children.
//! - [`Card`] (`<lq-card>`) — a panel with an explicit header / body / footer
//!   three-region structure (`data-part="header"/"body"/"footer"`), each
//!   optional. The body holds the slotted children.
//! - [`GroupBox`] (`<lq-group-box>`) — a labelled bordered container: a caption
//!   (`data-part="caption"`) sitting on the border + a bordered content region
//!   (`data-part="content"`) holding the children. The fieldset/legend idiom.
//!
//! Children are supplied as raw [`TemplateNode`]s (typically other widgets'
//! rendered subtrees, or text). The container reuses the same
//! `Component`/`TemplateNode` substrate as every other widget, so it composes in
//! the gallery and the real DOM identically.

use liquide_components::template::TemplateNode;
use liquide_dom::NodeId;
use liquide_hit_test::event::{DomEvent, DomEventKind};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::layout_query::LayoutQuery;

/// Clone the static child subtrees a container renders.
fn clone_children(children: &[TemplateNode]) -> Vec<TemplateNode> {
    children.to_vec()
}

/// A simple styled container box that slots arbitrary children.
#[derive(Debug, Clone, Default)]
pub struct Panel {
    /// Extra CSS class (e.g. an `elevated` / `flush` variant) or empty.
    variant: String,
    /// Slotted child subtrees.
    children: Vec<TemplateNode>,
}

impl Panel {
    /// An empty panel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a CSS variant class.
    pub fn variant(mut self, v: impl Into<String>) -> Self {
        self.variant = v.into();
        self
    }

    /// Slot a child subtree.
    pub fn child(mut self, child: TemplateNode) -> Self {
        self.children.push(child);
        self
    }

    /// Slot a plain-text child.
    pub fn text(self, text: &str) -> Self {
        self.child(TemplateNode::text(text))
    }
}

impl WidgetBehavior for Panel {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Container
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
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
        TemplateNode::el("lq-panel")
            .class_if(&self.variant, !self.variant.is_empty())
            .children(clone_children(&self.children))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A card: a panel with an explicit header / body / footer structure. Each region
/// is optional; the body holds the slotted children.
#[derive(Debug, Clone, Default)]
pub struct Card {
    header: Option<Vec<TemplateNode>>,
    body: Vec<TemplateNode>,
    footer: Option<Vec<TemplateNode>>,
    variant: String,
}

impl Card {
    /// An empty card.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a CSS variant class.
    pub fn variant(mut self, v: impl Into<String>) -> Self {
        self.variant = v.into();
        self
    }

    /// Set the header to a single text title.
    pub fn header_text(mut self, title: &str) -> Self {
        self.header = Some(vec![TemplateNode::text(title)]);
        self
    }

    /// Set the header to arbitrary subtrees.
    pub fn header(mut self, nodes: impl IntoIterator<Item = TemplateNode>) -> Self {
        self.header = Some(nodes.into_iter().collect());
        self
    }

    /// Slot a child into the body.
    pub fn child(mut self, child: TemplateNode) -> Self {
        self.body.push(child);
        self
    }

    /// Slot a plain-text child into the body.
    pub fn text(self, text: &str) -> Self {
        self.child(TemplateNode::text(text))
    }

    /// Set the footer to a single text line.
    pub fn footer_text(mut self, text: &str) -> Self {
        self.footer = Some(vec![TemplateNode::text(text)]);
        self
    }

    /// Set the footer to arbitrary subtrees.
    pub fn footer(mut self, nodes: impl IntoIterator<Item = TemplateNode>) -> Self {
        self.footer = Some(nodes.into_iter().collect());
        self
    }

    /// Whether the card has a header region.
    pub fn has_header(&self) -> bool {
        self.header.is_some()
    }

    /// Whether the card has a footer region.
    pub fn has_footer(&self) -> bool {
        self.footer.is_some()
    }
}

impl WidgetBehavior for Card {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Container
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
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
        let mut card = TemplateNode::el("lq-card")
            .class_if(&self.variant, !self.variant.is_empty());

        if let Some(header) = &self.header {
            card = card.child(
                TemplateNode::el("lq-card-header")
                    .attr("data-part", "header")
                    .children(clone_children(header)),
            );
        }
        card = card.child(
            TemplateNode::el("lq-card-body")
                .attr("data-part", "body")
                .children(clone_children(&self.body)),
        );
        if let Some(footer) = &self.footer {
            card = card.child(
                TemplateNode::el("lq-card-footer")
                    .attr("data-part", "footer")
                    .children(clone_children(footer)),
            );
        }
        card
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A group box: a labelled bordered container (the fieldset/legend idiom). A
/// caption sits on the top border and a bordered content region holds children.
#[derive(Debug, Clone, Default)]
pub struct GroupBox {
    caption: String,
    children: Vec<TemplateNode>,
}

impl GroupBox {
    /// A group box captioned `caption`.
    pub fn new(caption: impl Into<String>) -> Self {
        Self {
            caption: caption.into(),
            children: Vec::new(),
        }
    }

    /// The caption text.
    pub fn caption(&self) -> &str {
        &self.caption
    }

    /// Slot a child subtree into the content region.
    pub fn child(mut self, child: TemplateNode) -> Self {
        self.children.push(child);
        self
    }

    /// Slot a plain-text child into the content region.
    pub fn text(self, text: &str) -> Self {
        self.child(TemplateNode::text(text))
    }
}

impl WidgetBehavior for GroupBox {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Container
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
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
        TemplateNode::el("lq-group-box")
            .attr("role", "group")
            .child(
                TemplateNode::el("lq-caption")
                    .attr("data-part", "caption")
                    .child(TemplateNode::text(&self.caption)),
            )
            .child(
                TemplateNode::el("lq-group-content")
                    .attr("data-part", "content")
                    .children(clone_children(&self.children)),
            )
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
