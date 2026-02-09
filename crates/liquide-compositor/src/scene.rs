//! Scene graph node types for the compositor.
//!
//! The scene graph is a hierarchical tree of nodes, each representing a visual
//! element on the desktop. The compositor walks the tree, flattens it into a
//! z-sorted list of visible leaf nodes, and hands that list to the renderer.

use crate::geometry::{Affine2D, Rect};
use crate::pixel::{Color, PixelFormat};
use serde::{Deserialize, Serialize};

/// Unique identifier for a scene graph node.
pub type NodeId = u64;

/// Properties carried by every scene graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProperties {
    /// Bounding rectangle in parent-relative coordinates.
    pub bounds: Rect,
    /// Opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// Local transform applied before rendering.
    pub transform: Affine2D,
    /// Optional clip rectangle (in parent coordinates).
    pub clip: Option<Rect>,
    /// Whether the node is visible.
    pub visible: bool,
    /// Z-order within the parent (higher = on top).
    pub z_order: u32,
}

impl NodeProperties {
    /// Create default properties for the given bounds.
    #[must_use]
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            opacity: 1.0,
            transform: Affine2D::identity(),
            clip: None,
            visible: true,
            z_order: 0,
        }
    }

    /// Set the opacity.
    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Set the local transform.
    #[must_use]
    pub fn with_transform(mut self, transform: Affine2D) -> Self {
        self.transform = transform;
        self
    }

    /// Set the clip rectangle.
    #[must_use]
    pub fn with_clip(mut self, clip: Rect) -> Self {
        self.clip = Some(clip);
        self
    }

    /// Set the z-order.
    #[must_use]
    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z_order = z;
        self
    }

    /// Set visibility.
    #[must_use]
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

/// Glass surface parameters for the Liquid Glass effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlassParams {
    /// Blur radius in pixels for the backdrop.
    pub blur_radius: u32,
    /// Tint color applied over the blurred backdrop.
    pub tint_color: Color,
    /// Whether to draw an inner glow border.
    pub inner_glow: bool,
    /// Whether parallax is enabled (background shifts slightly on scroll).
    pub parallax: bool,
}

impl Default for GlassParams {
    fn default() -> Self {
        Self {
            blur_radius: 20,
            tint_color: Color::new(255, 255, 255, 40),
            inner_glow: true,
            parallax: false,
        }
    }
}

/// A reference to pixel data from a Wayland client surface.
#[derive(Debug, Clone)]
pub struct SurfaceBuffer {
    /// Raw pixel data.
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Bytes per row (may include padding).
    pub stride: u32,
    pub format: PixelFormat,
}

/// The type-specific payload of a scene graph node.
#[derive(Debug, Clone)]
pub enum SceneNodeKind {
    /// Root of the scene tree.
    Root,
    /// Desktop wallpaper / solid background.
    Background { color: Color },
    /// Pre-blurred wallpaper cache.
    BlurCache,
    /// A workspace container (only the active workspace is visible).
    Workspace { index: u32 },
    /// A toplevel Wayland client surface.
    Surface {
        surface_id: u64,
        buffer: Option<SurfaceBuffer>,
    },
    /// Drop shadow behind a surface.
    Shadow {
        spread: f32,
        blur_radius: f32,
        color: Color,
    },
    /// Server-side window decoration (title bar, borders).
    Decoration,
    /// Child surface (subsurface, popup).
    ChildSurface {
        surface_id: u64,
        buffer: Option<SurfaceBuffer>,
    },
    /// Transient overlay (tooltip, menu, drag-and-drop feedback).
    Overlay,
    /// Glass panel (dock, status bar, notification).
    Glass(GlassParams),
    /// Blurred backdrop region behind glass.
    BlurBackdrop,
    /// Color tint overlay for glass.
    Tint { color: Color },
    /// Content rendered on a glass surface (text, icons, widgets).
    Content,
    /// Shell layer (layer-shell surfaces).
    ShellLayer,
    /// Hardware cursor (dispatched on a separate channel).
    Cursor,
    /// Lock screen overlay.
    LockScreen,
    /// Emergency crash overlay.
    CrashScreen,
}

/// A node in the compositor's scene graph.
#[derive(Debug, Clone)]
pub struct SceneNode {
    /// Unique node identifier.
    pub id: NodeId,
    /// The type-specific payload.
    pub kind: SceneNodeKind,
    /// Common visual properties.
    pub properties: NodeProperties,
    /// Child nodes (rendered in z-order).
    pub children: Vec<SceneNode>,
}

impl SceneNode {
    /// Create a new scene node with no children.
    #[must_use]
    pub fn new(id: NodeId, kind: SceneNodeKind, properties: NodeProperties) -> Self {
        Self {
            id,
            kind,
            properties,
            children: Vec::new(),
        }
    }

    /// Append a child node.
    pub fn add_child(&mut self, child: SceneNode) {
        self.children.push(child);
    }

    /// Walk the tree depth-first in z-order, calling the visitor on each node
    /// with the accumulated absolute transform.
    pub fn walk<F: FnMut(&SceneNode, &Affine2D)>(&self, visitor: &mut F) {
        self.walk_inner(&Affine2D::identity(), visitor);
    }

    fn walk_inner<F: FnMut(&SceneNode, &Affine2D)>(
        &self,
        parent_transform: &Affine2D,
        visitor: &mut F,
    ) {
        if !self.properties.visible {
            return;
        }

        // Compose: translation from bounds origin + local transform
        let local = Affine2D::translation(self.properties.bounds.x, self.properties.bounds.y)
            .then(&self.properties.transform);
        let absolute = local.then(parent_transform);

        visitor(self, &absolute);

        // Sort children by z-order before walking
        let mut sorted_indices: Vec<usize> = (0..self.children.len()).collect();
        sorted_indices.sort_by_key(|&i| self.children[i].properties.z_order);

        for &i in &sorted_indices {
            self.children[i].walk_inner(&absolute, visitor);
        }
    }

    /// Flatten the tree into a z-sorted list of visible leaf nodes with
    /// computed absolute bounds and transforms.
    #[must_use]
    pub fn flatten(&self) -> Vec<FlatNode> {
        let mut result = Vec::new();
        self.walk(&mut |node, abs_transform| {
            // Skip non-visual structural nodes (Root, Workspace containers)
            let is_visual = !matches!(node.kind, SceneNodeKind::Root | SceneNodeKind::Workspace { .. });

            if is_visual {
                let abs_bounds = abs_transform.transform_rect(Rect::new(
                    0.0,
                    0.0,
                    node.properties.bounds.width,
                    node.properties.bounds.height,
                ));

                result.push(FlatNode {
                    id: node.id,
                    kind: node.kind.clone(),
                    absolute_bounds: abs_bounds,
                    absolute_transform: *abs_transform,
                    clip: node.properties.clip,
                    opacity: node.properties.opacity,
                    z_order: node.properties.z_order,
                });
            }
        });
        result
    }
}

/// A flattened scene node after tree walking, ready for rendering.
#[derive(Debug, Clone)]
pub struct FlatNode {
    /// The node's unique identifier.
    pub id: NodeId,
    /// The type-specific payload.
    pub kind: SceneNodeKind,
    /// Bounding rectangle in absolute (screen) coordinates.
    pub absolute_bounds: Rect,
    /// Accumulated absolute transform.
    pub absolute_transform: Affine2D,
    /// Clip rectangle in absolute coordinates (if any).
    pub clip: Option<Rect>,
    /// Effective opacity (not yet multiplied with parent).
    pub opacity: f32,
    /// Z-order within parent.
    pub z_order: u32,
}
