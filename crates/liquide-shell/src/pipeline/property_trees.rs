//! Property tree construction from a pipeline output.

use liquide_compositor::property_tree::{
    ClipNode, EffectNode, PropertyTrees, ROOT_NODE_ID, RenderSurfaceReason, TransformNode,
};
use liquide_paint::DisplayItem;

use super::{DesktopPipeline, PipelineOutput};

impl DesktopPipeline {
    /// Build compositor property trees from a pipeline output.
    ///
    /// Walks the display list and the layout tree together to produce
    /// four property trees (Transform, Clip, Effect, Scroll) that model
    /// the compositing hierarchy. This is needed for:
    /// - Efficient compositor-side animations (update a transform without re-layout)
    /// - Correct render surface allocation (filters, opacity, blend modes)
    /// - Scroll-linked transforms and clip computation
    pub fn build_property_trees(&self, output: &PipelineOutput) -> PropertyTrees {
        let mut trees = PropertyTrees::new();

        // Track parent IDs as we walk the display list Push/Pop structure
        let mut transform_stack: Vec<u32> = vec![ROOT_NODE_ID];
        let mut clip_stack: Vec<u32> = vec![ROOT_NODE_ID];
        let mut effect_stack: Vec<u32> = vec![ROOT_NODE_ID];

        for item in &output.display_list.items {
            match item {
                DisplayItem::PushTransform { transform } => {
                    let parent = *transform_stack.last().unwrap_or(&ROOT_NODE_ID);
                    // Use the precomputed transform matrix directly - preserves exact
                    // CSS transform composition order and transform-origin handling
                    let node = TransformNode {
                        parent,
                        local: *transform,
                        ..Default::default()
                    };
                    let id = trees.transform_tree.insert(node);
                    transform_stack.push(id);
                }
                DisplayItem::PopTransform => {
                    if transform_stack.len() > 1 {
                        transform_stack.pop();
                    }
                }

                DisplayItem::PushClip { rect, .. } => {
                    let parent = *clip_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let transform_id = *transform_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = ClipNode {
                        parent,
                        clip_rect: liquide_compositor::geometry::Rect {
                            x: rect.x,
                            y: rect.y,
                            width: rect.width,
                            height: rect.height,
                        },
                        transform_id,
                        ..Default::default()
                    };
                    let id = trees.clip_tree.insert(node);
                    clip_stack.push(id);
                }
                DisplayItem::PopClip => {
                    if clip_stack.len() > 1 {
                        clip_stack.pop();
                    }
                }

                DisplayItem::PushOpacity { opacity } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        opacity: *opacity,
                        render_surface_reason: RenderSurfaceReason::Opacity,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopOpacity => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushBlendMode { mode } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        blend_mode: *mode,
                        render_surface_reason: RenderSurfaceReason::BlendMode,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopBlendMode => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushFilter { filters } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        filters: filters.clone(),
                        render_surface_reason: RenderSurfaceReason::Filter,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopFilter => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushBackdropFilter { filters, .. } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        backdrop_filters: filters.clone(),
                        render_surface_reason: RenderSurfaceReason::BackdropFilter,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopBackdropFilter => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushMask { .. } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        render_surface_reason: RenderSurfaceReason::Mask,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopMask => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushStackingContext {
                    z_index: _,
                    isolation,
                } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        is_isolated: *isolation
                            == liquide_style_engine::computed::Isolation::Isolate,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopStackingContext => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushClipPath { .. } => {
                    // PushClipPath pushes a clip node just like PushClip.  Both are
                    // popped by `DisplayItem::PopClip` — there is no separate
                    // `PopClipPath` variant. The painter emits them in this order:
                    //
                    //   PushClipPath  (if clip-path)
                    //   PushClip      (if overflow clip)
                    //   … children …
                    //   PopClip       (overflow — pops PushClip)
                    //   PopClip       (clip-path — pops PushClipPath)
                    //
                    // Each push gets its own pop, so the clip stack stays balanced.
                    let parent = *clip_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let transform_id = *transform_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = ClipNode {
                        parent,
                        clip_rect: liquide_compositor::geometry::Rect::new(0.0, 0.0, 0.0, 0.0),
                        transform_id,
                        ..Default::default()
                    };
                    let id = trees.clip_tree.insert(node);
                    clip_stack.push(id);
                }

                // Non-structural items don't affect the trees
                _ => {}
            }
        }

        // Compute cached values
        trees.update_transform_cache();
        trees.update_clip_cache();

        trees
    }
}
