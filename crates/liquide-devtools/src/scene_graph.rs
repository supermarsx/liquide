//! Scene graph debugger — provides a structured view of the compositor
//! scene graph with per-node information (type, bounds, z-order, etc.).

use liquide_compositor::scene::{SceneNode, SceneNodeKind};
use serde::{Deserialize, Serialize};

/// A flattened entry in the scene graph debug view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneGraphEntry {
    /// Scene node ID.
    pub id: u64,
    /// Depth in the scene tree (0 = root).
    pub depth: u32,
    /// Human-readable kind label.
    pub kind: String,
    /// Bounding rect.
    pub bounds: (f32, f32, f32, f32),
    /// Z-order.
    pub z_order: u32,
    /// Opacity.
    pub opacity: f32,
    /// Whether visible.
    pub visible: bool,
    /// Number of children.
    pub child_count: usize,
}

/// Scene graph debugger state.
pub struct SceneGraphDebugger {
    /// Flattened entries from last snapshot.
    entries: Vec<SceneGraphEntry>,
    /// Currently selected entry index.
    selected: Option<usize>,
    /// Whether to show hidden (invisible) nodes.
    show_hidden: bool,
    /// Whether to show devtools-internal nodes (IDs >= 900_000).
    show_devtools_nodes: bool,
    /// Filter by kind (empty = show all).
    kind_filter: String,
}

impl SceneGraphDebugger {
    /// Create a new scene graph debugger.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: None,
            show_hidden: false,
            show_devtools_nodes: false,
            kind_filter: String::new(),
        }
    }

    /// Build a snapshot from a scene graph root.
    pub fn snapshot(&mut self, root: &SceneNode) {
        self.entries.clear();
        self.walk(root, 0);
    }

    fn walk(&mut self, node: &SceneNode, depth: u32) {
        // Optionally skip devtools overlay nodes.
        if !self.show_devtools_nodes && node.id >= 900_000 {
            return;
        }

        let props = &node.properties;
        if !self.show_hidden && !props.visible {
            return;
        }

        let kind_label = kind_label(&node.kind);
        if !self.kind_filter.is_empty()
            && !kind_label.to_lowercase().contains(&self.kind_filter.to_lowercase())
        {
            return;
        }

        self.entries.push(SceneGraphEntry {
            id: node.id,
            depth,
            kind: kind_label,
            bounds: (
                props.bounds.x,
                props.bounds.y,
                props.bounds.width,
                props.bounds.height,
            ),
            z_order: props.z_order,
            opacity: props.opacity,
            visible: props.visible,
            child_count: node.children.len(),
        });

        for child in &node.children {
            self.walk(child, depth + 1);
        }
    }

    /// Get all entries.
    pub fn entries(&self) -> &[SceneGraphEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Select an entry by index.
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
    }

    /// Get selected entry index.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Get selected entry.
    pub fn selected_entry(&self) -> Option<&SceneGraphEntry> {
        self.selected.and_then(|i| self.entries.get(i))
    }

    /// Toggle show hidden nodes.
    pub fn toggle_show_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
    }

    /// Toggle show devtools internal nodes.
    pub fn toggle_show_devtools(&mut self) {
        self.show_devtools_nodes = !self.show_devtools_nodes;
    }

    /// Set kind filter.
    pub fn set_kind_filter(&mut self, filter: String) {
        self.kind_filter = filter;
    }

    /// Get kind filter.
    pub fn kind_filter(&self) -> &str {
        &self.kind_filter
    }

    /// Whether showing hidden.
    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Whether showing devtools nodes.
    pub fn show_devtools_nodes(&self) -> bool {
        self.show_devtools_nodes
    }
}

impl Default for SceneGraphDebugger {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a `SceneNodeKind` into a short label for display.
fn kind_label(kind: &SceneNodeKind) -> String {
    match kind {
        SceneNodeKind::Root => "Root".into(),
        SceneNodeKind::Background { .. } => "Background".into(),
        SceneNodeKind::Text { text, .. } => {
            let preview = if text.len() > 20 {
                format!("Text(\"{}…\")", &text[..20])
            } else {
                format!("Text(\"{}\")", text)
            };
            preview
        }
        SceneNodeKind::Surface { surface_id, .. } => format!("Surface({})", surface_id),
        SceneNodeKind::ChildSurface { surface_id, .. } => format!("ChildSurface({})", surface_id),
        SceneNodeKind::Shadow { .. } => "Shadow".into(),
        SceneNodeKind::Decoration { title, .. } => {
            let t = title.as_deref().unwrap_or("?");
            format!("Decoration(\"{}\")", t)
        }
        SceneNodeKind::Overlay => "Overlay".into(),
        SceneNodeKind::Glass(_) => "Glass".into(),
        SceneNodeKind::BlurBackdrop => "BlurBackdrop".into(),
        SceneNodeKind::BlurCache => "BlurCache".into(),
        SceneNodeKind::Tint { .. } => "Tint".into(),
        SceneNodeKind::Content => "Content".into(),
        SceneNodeKind::ShellLayer => "ShellLayer".into(),
        SceneNodeKind::Cursor { .. } => "Cursor".into(),
        SceneNodeKind::Icon { icon_id, .. } => format!("Icon({})", icon_id),
        SceneNodeKind::RenderLayer { .. } => "RenderLayer".into(),
        SceneNodeKind::ClipPath { .. } => "ClipPath".into(),
        SceneNodeKind::Filter { .. } => "Filter".into(),
        SceneNodeKind::BackdropFilter { .. } => "BackdropFilter".into(),
        SceneNodeKind::Image { image_id, .. } => format!("Image({})", image_id),
        SceneNodeKind::GradientFill { .. } => "GradientFill".into(),
        SceneNodeKind::SvgPath { .. } => "SvgPath".into(),
        SceneNodeKind::BackgroundFill { .. } => "BackgroundFill".into(),
        SceneNodeKind::Outline { .. } => "Outline".into(),
        SceneNodeKind::BoxShadows { .. } => "BoxShadows".into(),
        SceneNodeKind::Mask { .. } => "Mask".into(),
        SceneNodeKind::Border { .. } => "Border".into(),
        SceneNodeKind::BorderImage { .. } => "BorderImage".into(),
        SceneNodeKind::TextCaret { .. } => "TextCaret".into(),
        SceneNodeKind::SelectionOverlay { .. } => "SelectionOverlay".into(),
        SceneNodeKind::LockScreen => "LockScreen".into(),
        SceneNodeKind::CrashScreen => "CrashScreen".into(),
        SceneNodeKind::Workspace { .. } => "Workspace".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Rect;
    use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
    use liquide_compositor::pixel::Color;

    #[test]
    fn test_empty_snapshot() {
        let mut dbg = SceneGraphDebugger::new();
        let root = SceneNode::new(
            0,
            SceneNodeKind::Root,
            NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
        );
        dbg.snapshot(&root);
        assert_eq!(dbg.len(), 1);
        assert_eq!(dbg.entries()[0].kind, "Root");
    }

    #[test]
    fn test_depth_tracking() {
        let mut dbg = SceneGraphDebugger::new();
        let mut root = SceneNode::new(
            0,
            SceneNodeKind::Root,
            NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
        );
        root.add_child(SceneNode::new(
            1,
            SceneNodeKind::Background {
                color: Color::new(0, 0, 0, 255),
            },
            NodeProperties::new(Rect::new(0.0, 0.0, 50.0, 50.0)),
        ));
        dbg.snapshot(&root);
        assert_eq!(dbg.len(), 2);
        assert_eq!(dbg.entries()[0].depth, 0);
        assert_eq!(dbg.entries()[1].depth, 1);
    }

    #[test]
    fn test_devtools_filter() {
        let mut dbg = SceneGraphDebugger::new();
        let mut root = SceneNode::new(
            0,
            SceneNodeKind::Root,
            NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
        );
        root.add_child(SceneNode::new(
            920_000,
            SceneNodeKind::Background {
                color: Color::new(0, 0, 0, 255),
            },
            NodeProperties::new(Rect::new(0.0, 0.0, 50.0, 50.0)),
        ));
        dbg.snapshot(&root);
        // Devtools node filtered by default.
        assert_eq!(dbg.len(), 1);

        dbg.toggle_show_devtools();
        dbg.snapshot(&root);
        assert_eq!(dbg.len(), 2);
    }
}
