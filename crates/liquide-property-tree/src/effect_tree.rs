//! Hierarchical effect tree — opacity, blend modes, and filter effects.
//!
//! Each node stores local effect properties. Accumulated opacity is the product
//! of all ancestor opacities. Nodes that require isolation (non-trivial blend mode,
//! filters, or explicit isolation) need their own compositing surface.

use crate::transform_tree::{NodeId, ROOT_ID};

/// Blend modes for compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::Normal
    }
}

/// A filter operation applied to a subtree's rendered output.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOp {
    /// Gaussian blur with the given radius in pixels.
    Blur(f32),
    /// Brightness adjustment (1.0 = no change, 0.0 = black, >1.0 = brighter).
    Brightness(f32),
    /// Contrast adjustment (1.0 = no change).
    Contrast(f32),
    /// Grayscale conversion (0.0 = no change, 1.0 = fully grayscale).
    Grayscale(f32),
    /// Sepia tone (0.0 = no change, 1.0 = fully sepia).
    Sepia(f32),
    /// Saturation adjustment (1.0 = no change, 0.0 = desaturated).
    Saturate(f32),
    /// Hue rotation in degrees.
    HueRotate(f32),
    /// Inversion (0.0 = no change, 1.0 = fully inverted).
    Invert(f32),
    /// Opacity filter (0.0 = transparent, 1.0 = opaque).
    Opacity(f32),
    /// Drop shadow effect.
    DropShadow {
        dx: f32,
        dy: f32,
        blur: f32,
        color: [u8; 4],
    },
}

/// A node in the effect tree.
#[derive(Debug, Clone)]
pub struct EffectNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Parent node, or `None` for the root.
    pub parent: Option<NodeId>,
    /// Opacity at this node (0.0 = transparent, 1.0 = opaque).
    pub opacity: f32,
    /// Blend mode for compositing this node's output with the backdrop.
    pub blend_mode: BlendMode,
    /// Filter chain applied to this subtree's rendered output.
    pub filters: Vec<FilterOp>,
    /// Whether this node explicitly creates an isolated group.
    pub isolation: bool,
}

impl Default for EffectNode {
    fn default() -> Self {
        Self {
            id: ROOT_ID,
            parent: None,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            filters: Vec::new(),
            isolation: false,
        }
    }
}

/// Hierarchical tree of effect nodes with cached accumulated opacity.
pub struct EffectTree {
    nodes: Vec<EffectNode>,
    /// Cached accumulated opacity per node (product of all ancestors).
    acc_opacity: Vec<f32>,
    /// Per-node dirty flag.
    dirty: Vec<bool>,
    /// Children list per node.
    children: Vec<Vec<NodeId>>,
}

impl EffectTree {
    /// Create a new effect tree with just the root node.
    pub fn new() -> Self {
        Self {
            nodes: vec![EffectNode::default()],
            acc_opacity: vec![1.0],
            dirty: vec![false],
            children: vec![Vec::new()],
        }
    }

    /// Add a new effect node. Returns its `NodeId`.
    pub fn add(
        &mut self,
        parent: Option<NodeId>,
        opacity: f32,
        blend_mode: BlendMode,
        filters: Vec<FilterOp>,
        isolation: bool,
    ) -> NodeId {
        let id = self.nodes.len() as NodeId;
        let parent_id = match parent.unwrap_or(ROOT_ID) {
            pid if (pid as usize) < self.nodes.len() => pid,
            _ => ROOT_ID,
        };
        self.nodes.push(EffectNode {
            id,
            parent: Some(parent_id),
            opacity,
            blend_mode,
            filters,
            isolation,
        });
        self.acc_opacity.push(1.0);
        self.dirty.push(true);
        while self.children.len() <= id as usize {
            self.children.push(Vec::new());
        }
        self.children[parent_id as usize].push(id);
        id
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&EffectNode> {
        self.nodes.get(id as usize)
    }

    /// Set the opacity of a node and mark it dirty.
    pub fn set_opacity(&mut self, id: NodeId, opacity: f32) {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            node.opacity = opacity;
            self.mark_dirty(id);
        }
    }

    /// Set the blend mode of a node.
    pub fn set_blend_mode(&mut self, id: NodeId, mode: BlendMode) {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            node.blend_mode = mode;
            self.mark_dirty(id);
        }
    }

    /// Set the filter chain of a node.
    pub fn set_filters(&mut self, id: NodeId, filters: Vec<FilterOp>) {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            node.filters = filters;
            self.mark_dirty(id);
        }
    }

    /// Mark a node and all its descendants as dirty.
    pub fn mark_dirty(&mut self, id: NodeId) {
        let mut stack = vec![id];
        while let Some(nid) = stack.pop() {
            let idx = nid as usize;
            if idx < self.dirty.len() {
                self.dirty[idx] = true;
                if idx < self.children.len() {
                    for &child in &self.children[idx] {
                        stack.push(child);
                    }
                }
            }
        }
    }

    /// Recompute accumulated opacities for all dirty nodes (top-down).
    pub fn update(&mut self) {
        let len = self.nodes.len();
        for i in 0..len {
            if !self.dirty[i] {
                continue;
            }
            let parent_opacity = match self.nodes[i].parent {
                Some(pid) => self.acc_opacity[pid as usize],
                None => 1.0,
            };
            self.acc_opacity[i] = parent_opacity * self.nodes[i].opacity;
            self.dirty[i] = false;
        }
    }

    /// Get the accumulated opacity for a node.
    ///
    /// Call `update()` first to ensure dirty nodes are recomputed.
    pub fn accumulated_opacity(&self, id: NodeId) -> f32 {
        self.acc_opacity.get(id as usize).copied().unwrap_or(1.0)
    }

    /// Determine whether a node needs its own compositing surface (isolation).
    ///
    /// A node needs isolation if:
    /// - It has explicit `isolation: true`
    /// - It has a non-Normal blend mode
    /// - It has any filters
    /// - Its opacity is neither 0 nor 1 (partial opacity needs a surface)
    pub fn needs_isolation(&self, id: NodeId) -> bool {
        let node = match self.nodes.get(id as usize) {
            Some(n) => n,
            None => return false,
        };
        if node.isolation {
            return true;
        }
        if node.blend_mode != BlendMode::Normal {
            return true;
        }
        if !node.filters.is_empty() {
            return true;
        }
        let has_partial_opacity = node.opacity > 0.0 && node.opacity < 1.0;
        if has_partial_opacity {
            return true;
        }
        false
    }

    /// Number of nodes (including root).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree has only the root node.
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Whether any nodes are dirty.
    pub fn has_dirty(&self) -> bool {
        self.dirty.iter().any(|&d| d)
    }

    /// Clear all nodes except root.
    pub fn clear(&mut self) {
        self.nodes.truncate(1);
        self.acc_opacity.truncate(1);
        self.acc_opacity[0] = 1.0;
        self.dirty.truncate(1);
        self.dirty[0] = false;
        self.children.truncate(1);
        self.children[0].clear();
    }

    /// Iterate all nodes.
    pub fn iter(&self) -> impl Iterator<Item = &EffectNode> {
        self.nodes.iter()
    }
}

impl Default for EffectTree {
    fn default() -> Self {
        Self::new()
    }
}
