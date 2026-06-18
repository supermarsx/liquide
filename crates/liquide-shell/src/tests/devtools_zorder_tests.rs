//! Regression: the in-DE DevTools panel must paint ABOVE the window workspace.
//!
//! ROOT CAUSE (t128 Phase 1): the devtools panel renders through the CSS
//! pipeline (a `position: fixed; z-index: 9900` template mounted into the
//! desktop DOM root) with an OPAQUE background (`--bg-secondary` = #18181b).
//! The window workspace is appended as a SEPARATE manual scene node. If the
//! band classification in `scene.rs::build_scene` placed the panel below — or
//! at the same z-band as — the workspace, the opaque panel would paint UNDER
//! the windows and read as transparent/janky.
//!
//! The fix lives in the two-track merge in `scene.rs::build_scene`
//! (`WORKSPACE_Z_ORDER = 100` for the manual window workspace; every non-
//! background CSS chrome node — including the high-z devtools panel — is
//! classified into the overlay band at `CHROME_Z_BASE = 10_000`+). Because the
//! compositor flattens children in z-order, the panel's opaque background ends
//! up LATER in paint order than the window subtree it overlaps, so the panel's
//! pixels win.
//!
//! These tests do NOT depend on `liquide-devtools` (a higher-level crate); they
//! mount a template + stylesheet that mirror the devtools panel's exact CSS
//! contract (`position: fixed`, full-width bottom dock, opaque background,
//! `z-index: 9900`) and assert the band-classification invariant on the real
//! scene graph the locked code produces.
//!
//! NO-FAKE-GREEN: the teeth were verified by sabotage — forcing the panel into
//! the background band (or the workspace above `CHROME_Z_BASE`) in
//! `scene.rs::build_scene` flips both assertions RED (panel painted under the
//! window). See the inline notes.

use crate::shell::Shell;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{FlatNode, SceneNode, SceneNodeKind};

/// CSS that mirrors the real devtools panel contract: a fixed, full-width
/// bottom-docked strip with an OPAQUE background and the canonical
/// `--z-devtools` (9900) stacking level.
const PANEL_CSS: &str = "devtools-panel { position: fixed; left: 0; right: 0; \
     bottom: 0; height: 320px; z-index: 9900; background: #18181b; }";

/// The panel is a full-width opaque strip at the bottom of a 1280×720 surface:
/// y ∈ [400, 720]. We open the window so it OVERLAPS this strip.
const SURFACE_W: f32 = 1280.0;
const SURFACE_H: f32 = 720.0;
const PANEL_TOP: f32 = SURFACE_H - 320.0; // 400

fn test_shell() -> Shell {
    let mut shell = Shell::new(SURFACE_W, SURFACE_H);
    // Freeze the caret blink so build_scene never invalidates on a blink toggle.
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell
}

/// Mount the panel template (mirrors `DevToolsState::sync_template` → the shell
/// `mount_template` call) and build the scene with it present.
fn scene_with_panel_over_window(shell: &mut Shell) -> SceneNode {
    shell.add_stylesheet(PANEL_CSS);
    // A window whose body overlaps the bottom panel strip (y 100..580 ∩ 400..720).
    shell.open_window("Overlapped", Rect::new(100.0, 100.0, 640.0, 480.0));
    let _ = shell.build_scene();

    let template = liquide_components::TemplateNode::el("devtools-panel").id("devtools-panel");
    shell.mount_template("devtools-panel", &template);
    shell.build_scene()
}

/// True if two rects overlap with positive area.
fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    let ax2 = a.x + a.width;
    let ay2 = a.y + a.height;
    let bx2 = b.x + b.width;
    let by2 = b.y + b.height;
    a.x < bx2 && b.x < ax2 && a.y < by2 && b.y < ay2
}

/// Locate the panel's opaque background FlatNode: a full-width fill at the
/// bottom strip whose color is opaque. Returns (paint_index, FlatNode).
fn panel_background(flat: &[FlatNode]) -> Option<(usize, FlatNode)> {
    flat.iter().enumerate().find_map(|(i, n)| {
        let b = &n.absolute_bounds;
        let is_bottom_strip = (b.y - PANEL_TOP).abs() < 1.0
            && b.width >= SURFACE_W - 1.0
            && (b.height - 320.0).abs() < 1.0;
        let opaque = matches!(
            n.kind_ref(),
            SceneNodeKind::Background { color } if color.a == 255
        );
        (is_bottom_strip && opaque).then(|| (i, n.clone()))
    })
}

#[test]
fn devtools_panel_opaque_background_paints_above_overlapping_window() {
    let mut shell = test_shell();
    let scene = scene_with_panel_over_window(&mut shell);
    let flat = scene.flatten();

    // The panel's opaque background must exist in the scene at all.
    let (panel_idx, panel) = panel_background(&flat).expect(
        "devtools panel opaque background must be present in the flattened scene \
         (CSS pipeline emits a fixed bottom-docked opaque fill)",
    );

    // Find the window workspace subtree's painted nodes. Window leaf nodes live
    // in the manual workspace subtree (ids in the 10_000+ window range). We
    // assert paint ORDER: every window node that OVERLAPS the panel strip must
    // come BEFORE the panel background in the flattened (z-sorted, painter-
    // order) output — i.e. the panel paints OVER it.
    const NODE_WINDOW_BASE: u64 = 10_000;
    let panel_rect = panel.absolute_bounds;

    let overlapping_window_nodes: Vec<(usize, &FlatNode)> = flat
        .iter()
        .enumerate()
        .filter(|(_, n)| n.id >= NODE_WINDOW_BASE && n.id < 100_000)
        .filter(|(_, n)| rects_overlap(&n.absolute_bounds, &panel_rect))
        .collect();

    assert!(
        !overlapping_window_nodes.is_empty(),
        "test precondition: the window must overlap the panel strip so the \
         occlusion is exercised (window 100,100,640,480 ∩ panel y>=400)",
    );

    for (idx, n) in &overlapping_window_nodes {
        assert!(
            *idx < panel_idx,
            "window node id={} (paint index {}) paints AFTER the devtools panel \
             background (paint index {}) — the panel is occluded by the window. \
             Panel z={}, window node z={}.",
            n.id,
            idx,
            panel_idx,
            panel.z_order,
            n.z_order,
        );
    }
}

#[test]
fn devtools_panel_z_order_exceeds_window_workspace_band() {
    // A tighter invariant on the locked band classification: the panel's z_order
    // must land in the chrome overlay band (>= CHROME_Z_BASE = 10_000), strictly
    // above the manual window workspace band (WORKSPACE_Z_ORDER = 100). Sabotage:
    // classifying the panel into the background band, or raising WORKSPACE_Z_ORDER
    // above CHROME_Z_BASE, flips this RED.
    let mut shell = test_shell();
    let scene = scene_with_panel_over_window(&mut shell);
    let flat = scene.flatten();

    let (_, panel) = panel_background(&flat).expect("panel background present");

    // The workspace node carries WORKSPACE_Z_ORDER (100) at the root level.
    const WORKSPACE_NODE_ID: u64 = 100;
    let workspace_z = scene
        .children
        .iter()
        .find(|c| c.id == WORKSPACE_NODE_ID)
        .map(|c| c.properties.z_order)
        .expect("window workspace node present in root children");

    assert!(
        panel.z_order > workspace_z,
        "devtools panel z_order ({}) must exceed the window workspace z_order \
         ({}) so it composites above windows",
        panel.z_order,
        workspace_z,
    );
    assert!(
        panel.z_order >= 10_000,
        "devtools panel must be classified into the chrome overlay band \
         (>= CHROME_Z_BASE 10_000), got z_order {}",
        panel.z_order,
    );
}

/// Sanity: a fully opaque #18181b really does mean alpha 255 (guards the
/// `panel_background` opaque filter above — if the fill ever resolved with
/// alpha < 255 the test would silently stop finding the panel and pass vacuously).
#[test]
fn opaque_panel_fill_is_actually_opaque() {
    let mut shell = test_shell();
    let scene = scene_with_panel_over_window(&mut shell);
    let flat = scene.flatten();
    let strip = flat.iter().find(|n| {
        let b = &n.absolute_bounds;
        (b.y - PANEL_TOP).abs() < 1.0 && b.width >= SURFACE_W - 1.0
    });
    let strip = strip.expect("a full-width fill at the panel strip must exist");
    match strip.kind_ref() {
        SceneNodeKind::Background { color } => assert_eq!(
            *color,
            Color::new(0x18, 0x18, 0x1b, 255),
            "panel background must be the opaque #18181b fill"
        ),
        other => panic!("expected an opaque Background fill at the panel strip, got {other:?}"),
    }
}
