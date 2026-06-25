//! Teeth for the CSS group-opacity → isolated `RenderLayer` wiring in the scene
//! bridge (`pipeline/scene_bridge.rs`).
//!
//! CSS `opacity < 1` creates a stacking context with GROUP opacity (CSS Color
//! §opacity / Compositing §pgl): the element's whole subtree must composite onto
//! an ISOLATED layer and merge ONCE at the group opacity. The renderer already
//! honours this via its `LayerScope` when it sees a `RenderLayer { isolate: true }`
//! scene node — but the CSS path never emitted such a node: the bridge merely
//! folded `opacity` into accumulated state and applied it to each leaf, so two
//! overlapping translucent children DOUBLE-composited (the overlap darkened).
//!
//! These tests assert the WIRING the fix added:
//!   1. an element with `opacity: 0.5` emits a `RenderLayer { isolate: true }`
//!      carrying that opacity, whose window covers the whole opacity subtree;
//!   2. a fully OPAQUE element (`opacity: 1`, the common case) emits NO such
//!      layer — no behaviour/perf change for opaque content;
//!   3. `isolation: isolate` (mix-blend / stacking isolation) likewise routes
//!      through the layer only when it actually carries group opacity.
//!
//! NO-FAKE-GREEN: assertion (1) is RED on the pre-fix bridge (no RenderLayer node
//! is ever produced from `opacity`), and assertion (2) is RED if the fix over-
//! wraps opaque elements. The end-to-end "overlap composites once" pixel proof
//! lives in `liquide-visual-test` (`cap_group_opacity_css`), which drives this
//! same bridge through the real `SoftwareRenderer`.

use crate::shell::Shell;
use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{FlatNode, SceneNode, SceneNodeKind};

const SURFACE_W: f32 = 800.0;
const SURFACE_H: f32 = 600.0;

fn test_shell() -> Shell {
    let mut shell = Shell::new(SURFACE_W, SURFACE_H);
    // Freeze the caret blink so build_scene never invalidates on a blink toggle.
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell
}

/// All flattened isolated render layers and their (opacity, window-bounds).
fn isolated_layers(scene: &SceneNode) -> Vec<(f32, Rect)> {
    scene
        .flatten()
        .into_iter()
        .filter_map(|n: FlatNode| match n.kind_ref() {
            SceneNodeKind::RenderLayer { isolate: true, .. } => {
                Some((n.opacity, n.absolute_bounds))
            }
            _ => None,
        })
        .collect()
}

/// `inner` is contained within `outer` (with a small tolerance).
fn contains(outer: &Rect, inner: &Rect, tol: f32) -> bool {
    outer.x <= inner.x + tol
        && outer.y <= inner.y + tol
        && outer.x + outer.width + tol >= inner.x + inner.width
        && outer.y + outer.height + tol >= inner.y + inner.height
}

/// Mount a fixed, full-surface container holding a single child subtree and
/// build the scene with it present.
fn scene_with(shell: &mut Shell, child: liquide_components::TemplateNode) -> SceneNode {
    let canvas = liquide_components::TemplateNode::el("div")
        .id("opacity-canvas")
        .style("position", "fixed")
        .style("left", "0")
        .style("top", "0")
        .style("width", &format!("{SURFACE_W}px"))
        .style("height", &format!("{SURFACE_H}px"))
        .style("background", "#000000")
        .style("z-index", "90000")
        .child(child);
    shell.mount_template("opacity-canvas", &canvas);
    shell.build_scene()
}

/// A group of two OVERLAPPING absolutely-positioned children, each opaque. The
/// children span x[100,300) and x[200,400) → overlap x[200,300). The group's
/// own opacity is supplied by the caller.
fn overlap_group(extra_style: &[(&str, &str)]) -> liquide_components::TemplateNode {
    let mut group = liquide_components::TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "0")
        .style("top", "0")
        .style("width", &format!("{SURFACE_W}px"))
        .style("height", &format!("{SURFACE_H}px"));
    for (k, v) in extra_style {
        group = group.style(k, v);
    }
    let a = liquide_components::TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "100px")
        .style("top", "100px")
        .style("width", "200px")
        .style("height", "200px")
        .style("background", "#ffffff");
    let b = liquide_components::TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "200px")
        .style("top", "100px")
        .style("width", "200px")
        .style("height", "200px")
        .style("background", "#ffffff");
    group.child(a).child(b)
}

#[test]
fn css_opacity_lt_one_emits_isolated_render_layer() {
    let mut shell = test_shell();
    let scene = scene_with(&mut shell, overlap_group(&[("opacity", "0.5")]));

    let layers = isolated_layers(&scene);
    assert!(
        !layers.is_empty(),
        "a CSS element with opacity:0.5 must emit a RenderLayer{{isolate:true}} \
         scene node so its subtree composites onto an isolated layer and merges \
         ONCE at the group opacity — got none (the CSS→RenderLayer wiring is \
         missing; overlaps would double-composite)."
    );

    // The group-opacity layer carries ~0.5 and its window covers both children
    // (x[100,400), y[100,300)) so the renderer snapshots/clears the whole group.
    let children_window = Rect::new(100.0, 100.0, 300.0, 200.0);
    let group_layer = layers
        .iter()
        .find(|(op, _)| (op - 0.5).abs() <= 0.02)
        .unwrap_or_else(|| {
            panic!("no isolated layer carries the 0.5 group opacity; got {layers:?}")
        });
    assert!(
        contains(&group_layer.1, &children_window, 1.0),
        "the isolated layer window {:?} must cover the whole opacity subtree \
         {children_window:?} (else the renderer clips the group)",
        group_layer.1
    );
}

#[test]
fn opaque_css_element_emits_no_isolated_layer() {
    // The common case: a fully-opaque element (opacity:1, no isolate, no blend)
    // must NOT be wrapped in a layer — opaque content is unchanged (no perf or
    // behaviour regression).
    let mut shell = test_shell();
    let scene = scene_with(&mut shell, overlap_group(&[("opacity", "1")]));

    let layers = isolated_layers(&scene);
    assert!(
        layers.is_empty(),
        "an opaque element (opacity:1) must NOT emit an isolated RenderLayer — \
         opaque content must composite directly. Got {layers:?}."
    );
}

#[test]
fn opacity_layer_dimming_matches_group_not_per_child() {
    // The layer must carry the GROUP opacity and the children must NOT each be
    // pre-dimmed (otherwise opacity is applied twice: once per child, once on the
    // layer). Assert every white child inside the group flattens at FULL alpha
    // (opacity ~1) — the dimming lives solely on the isolated layer.
    let mut shell = test_shell();
    let scene = scene_with(&mut shell, overlap_group(&[("opacity", "0.5")]));

    let white_children: Vec<FlatNode> = scene
        .flatten()
        .into_iter()
        .filter(|n| matches!(n.kind_ref(), SceneNodeKind::Background { color }
            if color.r == 255 && color.g == 255 && color.b == 255))
        .collect();
    assert!(
        white_children.len() >= 2,
        "expected the two overlapping white children to flatten; got {}",
        white_children.len()
    );
    for child in &white_children {
        assert!(
            child.opacity >= 0.99,
            "a child inside an opacity group must paint at FULL alpha onto the \
             isolated layer (the group opacity is applied ONCE on the layer, not \
             per-child) — got child opacity {}",
            child.opacity
        );
    }
}
