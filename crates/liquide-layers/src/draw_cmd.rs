//! LayerDrawCmd — flattened, z-ordered draw commands for final compositing.

use crate::layer::{BlendMode, LayerId, Rect, IDENTITY_TRANSFORM};
use crate::tree::LayerTree;

/// A single draw command emitted by [`flatten`] for the compositor.
///
/// Each command tells the compositor to blit one layer's pixels to the
/// output framebuffer with the given accumulated transform, opacity, and clip.
#[derive(Debug, Clone)]
pub struct LayerDrawCmd {
    /// Which layer to draw.
    pub layer_id: LayerId,
    /// Destination rectangle on screen (after accumulated transform).
    pub screen_rect: Rect,
    /// Source rectangle within the layer's pixel buffer.
    pub source_rect: Rect,
    /// Accumulated opacity (product of all ancestor opacities).
    pub opacity: f32,
    /// Accumulated affine transform from layer-local to screen space.
    pub transform: [f32; 6],
    /// Accumulated clip rectangle in screen coordinates (intersection of all
    /// ancestor clips and the viewport).
    pub clip: Option<Rect>,
    /// Compositing blend mode for this layer.
    pub blend_mode: BlendMode,
}

/// Flatten a layer tree into a z-ordered list of draw commands, clipped to
/// the given viewport.
///
/// The walk is a depth-first pre-order traversal that accumulates transforms,
/// opacity, and clips down the tree. Children within each parent are sorted
/// by `z_order` before traversal.
#[must_use]
pub fn flatten(tree: &LayerTree, viewport: Rect) -> Vec<LayerDrawCmd> {
    let mut commands = Vec::new();
    let mut stack: Vec<FlattenState> = Vec::new();

    stack.push(FlattenState {
        layer_id: tree.root,
        accumulated_transform: IDENTITY_TRANSFORM,
        accumulated_opacity: 1.0,
        accumulated_clip: Some(viewport),
    });

    while let Some(state) = stack.pop() {
        let layer = match tree.get(state.layer_id) {
            Some(l) => l,
            None => continue,
        };

        // Compose this layer's transform with the accumulated parent transform.
        let local_transform = compose_affine(&state.accumulated_transform, &layer.transform);

        // Accumulated opacity is the product of all ancestors.
        let local_opacity = state.accumulated_opacity * layer.opacity;

        // Skip fully transparent layers.
        if local_opacity < 1.0 / 512.0 {
            continue;
        }

        // Compute screen-space bounds by transforming the layer bounds.
        let screen_rect = transform_rect(&local_transform, &layer.bounds);

        // Compute accumulated clip: intersect parent clip with this layer's clip
        // (if any), both in screen space.
        let layer_clip_screen = layer.clip.map(|c| transform_rect(&local_transform, &c));
        let accumulated_clip = match (state.accumulated_clip, layer_clip_screen) {
            (Some(parent_clip), Some(layer_clip)) => parent_clip.intersection(&layer_clip),
            (Some(parent_clip), None) => Some(parent_clip),
            (None, Some(layer_clip)) => Some(layer_clip),
            (None, None) => None,
        };

        // Clip against viewport — skip layers entirely outside.
        let effective_clip = match accumulated_clip {
            Some(clip) => match clip.intersection(&viewport) {
                Some(clipped) => Some(clipped),
                None => continue, // entirely outside viewport
            },
            None => Some(viewport),
        };

        // Check if the screen rect has any visible area after clipping.
        if let Some(ref clip) = effective_clip {
            if !screen_rect.intersects(clip) {
                continue;
            }
        }

        // Emit a draw command for this layer. The compositor is responsible
        // for checking whether pixel data exists (and re-rasterizing or
        // skipping as needed).
        commands.push(LayerDrawCmd {
            layer_id: layer.id,
            screen_rect,
            source_rect: Rect::new(0.0, 0.0, layer.bounds.width, layer.bounds.height),
            opacity: local_opacity,
            transform: local_transform,
            clip: effective_clip,
            blend_mode: layer.blend_mode,
        });

        // Push children in reverse z-order so they pop in correct order.
        let mut child_ids: Vec<LayerId> = tree.children_of(state.layer_id).to_vec();
        // Sort by z_order ascending; reverse for stack push order.
        child_ids.sort_by(|a, b| {
            let za = tree.get(*a).map(|l| l.z_order).unwrap_or(0);
            let zb = tree.get(*b).map(|l| l.z_order).unwrap_or(0);
            za.cmp(&zb)
        });
        for &child_id in child_ids.iter().rev() {
            stack.push(FlattenState {
                layer_id: child_id,
                accumulated_transform: local_transform,
                accumulated_opacity: local_opacity,
                accumulated_clip: effective_clip,
            });
        }
    }

    commands
}

/// Internal state carried down the flatten traversal.
struct FlattenState {
    layer_id: LayerId,
    accumulated_transform: [f32; 6],
    accumulated_opacity: f32,
    accumulated_clip: Option<Rect>,
}

/// Compose two affine transforms: apply `parent` first, then `child`.
///
/// Affine `[a, b, c, d, tx, ty]` represents:
/// ```text
/// | a  b  tx |
/// | c  d  ty |
/// | 0  0  1  |
/// ```
fn compose_affine(parent: &[f32; 6], child: &[f32; 6]) -> [f32; 6] {
    let [pa, pb, pc, pd, ptx, pty] = *parent;
    let [ca, cb, cc, cd, ctx, cty] = *child;
    [
        ca * pa + cb * pc,       // a
        ca * pb + cb * pd,       // b
        cc * pa + cd * pc,       // c
        cc * pb + cd * pd,       // d
        ctx * pa + cty * pc + ptx, // tx
        ctx * pb + cty * pd + pty, // ty
    ]
}

/// Transform a rectangle through an affine, returning the axis-aligned
/// bounding box.
fn transform_rect(transform: &[f32; 6], r: &Rect) -> Rect {
    let [a, b, c, d, tx, ty] = *transform;
    let corners = [
        (a * r.x + b * r.y + tx, c * r.x + d * r.y + ty),
        (a * r.right() + b * r.y + tx, c * r.right() + d * r.y + ty),
        (a * r.x + b * r.bottom() + tx, c * r.x + d * r.bottom() + ty),
        (a * r.right() + b * r.bottom() + tx, c * r.right() + d * r.bottom() + ty),
    ];
    let min_x = corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let min_y = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let max_x = corners.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}
