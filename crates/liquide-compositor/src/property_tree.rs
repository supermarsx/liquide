//! Property trees — compositing property trees for spatial, clip, effect, and scroll hierarchies.
//!
//! Instead of a flat list of nodes, we maintain 4 independent property trees
//! (Transform, Clip, Effect, Scroll) that store compositing state separately
//! from the scene graph. Each scene node references into these trees by ID.
//!
//! This architecture enables:
//! - Efficient compositor-side animation (update a transform node without
//!   re-layout or re-paint)
//! - Shared property inheritance (many nodes share a clip or effect)
//! - Correct compositing order without sorting entire flat lists

use crate::geometry::{Affine2D, Point, Rect};
use crate::pixel::{BlendMode, Color};

/// Generic node ID in a property tree.
pub type PropertyNodeId = u32;

/// Root node ID (always 0).
pub const ROOT_NODE_ID: PropertyNodeId = 0;

// ── Transform Tree ──────────────────────────────────

/// A node in the transform property tree.
#[derive(Debug, Clone)]
pub struct TransformNode {
    /// Parent node ID.
    pub parent: PropertyNodeId,
    /// Local transform relative to parent.
    pub local: Affine2D,
    /// Translation applied after transform (for scroll offsets, etc.).
    pub post_translation: Point,
    /// Pre-computed transform from local space to root space.
    pub to_root: Affine2D,
    /// Whether inherited transforms should be flattened to 2D.
    pub flattens_inherited: bool,
    /// Whether this node has an active animation.
    pub is_animated: bool,
    /// `will-change: transform` hint.
    pub will_change: bool,
    /// Sorting context ID for 3D rendering order (0 = none).
    pub sorting_context_id: u32,
    /// Should coordinates be snapped to pixels.
    pub should_snap: bool,
}

impl Default for TransformNode {
    fn default() -> Self {
        Self {
            parent: ROOT_NODE_ID,
            local: Affine2D::identity(),
            post_translation: Point { x: 0.0, y: 0.0 },
            to_root: Affine2D::identity(),
            flattens_inherited: true,
            is_animated: false,
            will_change: false,
            sorting_context_id: 0,
            should_snap: true,
        }
    }
}

// ── Clip Tree ───────────────────────────────────────

/// A node in the clip property tree.
#[derive(Debug, Clone)]
pub struct ClipNode {
    /// Parent node ID.
    pub parent: PropertyNodeId,
    /// Clip rect in the transform space of `transform_id`.
    pub clip_rect: Rect,
    /// Which transform node this clip is expressed in.
    pub transform_id: PropertyNodeId,
    /// ID of a pixel-moving filter that might expand this clip.
    pub pixel_moving_filter_id: Option<PropertyNodeId>,
    /// Cached accumulated clip (intersection with ancestors).
    pub accumulated_clip: Option<Rect>,
}

impl Default for ClipNode {
    fn default() -> Self {
        Self {
            parent: ROOT_NODE_ID,
            clip_rect: Rect {
                x: f32::MIN / 2.0,
                y: f32::MIN / 2.0,
                width: f32::MAX,
                height: f32::MAX,
            },
            transform_id: ROOT_NODE_ID,
            pixel_moving_filter_id: None,
            accumulated_clip: None,
        }
    }
}

// ── Effect Tree ─────────────────────────────────────

/// Reason a render surface is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceReason {
    None,
    Root,
    Opacity,
    Filter,
    BackdropFilter,
    BlendMode,
    RoundedCorner,
    ClipPath,
    Mask,
    ThreeDTransformFlattening,
    Animation,
    WillChange,
    CopyRequest,
    ViewTransition,
}

/// A node in the effect property tree.
#[derive(Debug, Clone)]
pub struct EffectNode {
    /// Parent node ID.
    pub parent: PropertyNodeId,
    /// Opacity [0.0, 1.0].
    pub opacity: f32,
    /// Blend mode for compositing with backdrop.
    pub blend_mode: BlendMode,
    /// CSS filter chain applied to this subtree's output.
    pub filters: Vec<FilterOp>,
    /// Backdrop filter chain (applied to pixels behind this node).
    pub backdrop_filters: Vec<FilterOp>,
    /// Backdrop filter quality (0.0 = lowest, 1.0 = full-res).
    pub backdrop_filter_quality: f32,
    /// Corner radii for fast rounded-corner clipping.
    pub rounded_corner_bounds: Option<RoundedCornerBounds>,
    /// Why this node needs its own render surface.
    pub render_surface_reason: RenderSurfaceReason,
    /// Which transform node this effect is relative to.
    pub transform_id: PropertyNodeId,
    /// Which clip node covers this effect.
    pub clip_id: PropertyNodeId,
    /// Whether opacity is currently animating.
    pub has_opacity_animation: bool,
    /// Whether filter is currently animating.
    pub has_filter_animation: bool,
    /// Whether there's a potential backdrop-filter animation.
    pub has_backdrop_filter_animation: bool,
    /// Isolation: whether this starts a new stacking context.
    pub is_isolated: bool,
    /// Tint color overlay (for Glass panels).
    pub tint_color: Option<Color>,
}

impl Default for EffectNode {
    fn default() -> Self {
        Self {
            parent: ROOT_NODE_ID,
            opacity: 1.0,
            blend_mode: BlendMode::SrcOver,
            filters: Vec::new(),
            backdrop_filters: Vec::new(),
            backdrop_filter_quality: 1.0,
            rounded_corner_bounds: None,
            render_surface_reason: RenderSurfaceReason::None,
            transform_id: ROOT_NODE_ID,
            clip_id: ROOT_NODE_ID,
            has_opacity_animation: false,
            has_filter_animation: false,
            has_backdrop_filter_animation: false,
            is_isolated: false,
            tint_color: None,
        }
    }
}

/// Rounded corner bounds for fast GPU/CPU clipping.
#[derive(Debug, Clone, Copy)]
pub struct RoundedCornerBounds {
    pub rect: Rect,
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

/// Compositor-level filter operation types.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOp {
    Blur(f32),
    Brightness(f32),
    Contrast(f32),
    Grayscale(f32),
    Sepia(f32),
    Saturate(f32),
    HueRotate(f32),
    Invert(f32),
    Opacity(f32),
    DropShadow {
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        color: Color,
    },
    ColorMatrix([f32; 20]),
    Reference(String),
}

// ── Scroll Tree ─────────────────────────────────────

/// A node in the scroll property tree.
#[derive(Debug, Clone)]
pub struct ScrollNode {
    /// Parent node ID.
    pub parent: PropertyNodeId,
    /// Size of the visible viewport.
    pub container_bounds: (f32, f32),
    /// Total scrollable area size.
    pub scroll_bounds: (f32, f32),
    /// Current scroll offset.
    pub scroll_offset: Point,
    /// Whether the user can scroll horizontally.
    pub user_scrollable_x: bool,
    /// Whether the user can scroll vertically.
    pub user_scrollable_y: bool,
    /// Which transform node scroll offsets are relative to.
    pub transform_id: PropertyNodeId,
    /// Overscroll behavior.
    pub overscroll_behavior_x: OverscrollBehavior,
    pub overscroll_behavior_y: OverscrollBehavior,
}

/// Overscroll boundary behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverscrollBehavior {
    Auto,
    Contain,
    None,
}

impl Default for ScrollNode {
    fn default() -> Self {
        Self {
            parent: ROOT_NODE_ID,
            container_bounds: (0.0, 0.0),
            scroll_bounds: (0.0, 0.0),
            scroll_offset: Point { x: 0.0, y: 0.0 },
            user_scrollable_x: false,
            user_scrollable_y: false,
            transform_id: ROOT_NODE_ID,
            overscroll_behavior_x: OverscrollBehavior::Auto,
            overscroll_behavior_y: OverscrollBehavior::Auto,
        }
    }
}

// ── Property Tree Container ─────────────────────────

/// A single property tree — stores nodes in a flat Vec indexed by ID.
#[derive(Debug, Clone)]
pub struct PropertyTree<T> {
    nodes: Vec<T>,
}

impl<T: Default> PropertyTree<T> {
    /// Create a new tree with just the root node.
    pub fn new() -> Self {
        let mut nodes = Vec::new();
        nodes.push(T::default()); // root node at index 0
        Self { nodes }
    }

    /// Insert a new node, returning its ID.
    pub fn insert(&mut self, node: T) -> PropertyNodeId {
        let id = self.nodes.len() as PropertyNodeId;
        self.nodes.push(node);
        id
    }

    /// Get a node by ID.
    pub fn get(&self, id: PropertyNodeId) -> Option<&T> {
        self.nodes.get(id as usize)
    }

    /// Get a mutable reference to a node.
    pub fn get_mut(&mut self, id: PropertyNodeId) -> Option<&mut T> {
        self.nodes.get_mut(id as usize)
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Clear all nodes except root.
    pub fn clear(&mut self) {
        self.nodes.truncate(1);
    }

    /// Iterate all nodes with their IDs.
    pub fn iter(&self) -> impl Iterator<Item = (PropertyNodeId, &T)> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (i as PropertyNodeId, n))
    }
}

impl<T: Default> Default for PropertyTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Combined Property Trees ─────────────────────────

/// All four property trees combined — the compositor's understanding of
/// spatial, clipping, visual effect, and scroll relationships.
#[derive(Debug, Clone, Default)]
pub struct PropertyTrees {
    pub transform_tree: PropertyTree<TransformNode>,
    pub clip_tree: PropertyTree<ClipNode>,
    pub effect_tree: PropertyTree<EffectNode>,
    pub scroll_tree: PropertyTree<ScrollNode>,
}

impl PropertyTrees {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all trees for re-build.
    pub fn clear(&mut self) {
        self.transform_tree.clear();
        self.clip_tree.clear();
        self.effect_tree.clear();
        self.scroll_tree.clear();
    }

    /// Total node count across all trees.
    pub fn total_nodes(&self) -> usize {
        self.transform_tree.len()
            + self.clip_tree.len()
            + self.effect_tree.len()
            + self.scroll_tree.len()
    }

    /// Update cached transform-to-root on all transform nodes.
    pub fn update_transform_cache(&mut self) {
        let len = self.transform_tree.len();
        for i in 0..len {
            let id = i as PropertyNodeId;
            let parent_id = self.transform_tree.nodes[i].parent;
            if id == ROOT_NODE_ID {
                self.transform_tree.nodes[i].to_root = self.transform_tree.nodes[i].local;
            } else if let Some(parent) = self.transform_tree.nodes.get(parent_id as usize) {
                let parent_to_root = parent.to_root;
                let node = &mut self.transform_tree.nodes[i];
                // local→root maps a node-local point to root space by applying
                // the node's own `local` transform first, then the parent's
                // accumulated `to_root`. `Affine2D::then` is "apply self first,
                // then other", so this must be `local.then(&parent_to_root)`
                // (matching the scene walker's `local.then(parent_transform)`),
                // NOT the reverse.
                node.to_root = node.local.then(&parent_to_root);
            }
        }
    }

    /// Update cached accumulated clips.
    pub fn update_clip_cache(&mut self) {
        let len = self.clip_tree.len();
        for i in 0..len {
            let id = i as PropertyNodeId;
            let parent_id = self.clip_tree.nodes[i].parent;
            if id == ROOT_NODE_ID {
                self.clip_tree.nodes[i].accumulated_clip = Some(self.clip_tree.nodes[i].clip_rect);
            } else if let Some(parent) = self.clip_tree.nodes.get(parent_id as usize) {
                let parent_acc = parent.accumulated_clip.unwrap_or(parent.clip_rect);
                let my_clip = self.clip_tree.nodes[i].clip_rect;
                self.clip_tree.nodes[i].accumulated_clip =
                    Some(intersect_rects(parent_acc, my_clip));
            }
        }
    }
}

/// Intersect two rectangles.
fn intersect_rects(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    Rect {
        x: x1,
        y: y1,
        width: (x2 - x1).max(0.0),
        height: (y2 - y1).max(0.0),
    }
}

/// A scene node that references into property trees instead of storing
/// properties inline. This replaces the old flat FlatNode for compositing.
#[derive(Debug, Clone)]
pub struct CompositorNode {
    /// Unique node ID.
    pub id: u64,
    /// Content bounds in local space.
    pub bounds: Rect,
    /// Index into the transform tree.
    pub transform_id: PropertyNodeId,
    /// Index into the clip tree.
    pub clip_id: PropertyNodeId,
    /// Index into the effect tree.
    pub effect_id: PropertyNodeId,
    /// Index into the scroll tree (0 = root/no scroll).
    pub scroll_id: PropertyNodeId,
    /// Z-order within the parent stacking context.
    pub z_order: i32,
    /// The actual visual content.
    pub content: CompositorContent,
}

/// What a compositor node renders.
#[derive(Debug, Clone)]
pub enum CompositorContent {
    /// Nothing — pure container.
    Container,
    /// Solid color fill.
    SolidColor(Color),
    /// Glass panel (backdrop blur + tint).
    Glass {
        blur_radius: f32,
        tint: Color,
        inner_glow: bool,
    },
    /// Client window surface.
    Surface { surface_id: u64 },
    /// Text content.
    Text {
        text: String,
        color: Color,
        font_size: f32,
        font_family: String,
        font_weight: u16,
    },
    /// Icon.
    Icon { icon_id: u32, color: Color },
    /// Image.
    Image { image_id: String },
    /// Shadow.
    Shadow {
        blur_radius: f32,
        spread: f32,
        color: Color,
    },
    /// Border.
    Border {
        widths: (f32, f32, f32, f32),
        colors: (Color, Color, Color, Color),
        radii: (f32, f32, f32, f32),
    },
    /// Window decoration (title bar).
    Decoration { title: String },
    /// Display list paint chunk (recorded paint ops for a subtree).
    PaintChunk { display_list_range: (usize, usize) },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_tree_basics() {
        let mut tree = PropertyTree::<TransformNode>::new();
        assert_eq!(tree.len(), 1); // root

        let child = TransformNode {
            parent: ROOT_NODE_ID,
            local: Affine2D::identity(),
            ..Default::default()
        };
        let id = tree.insert(child);
        assert_eq!(id, 1);
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn property_trees_combined() {
        let trees = PropertyTrees::new();
        assert_eq!(trees.total_nodes(), 4); // 1 root per tree × 4 trees
    }

    // Regression for t49-e1-F2: the world-transform cache must compose
    // `local.then(&parent_to_root)` (local applied first, then the parent's
    // accumulated transform), matching the scene walker. The previous code
    // used the reversed order, which only happened to pass because pure
    // translations commute. Use a scale+translate parent so the reversed
    // composition gives an observably different result, and assert a known
    // node-local point maps to the correct root-space coordinate.
    #[test]
    fn transform_cache_composes_parent_then_local_not_reversed() {
        let mut trees = PropertyTrees::new();

        // Parent: scale by (2,3) first, then translate by (10,20).
        // For a point p this maps to (2*px + 10, 3*py + 20).
        let parent_local = Affine2D::scale(2.0, 3.0).then(&Affine2D::translation(10.0, 20.0));
        let parent_id = trees.transform_tree.insert(TransformNode {
            parent: ROOT_NODE_ID,
            local: parent_local,
            ..Default::default()
        });

        // Child: translate by (5,7), under the parent.
        let child_local = Affine2D::translation(5.0, 7.0);
        let child_id = trees.transform_tree.insert(TransformNode {
            parent: parent_id,
            local: child_local,
            ..Default::default()
        });

        trees.update_transform_cache();

        let child = trees.transform_tree.get(child_id).unwrap();
        // For node-local point (1,1):
        //   child translate -> (6, 8)
        //   parent scale+translate -> (2*6 + 10, 3*8 + 20) = (22, 44)
        let mapped = child.to_root.transform_point(Point { x: 1.0, y: 1.0 });
        assert!(
            (mapped.x - 22.0).abs() < 1e-4 && (mapped.y - 44.0).abs() < 1e-4,
            "child world transform mapped (1,1) to ({}, {}), expected (22, 44)",
            mapped.x,
            mapped.y
        );

        // Cross-check: the cached world transform must equal the same
        // composition the scene walker would produce.
        let expected = child_local.then(&parent_local);
        let e = expected.transform_point(Point { x: 1.0, y: 1.0 });
        assert!((mapped.x - e.x).abs() < 1e-4 && (mapped.y - e.y).abs() < 1e-4);
    }

    #[test]
    fn effect_node_render_surface() {
        let mut trees = PropertyTrees::new();
        let effect = EffectNode {
            opacity: 0.5,
            render_surface_reason: RenderSurfaceReason::Opacity,
            ..Default::default()
        };
        let id = trees.effect_tree.insert(effect);
        assert_eq!(
            trees.effect_tree.get(id).unwrap().render_surface_reason,
            RenderSurfaceReason::Opacity
        );
    }

    #[test]
    fn clip_tree_intersection() {
        let mut trees = PropertyTrees::new();
        // Root clip at (0,0)-(100,100)
        trees.clip_tree.nodes[0].clip_rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        // Child clip at (50,50)-(150,150)
        let child = ClipNode {
            parent: ROOT_NODE_ID,
            clip_rect: Rect {
                x: 50.0,
                y: 50.0,
                width: 100.0,
                height: 100.0,
            },
            ..Default::default()
        };
        let child_id = trees.clip_tree.insert(child);
        trees.update_clip_cache();

        let acc = trees
            .clip_tree
            .get(child_id)
            .unwrap()
            .accumulated_clip
            .unwrap();
        assert!((acc.x - 50.0).abs() < 0.001);
        assert!((acc.y - 50.0).abs() < 0.001);
        assert!((acc.width - 50.0).abs() < 0.001);
        assert!((acc.height - 50.0).abs() < 0.001);
    }

    #[test]
    fn filter_op_variants() {
        let filters = vec![
            FilterOp::Blur(10.0),
            FilterOp::Brightness(1.2),
            FilterOp::Contrast(0.8),
            FilterOp::Grayscale(1.0),
            FilterOp::Sepia(0.5),
            FilterOp::Saturate(1.5),
            FilterOp::HueRotate(90.0),
            FilterOp::Invert(1.0),
            FilterOp::Opacity(0.5),
            FilterOp::DropShadow {
                offset_x: 2.0,
                offset_y: 4.0,
                blur_radius: 8.0,
                color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 128,
                },
            },
        ];
        assert_eq!(filters.len(), 10);
    }
}
