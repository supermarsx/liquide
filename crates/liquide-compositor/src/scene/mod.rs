//! Scene graph node types for the compositor.
//!
//! The scene graph is a hierarchical tree of nodes, each representing a visual
//! element on the desktop. The compositor walks the tree, flattens it into a
//! z-sorted list of visible leaf nodes, and hands that list to the renderer.

mod background;
mod cursor;
mod decoration;
mod effects;
mod text;

// Re-export everything so external crates see no change.
pub use background::*;
pub use cursor::*;
pub use decoration::*;
pub use effects::*;
pub use text::*;

use std::sync::Arc;

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
    /// Per-corner border radius (top-left, top-right, bottom-right, bottom-left).
    /// Used for rounded backgrounds, gradient fills, images, and clip paths.
    #[serde(default)]
    pub corner_radius: (f32, f32, f32, f32),
    /// Per-corner clip radius for rounded overflow clipping (top-left, top-right, bottom-right, bottom-left).
    /// When set with a clip rect, creates a rounded clip mask.
    #[serde(default)]
    pub clip_radius: (f32, f32, f32, f32),
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
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
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

    /// Set corner radii (top-left, top-right, bottom-right, bottom-left).
    #[must_use]
    pub fn with_corner_radius(mut self, radius: (f32, f32, f32, f32)) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Returns `true` if any corner radius is non-zero.
    #[must_use]
    pub fn has_border_radius(&self) -> bool {
        let (tl, tr, br, bl) = self.corner_radius;
        tl > 0.5 || tr > 0.5 || br > 0.5 || bl > 0.5
    }
}

/// Image fit mode for `SceneNodeKind::Image`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFit {
    /// Scale to fill bounds, preserving aspect ratio (may crop).
    Cover,
    /// Scale to fit within bounds, preserving aspect ratio (may letterbox).
    Contain,
    /// Stretch to exactly fill bounds (may distort).
    Fill,
    /// No scaling — display at natural size.
    None,
}

/// A reference to pixel data from a Wayland client surface.
#[derive(Debug, Clone)]
pub struct SurfaceBuffer {
    /// Raw pixel data (shared via `Arc` to avoid cloning megabytes during
    /// scene flattening — cloning an `Arc` is just an atomic increment).
    pub pixels: Arc<Vec<u8>>,
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
        corner_radius: f32,
    },
    /// Server-side window decoration (title bar, borders).
    Decoration {
        title: Option<String>,
        title_color: Color,
        background: Color,
        border_color: Color,
        border_width: f32,
        corner_radius: f32,
        button_state: DecorationButtons,
        button_colors: DecorationColors,
        button_layout: DecorationLayout,
    },
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
    /// Software cursor with context-sensitive shape.
    Cursor { shape: CursorShape },
    /// Text label rendered with the font system.
    Text {
        text: String,
        color: Color,
        /// Legacy scale factor (1 = 16px base). Used when font_family is empty.
        scale: u32,
        /// Font family name (e.g. "Manrope", "Inter"). Empty = bitmap fallback.
        font_family: String,
        /// Font size in logical pixels (e.g. 14.0). 0 = use scale-based sizing.
        font_size: f32,
        /// Font weight (100–900, 400 = Regular, 700 = Bold).
        font_weight: u16,
        /// Whether the text is italic.
        font_style_italic: bool,
        /// Letter-spacing adjustment in pixels.
        letter_spacing: f32,
        /// Word-spacing adjustment in pixels.
        word_spacing: f32,
        /// Line-height in pixels.
        line_height: f32,
        /// Text alignment: 0=start/left, 1=center, 2=right/end, 3=justify.
        text_align: u8,
        /// Text transform: 0=none, 1=capitalize, 2=uppercase, 3=lowercase.
        text_transform: u8,
        /// Text overflow: 0=clip, 1=ellipsis.
        text_overflow: u8,
        /// White-space handling: 0=normal, 1=nowrap, 2=pre, 3=pre-wrap, 4=pre-line, 5=break-spaces.
        white_space: u8,
        /// Text indent in pixels (first line).
        text_indent: f32,
        /// Optional text decoration (underline/strikethrough etc.).
        text_decoration: Option<TextDecoration>,
        /// Optional text shadows.
        text_shadows: Vec<TextShadow>,
    },
    /// Built-in vector icon rendered at the node bounds.
    Icon { icon_id: u32, color: Color },
    /// Isolated render layer with custom blend mode (for compositing groups).
    RenderLayer {
        blend_mode: crate::pixel::BlendMode,
        isolate: bool,
    },
    /// Arbitrary clip path (circle, rounded rect, or polygon).
    ClipPath { clip_kind: ClipPathKind },
    /// Post-processing filter chain applied to children.
    Filter { filters: Vec<FilterSpec> },
    /// Backdrop filter chain (blur/brightness/etc. behind element).
    BackdropFilter { filters: Vec<BackdropFilterSpec> },
    /// Decoded image content (PNG, BMP, etc.).
    Image {
        image_id: u64,
        width: u32,
        height: u32,
        fit: ImageFit,
    },
    /// Gradient fill across the node bounds.
    GradientFill { gradient: GradientSpec },
    /// SVG path element with fill and stroke.
    SvgPath {
        /// SVG `d` path data string.
        d: String,
        /// Fill color (None = no fill).
        fill: Option<Color>,
        /// Stroke color.
        stroke: Color,
        /// Stroke width in pixels.
        stroke_width: f32,
    },
    /// Full background specification (color + image + gradients).
    BackgroundFill { background: BackgroundSpec },
    /// Outline (rendered outside the border box).
    Outline { outline: OutlineSpec },
    /// Multiple box shadows (CSS box-shadow, supports inset).
    BoxShadows { shadows: Vec<BoxShadowSpec> },
    /// Mask applied to children (CSS mask / mask-image).
    Mask { mask: MaskSpec },
    /// Border with per-side styling.
    Border {
        sides: BorderSides,
        radius: (f32, f32, f32, f32), // top-left, top-right, bottom-right, bottom-left
    },
    /// Border image (CSS border-image).
    BorderImage { spec: BorderImageSpec },
    /// Text input caret — a blinking vertical insertion cursor.
    ///
    /// The node bounds define the position and height of the caret.
    /// The `width` field controls the caret thickness (typically 1–2 px).
    /// Callers control blink state by including or excluding this node
    /// from the scene on each frame.
    TextCaret {
        /// Caret color.
        color: Color,
        /// Caret width in logical pixels (typically 1.0–2.0).
        width: f32,
    },
    /// Highlight overlay for element inspection / selection feedback.
    ///
    /// Draws a semi-transparent filled rectangle with an optional border,
    /// typically used to highlight hovered or selected elements in the
    /// viewport during devtools inspection.
    SelectionOverlay {
        /// Fill color (semi-transparent).
        fill: Color,
        /// Border color.
        border_color: Color,
        /// Border width.
        border_width: f32,
    },
    /// Lock screen overlay.
    LockScreen,
    /// Emergency crash overlay.
    CrashScreen,
}

impl SceneNodeKind {
    /// Extract the `white_space` value from a `Text` variant, if applicable.
    /// Returns `None` for non-text node kinds.
    pub fn text_white_space(&self) -> Option<u8> {
        match self {
            SceneNodeKind::Text { white_space, .. } => Some(*white_space),
            _ => None,
        }
    }
}

/// Guard against pathologically deep scene trees (stack overflow prevention).
const MAX_SCENE_DEPTH: u32 = 512;

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
    /// with the accumulated absolute transform and effective opacity.
    pub fn walk<F: FnMut(&SceneNode, &Affine2D, f32)>(&self, visitor: &mut F) {
        self.walk_inner(&Affine2D::identity(), 1.0, visitor, 0);
    }

    fn walk_inner<F: FnMut(&SceneNode, &Affine2D, f32)>(
        &self,
        parent_transform: &Affine2D,
        parent_opacity: f32,
        visitor: &mut F,
        depth: u32,
    ) {
        if depth >= MAX_SCENE_DEPTH {
            return;
        }

        if !self.properties.visible {
            return;
        }

        let effective_opacity = parent_opacity * self.properties.opacity;

        // Compose: translation from bounds origin + local transform
        let local = Affine2D::translation(self.properties.bounds.x, self.properties.bounds.y)
            .then(&self.properties.transform);
        let absolute = local.then(parent_transform);

        visitor(self, &absolute, effective_opacity);

        // Sort children by z-order before walking.
        // Use a stack-allocated array for small child counts to avoid
        // heap allocation on every node traversal.
        let n = self.children.len();
        if n <= 1 {
            // 0 or 1 children — no sorting needed
            for child in &self.children {
                child.walk_inner(&absolute, effective_opacity, visitor, depth + 1);
            }
        } else if n <= 16 {
            // Small child count — use stack array
            let mut indices = [0u16; 16];
            for i in 0..n {
                indices[i] = i as u16;
            }
            indices[..n].sort_by(|&a, &b| self.children[a as usize].properties.z_order.cmp(&self.children[b as usize].properties.z_order).then_with(|| self.children[a as usize].id.cmp(&self.children[b as usize].id)));
            for &i in &indices[..n] {
                self.children[i as usize].walk_inner(&absolute, effective_opacity, visitor, depth + 1);
            }
        } else {
            // Fallback to heap-allocated sort for large child counts
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by(|&a, &b| self.children[a].properties.z_order.cmp(&self.children[b].properties.z_order).then_with(|| self.children[a].id.cmp(&self.children[b].id)));
            for &i in &sorted_indices {
                self.children[i].walk_inner(&absolute, effective_opacity, visitor, depth + 1);
            }
        }
    }

    /// Find a node by ID using depth-first search.
    #[must_use]
    pub fn find(&self, id: NodeId) -> Option<&SceneNode> {
        self.find_inner(id, 0)
    }

    fn find_inner(&self, id: NodeId, depth: u32) -> Option<&SceneNode> {
        if depth >= MAX_SCENE_DEPTH {
            return None;
        }
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_inner(id, depth + 1) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node by ID (mutable) using depth-first search.
    pub fn find_mut(&mut self, id: NodeId) -> Option<&mut SceneNode> {
        self.find_mut_inner(id, 0)
    }

    fn find_mut_inner(&mut self, id: NodeId, depth: u32) -> Option<&mut SceneNode> {
        if depth >= MAX_SCENE_DEPTH {
            return None;
        }
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut_inner(id, depth + 1) {
                return Some(found);
            }
        }
        None
    }

    /// Remove a direct or nested child by ID, returning it if found.
    pub fn remove_child(&mut self, id: NodeId) -> Option<SceneNode> {
        // Check direct children first
        if let Some(pos) = self.children.iter().position(|c| c.id == id) {
            return Some(self.children.remove(pos));
        }
        // Recurse into children
        for child in &mut self.children {
            if let Some(removed) = child.remove_child(id) {
                return Some(removed);
            }
        }
        None
    }

    /// Replace a node by ID with a new node, returning the old node if found.
    pub fn replace_child(&mut self, id: NodeId, new: SceneNode) -> Option<SceneNode> {
        // Check direct children
        for child in &mut self.children {
            if child.id == id {
                let old = std::mem::replace(child, new);
                return Some(old);
            }
        }
        // Recurse
        for child in &mut self.children {
            if let Some(old) = child.replace_child(id, new.clone()) {
                return Some(old);
            }
        }
        None
    }

    /// Move a child node to new bounds.
    pub fn move_child(&mut self, id: NodeId, new_bounds: Rect) {
        if let Some(node) = self.find_mut(id) {
            node.properties.bounds = new_bounds;
        }
    }

    /// Set the opacity of a node by ID.
    pub fn set_opacity(&mut self, id: NodeId, opacity: f32) {
        if let Some(node) = self.find_mut(id) {
            node.properties.opacity = opacity;
        }
    }

    /// List all descendant node IDs (depth-first order, excludes self).
    #[must_use]
    pub fn descendants(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        self.descendants_inner(&mut result, 0);
        result
    }

    fn descendants_inner(&self, result: &mut Vec<NodeId>, depth: u32) {
        if depth >= MAX_SCENE_DEPTH {
            return;
        }
        for child in &self.children {
            result.push(child.id);
            child.descendants_inner(result, depth + 1);
        }
    }

    /// Compute the depth of the subtree (0 for a leaf, 1+ for internal nodes).
    #[must_use]
    pub fn depth(&self) -> u32 {
        self.depth_inner(0)
    }

    fn depth_inner(&self, depth: u32) -> u32 {
        if depth >= MAX_SCENE_DEPTH {
            return depth;
        }
        if self.children.is_empty() {
            return 0;
        }
        self.children
            .iter()
            .map(|c| c.depth_inner(depth + 1) + 1)
            .max()
            .unwrap_or(0)
    }

    /// Total number of descendants (recursive child count, excludes self).
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.child_count_inner(0)
    }

    fn child_count_inner(&self, depth: u32) -> usize {
        if depth >= MAX_SCENE_DEPTH {
            return 0;
        }
        let mut count = self.children.len();
        for child in &self.children {
            count += child.child_count_inner(depth + 1);
        }
        count
    }

    /// Walk the tree depth-first in z-order with mutable access,
    /// calling the visitor on each visible node.
    pub fn walk_mut<F: FnMut(&mut SceneNode)>(&mut self, visitor: &mut F) {
        self.walk_mut_inner(visitor, 0);
    }

    fn walk_mut_inner<F: FnMut(&mut SceneNode)>(&mut self, visitor: &mut F, depth: u32) {
        if depth >= MAX_SCENE_DEPTH {
            return;
        }

        if !self.properties.visible {
            return;
        }
        visitor(self);
        // Sort children indices by z-order before walking.
        let n = self.children.len();
        if n <= 1 {
            for child in &mut self.children {
                child.walk_mut_inner(visitor, depth + 1);
            }
        } else if n <= 16 {
            let mut indices = [0u16; 16];
            for i in 0..n {
                indices[i] = i as u16;
            }
            indices[..n].sort_by(|&a, &b| self.children[a as usize].properties.z_order.cmp(&self.children[b as usize].properties.z_order).then_with(|| self.children[a as usize].id.cmp(&self.children[b as usize].id)));
            for &i in &indices[..n] {
                self.children[i as usize].walk_mut_inner(visitor, depth + 1);
            }
        } else {
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by(|&a, &b| self.children[a].properties.z_order.cmp(&self.children[b].properties.z_order).then_with(|| self.children[a].id.cmp(&self.children[b].id)));
            for &i in &sorted_indices {
                self.children[i].walk_mut_inner(visitor, depth + 1);
            }
        }
    }

    /// Flatten the tree into a z-sorted list of visible leaf nodes with
    /// computed absolute bounds and transforms.
    #[must_use]
    pub fn flatten(&self) -> Vec<FlatNode> {
        let mut result = Vec::new();
        self.flatten_into(&mut result);
        result
    }

    /// Flatten the tree into `output`, reusing its allocation.
    ///
    /// Equivalent to [`flatten()`](Self::flatten) but clears and fills the
    /// caller-provided `Vec` instead of allocating a new one each frame.
    pub fn flatten_into(&self, output: &mut Vec<FlatNode>) {
        output.clear();
        self.flatten_walk(
            &Affine2D::identity(),
            1.0,
            None,
            (0.0, 0.0, 0.0, 0.0),
            output,
            0,
        );
    }

    /// Recursive helper for [`flatten_into`](Self::flatten_into) that
    /// accumulates transforms, opacity, clip rectangles and clip radii.
    fn flatten_walk(
        &self,
        parent_transform: &Affine2D,
        parent_opacity: f32,
        parent_clip: Option<Rect>,
        parent_clip_radius: (f32, f32, f32, f32),
        output: &mut Vec<FlatNode>,
        depth: u32,
    ) {
        const MAX_SCENE_DEPTH: u32 = 512;
        if depth >= MAX_SCENE_DEPTH {
            return;
        }

        if !self.properties.visible {
            return;
        }

        let effective_opacity = parent_opacity * self.properties.opacity;

        let local = Affine2D::translation(self.properties.bounds.x, self.properties.bounds.y)
            .then(&self.properties.transform);
        let abs_transform = local.then(parent_transform);

        // Accumulate clip: intersect the parent's absolute clip with this
        // node's own clip (transformed to absolute coordinates).
        let node_abs_clip = self
            .properties
            .clip
            .map(|c| abs_transform.transform_rect(c));
        let effective_clip = match (parent_clip, node_abs_clip) {
            (Some(pc), Some(nc)) => Some(
                pc.intersection(&nc)
                    .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
            ),
            (Some(pc), None) => Some(pc),
            (None, Some(nc)) => Some(nc),
            (None, None) => None,
        };

        // Accumulate clip radius: if the child has its own clip with a
        // radius, use the child's; otherwise inherit the parent's radius
        // when the parent's clip is still in effect.
        let effective_clip_radius = if self.properties.clip.is_some() {
            self.properties.clip_radius
        } else if parent_clip.is_some() {
            parent_clip_radius
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        // Emit a FlatNode for visual nodes only.
        let is_visual = !matches!(
            self.kind,
            SceneNodeKind::Root | SceneNodeKind::Workspace { .. }
        );

        if is_visual {
            let abs_bounds = abs_transform.transform_rect(Rect::new(
                0.0,
                0.0,
                self.properties.bounds.width,
                self.properties.bounds.height,
            ));

            output.push(FlatNode {
                id: self.id,
                kind: self.kind.clone(),
                absolute_bounds: abs_bounds,
                absolute_transform: abs_transform,
                clip: effective_clip,
                opacity: effective_opacity,
                z_order: self.properties.z_order,
                corner_radius: self.properties.corner_radius,
                clip_radius: effective_clip_radius,
            });
        }

        // Recurse into children sorted by z-order (same logic as walk_inner).
        let n = self.children.len();
        if n <= 1 {
            for child in &self.children {
                child.flatten_walk(&abs_transform, effective_opacity, effective_clip, effective_clip_radius, output, depth + 1);
            }
        } else if n <= 16 {
            let mut indices = [0u16; 16];
            for i in 0..n {
                indices[i] = i as u16;
            }
            indices[..n].sort_by(|&a, &b| {
                self.children[a as usize].properties.z_order.cmp(&self.children[b as usize].properties.z_order)
                    .then_with(|| self.children[a as usize].id.cmp(&self.children[b as usize].id))
            });
            for &i in &indices[..n] {
                self.children[i as usize].flatten_walk(&abs_transform, effective_opacity, effective_clip, effective_clip_radius, output, depth + 1);
            }
        } else {
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by(|&a, &b| {
                self.children[a].properties.z_order.cmp(&self.children[b].properties.z_order)
                    .then_with(|| self.children[a].id.cmp(&self.children[b].id))
            });
            for &i in &sorted_indices {
                self.children[i].flatten_walk(&abs_transform, effective_opacity, effective_clip, effective_clip_radius, output, depth + 1);
            }
        }
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
    /// Effective opacity (accumulated from ancestors).
    pub opacity: f32,
    /// Z-order within parent.
    pub z_order: u32,
    /// Per-corner border radius (top-left, top-right, bottom-right, bottom-left).
    pub corner_radius: (f32, f32, f32, f32),
    /// Per-corner clip radius for rounded overflow clipping.
    pub clip_radius: (f32, f32, f32, f32),
}
