//! Front-to-back occlusion culling for the CPU raster path (t137, t90 lever #5).
//!
//! The renderer paints nodes back-to-front (array index 0 = bottom, last =
//! top). A node whose every painted pixel is guaranteed to be over-painted by a
//! later **fully-opaque** node is invisible in the final frame: the opaque cover
//! writes the final value at every shared pixel regardless of what was beneath.
//! Skipping such a node is therefore **byte-identical** to painting it, while
//! saving the (dominant) raster bandwidth of filling pixels that are immediately
//! discarded — e.g. the wallpaper / a large panel beneath an opaque window.
//!
//! # The conservative opaque-occluder rule (a node may CULL only if)
//!
//! - It is a plain **solid-color rectangle fill**: `Background { color }` with
//!   `color.a == 255`, or `BackgroundFill { background }` whose `color` is an
//!   opaque (`a == 255`) solid **with NO image layer** (an image may have
//!   transparent texels or not cover the whole box).
//! - `opacity >= 1.0` (a semi-transparent node samples what is beneath).
//! - **No rounded corners** (a rounded fill leaves the corners uncovered).
//! - Its **blend mode at paint time is `SrcOver`** (a `Multiply`/etc. layer
//!   composites with the backdrop instead of replacing it). The active blend
//!   mode is driven by `RenderLayer` nodes during the walk, so this module
//!   replays that state forward, exactly like the paint loop.
//! - If the node carries a `clip`, only the `bounds ∩ clip` region is actually
//!   painted, so that intersection (not the full bounds) is used as the
//!   occluder rect.
//!
//! Anything that samples or only partially covers its box — `Glass`,
//! `BlurBackdrop`/`BlurCache`, `Filter`/`BackdropFilter`, `Tint`, `Mask`,
//! `GradientFill`, `Image`, `Surface` (per-pixel alpha unknown), `SvgPath`,
//! `Border`, `Shadow`, `Outline`, text, rounded/semi-transparent fills — is
//! **never** an occluder.
//!
//! # The cull rule (a node IS culled only if)
//!
//! Its painted rect (bounds confined to the active raster clip — outside the
//! clip nothing is written anyway) is **entirely** contained in the union of
//! the opaque occluder rects of strictly-later nodes. Partial coverage → still
//! painted (clip-culling partial coverage is `raster_clip`'s job, not this).

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{FlatNode, NodeId, SceneNodeKind};

/// Sub-pixel tolerance for treating a corner radius as "square".
const RADIUS_EPS: f32 = 0.5;

/// Returns the rect a node is **guaranteed to fully paint opaque** (its
/// occluder rect), or `None` if the node is not a safe opaque occluder under the
/// conservative rule documented at the module level.
///
/// `blend` is the active blend mode at the moment this node would paint.
fn opaque_occluder_rect(node: &FlatNode, blend: BlendMode) -> Option<Rect> {
    // A non-SrcOver blend composites with the backdrop; it does not replace it.
    if blend != BlendMode::SrcOver {
        return None;
    }
    // Accumulated opacity below 1.0 means the node samples what is beneath.
    if node.opacity < 1.0 {
        return None;
    }
    // Rounded corners leave the corners of the box uncovered.
    let (r_tl, r_tr, r_br, r_bl) = node.corner_radius;
    if r_tl > RADIUS_EPS || r_tr > RADIUS_EPS || r_br > RADIUS_EPS || r_bl > RADIUS_EPS {
        return None;
    }

    let is_opaque_solid = match node.kind_ref() {
        // Solid wallpaper / fill: opaque iff alpha == 255 (the paint path uses
        // `BlendMode::Src` only in exactly this case).
        SceneNodeKind::Background { color } => color.a == 255,
        // Full background spec is a safe occluder only when it is a pure opaque
        // solid color with NO image layer (an image may be transparent or not
        // tile-cover the whole box).
        SceneNodeKind::BackgroundFill { background } => {
            background.image.is_none()
                && matches!(background.color, Some(Color { a: 255, .. }))
        }
        _ => false,
    };
    if !is_opaque_solid {
        return None;
    }

    // The node only paints within `bounds ∩ clip`; outside the clip nothing is
    // written, so the guaranteed-opaque region is that intersection.
    match node.clip {
        None => Some(node.absolute_bounds),
        Some(clip) => node.absolute_bounds.intersection(&clip),
    }
}

/// Compute, for every node, its opaque occluder rect (or `None`), replaying the
/// `RenderLayer`-driven blend-mode state forward exactly like the paint walk so
/// each node's occluder eligibility is judged with the blend mode it paints
/// under. Index `i` of the result corresponds to `nodes[i]`.
pub(super) fn occluder_rects(nodes: &[FlatNode]) -> Vec<Option<Rect>> {
    let mut out = Vec::with_capacity(nodes.len());
    // Mirrors `render_with_mode`, which resets the active blend mode to SrcOver
    // at the start of every frame before walking the nodes.
    let mut active_blend = BlendMode::SrcOver;
    for node in nodes {
        // A RenderLayer node sets the blend mode for SUBSEQUENT nodes (it paints
        // nothing itself), so update state AFTER deciding it is not an occluder.
        if let SceneNodeKind::RenderLayer { blend_mode, .. } = node.kind_ref() {
            active_blend = *blend_mode;
            out.push(None);
            continue;
        }
        out.push(opaque_occluder_rect(node, active_blend));
    }
    out
}

/// The rect a node would actually paint this frame, confined to the active
/// raster clip, **iff** the node is a kind that is safe to skip wholesale when
/// fully covered. Returns `None` for nodes that must always run (state-mutating
/// `RenderLayer`, structural containers, or kinds whose cull is not worth the
/// test).
///
/// Only *pure bounded paint* kinds are eligible to be culled. Crucially we do
/// NOT cull `RenderLayer` (it sets blend state for later nodes) — skipping it
/// would corrupt the blend mode of everything after it.
pub(super) fn cullable_paint_rect(node: &FlatNode, raster_clip: Option<Rect>) -> Option<Rect> {
    let eligible = matches!(
        node.kind_ref(),
        SceneNodeKind::Background { .. }
            | SceneNodeKind::BackgroundFill { .. }
            | SceneNodeKind::GradientFill { .. }
            | SceneNodeKind::Image { .. }
            | SceneNodeKind::Surface { .. }
            | SceneNodeKind::ChildSurface { .. }
            | SceneNodeKind::Tint { .. }
            | SceneNodeKind::Glass(_)
            | SceneNodeKind::Border { .. }
            | SceneNodeKind::BoxShadows { .. }
            | SceneNodeKind::Icon { .. }
            | SceneNodeKind::SvgPath { .. }
            | SceneNodeKind::BorderImage { .. }
            | SceneNodeKind::Text { .. }
    );
    if !eligible {
        return None;
    }

    let bounds = node.absolute_bounds;
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return None;
    }
    // The painted region never exceeds the node bounds confined to the active
    // raster clip (a partial-damage frame). Testing this confined rect for full
    // coverage is conservative: any pixel outside the clip is not written, and
    // any pixel inside must be covered for the skip to be byte-identical.
    let test = match raster_clip {
        None => bounds,
        Some(clip) => bounds.intersection(&clip)?,
    };
    if test.width <= 0.0 || test.height <= 0.0 {
        None
    } else {
        Some(test)
    }
}

/// True iff `rect` is **entirely** covered by the union of occluder rects at
/// indices strictly greater than `index`.
///
/// Implemented by rectangle subtraction: starting from `[rect]`, subtract each
/// later occluder; if the remaining-uncovered list ever empties, the rect is
/// fully covered. This is exact for axis-aligned rects and handles the
/// union-of-several case (not just a single covering rect), while staying
/// conservative — sub-pixel gaps never collapse, so a near-cover never culls.
pub(super) fn is_fully_covered_by_later(
    rect: Rect,
    index: usize,
    occluders: &[Option<Rect>],
) -> bool {
    // Collect the later occluders that actually overlap `rect`.
    let mut covers: Vec<Rect> = Vec::new();
    for occ in occluders.iter().skip(index + 1).flatten() {
        if occ.intersects(&rect) {
            covers.push(*occ);
        }
    }
    if covers.is_empty() {
        return false;
    }

    // Fast path: a single occluder fully contains the rect.
    for c in &covers {
        if rect_contains(c, &rect) {
            return true;
        }
    }

    // General path: subtract each occluder from the set of uncovered fragments.
    // Bound the work so a pathological scene can never blow up the fragment list
    // (in which case we conservatively decline to cull — never a wrong cull).
    const MAX_FRAGMENTS: usize = 256;
    let mut uncovered: Vec<Rect> = vec![rect];
    for c in &covers {
        let mut next: Vec<Rect> = Vec::new();
        for frag in &uncovered {
            subtract_rect(frag, c, &mut next);
            if next.len() > MAX_FRAGMENTS {
                return false;
            }
        }
        uncovered = next;
        if uncovered.is_empty() {
            return true;
        }
    }
    uncovered.is_empty()
}

/// True iff `outer` fully contains `inner` (closed containment, sub-pixel safe).
#[inline]
fn rect_contains(outer: &Rect, inner: &Rect) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && outer.right() >= inner.right()
        && outer.bottom() >= inner.bottom()
}

/// Push the parts of `frag` NOT covered by `cut` into `out` (rectangle
/// subtraction → up to 4 axis-aligned pieces). If `cut` does not overlap
/// `frag`, `frag` is emitted unchanged.
fn subtract_rect(frag: &Rect, cut: &Rect, out: &mut Vec<Rect>) {
    let Some(overlap) = frag.intersection(cut) else {
        out.push(*frag);
        return;
    };

    let fx0 = frag.x;
    let fy0 = frag.y;
    let fx1 = frag.right();
    let fy1 = frag.bottom();
    let ox0 = overlap.x;
    let oy0 = overlap.y;
    let ox1 = overlap.right();
    let oy1 = overlap.bottom();

    // Top strip (full width, above the overlap).
    if oy0 > fy0 {
        out.push(Rect::new(fx0, fy0, fx1 - fx0, oy0 - fy0));
    }
    // Bottom strip (full width, below the overlap).
    if oy1 < fy1 {
        out.push(Rect::new(fx0, oy1, fx1 - fx0, fy1 - oy1));
    }
    // Left strip (only the overlap's vertical extent).
    if ox0 > fx0 {
        out.push(Rect::new(fx0, oy0, ox0 - fx0, oy1 - oy0));
    }
    // Right strip (only the overlap's vertical extent).
    if ox1 < fx1 {
        out.push(Rect::new(ox1, oy0, fx1 - ox1, oy1 - oy0));
    }
}

// --- Test-only cull probe ------------------------------------------------
//
// The cull tests need to assert that a fully-covered node was actually SKIPPED
// (not merely painted-then-overwritten). A thread-local records the ids culled
// by the most recent render so a test can prove the skip happened.
#[cfg(test)]
thread_local! {
    static CULLED_IDS: std::cell::RefCell<Vec<NodeId>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Record that `id` was occlusion-culled this frame (test-only probe).
#[cfg(test)]
pub(super) fn record_culled(id: NodeId) {
    CULLED_IDS.with(|c| c.borrow_mut().push(id));
}

/// Clear the cull probe; call before a render whose culls you want to inspect.
#[cfg(test)]
pub(crate) fn reset_cull_probe() {
    CULLED_IDS.with(|c| c.borrow_mut().clear());
}

/// Whether `id` was occlusion-culled since the last [`reset_cull_probe`].
#[cfg(test)]
pub(crate) fn was_culled(id: NodeId) -> bool {
    CULLED_IDS.with(|c| c.borrow().contains(&id))
}

#[cfg(not(test))]
#[allow(dead_code)]
fn _unused_nodeid(_: NodeId) {}
