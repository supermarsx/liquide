//! t112-p9: prove the CSS pipeline is the single render path for chrome,
//! overlays, and windows — i.e. the retired imperative `thread_coordinator`
//! fallback track is never composited.
//!
//! Before P9 the shell carried a parallel imperative render track (the dock /
//! statusbar / launcher / notification element threads, plus the
//! `Launcher::build_scene` / `NotificationManager::build_scene` painters). That
//! track was composited ONLY when the CSS pipeline produced no chrome nodes
//! (`pipeline_empty`). With every chrome surface CSS-driven the pipeline always
//! emits at least the desktop-background fill, so the fallback was dead. These
//! tests pin that invariant so the dead track cannot be silently revived.
//!
//! NO-FAKE-GREEN: the fallback track, when it DID composite, remapped every
//! node id into the `THREAD_NODE_ID_BASE` (9_000_000_000_000+) range via
//! `remap_thread_scene_ids`. We assert no scene node ever lands in that range.
//! Teeth: re-introducing the fallback compositing branch (so threaded nodes
//! replace the CSS nodes) would push every id into that range and fail
//! `no_thread_remapped_node_ids_appear_in_the_scene`.

use liquide_compositor::scene::{SceneNode, SceneNodeKind};

use crate::shell::Shell;

/// The id base the retired fallback used to remap threaded scene nodes onto.
/// No node produced by the live CSS pipeline (or the by-design window/cursor
/// painters) ever reaches this range, so its appearance would mean the dead
/// fallback track composited.
const THREAD_NODE_ID_BASE: u64 = 9_000_000_000_000;

/// CSS chrome (statusbar, dock, menus, launcher, notifications, overlays) is
/// emitted at this z-band by `build_scene`; the desktop background sits below
/// and windows at `WORKSPACE_Z_ORDER` (100) in between.
const CHROME_Z_BASE: u32 = 10_000;

fn walk<'a>(node: &'a SceneNode, out: &mut Vec<&'a SceneNode>) {
    out.push(node);
    for c in &node.children {
        walk(c, out);
    }
}

fn flatten(node: &SceneNode) -> Vec<&SceneNode> {
    let mut out = Vec::new();
    walk(node, &mut out);
    out
}

fn max_thread_id(node: &SceneNode) -> u64 {
    flatten(node).iter().map(|n| n.id).max().unwrap_or(0)
}

#[test]
fn css_pipeline_produces_chrome_so_fallback_is_never_taken() {
    // A freshly booted shell loads the default CSS theme + desktop DOM, so the
    // CSS pipeline emits the desktop-background plus the chrome band.
    let mut shell = Shell::new(1280.0, 720.0);
    let scene = shell.build_scene();
    let nodes = flatten(&scene);

    // There IS a CSS-emitted background (full-screen fill at the low z-band).
    let screen_area = 1280.0 * 720.0;
    let has_background = nodes.iter().any(|n| {
        let b = &n.properties.bounds;
        b.width * b.height >= screen_area * 0.9
            && matches!(
                n.kind,
                SceneNodeKind::Background { .. }
                    | SceneNodeKind::GradientFill { .. }
                    | SceneNodeKind::Image { .. }
            )
    });
    assert!(
        has_background,
        "CSS pipeline must emit a full-screen desktop background — if it does \
         not, `pipeline_empty` could be true and the dead fallback would revive"
    );

    // And there IS chrome composited at the CSS chrome z-band (statusbar/dock).
    let has_chrome = nodes
        .iter()
        .any(|n| n.properties.z_order >= CHROME_Z_BASE);
    assert!(
        has_chrome,
        "CSS pipeline must emit shell chrome at the CHROME_Z_BASE band"
    );
}

#[test]
fn no_thread_remapped_node_ids_appear_in_the_scene() {
    // Boot a busy desktop: a window, an open launcher, and active notifications
    // — exactly the surfaces the retired imperative painters used to draw.
    let mut shell = Shell::new(1280.0, 720.0);
    let _ = shell.open_app_window("com.liquide.files");
    shell.launcher.open();
    let _ = shell.notifications.notify(
        liquide_interop::notification::Notification::new("test", "Title"),
        1,
    );

    let scene = shell.build_scene();

    // If the fallback track ever composited, `remap_thread_scene_ids` would push
    // every id past THREAD_NODE_ID_BASE. The single CSS path never does.
    assert!(
        max_thread_id(&scene) < THREAD_NODE_ID_BASE,
        "a scene node id landed in the retired thread-remap range — the dead \
         fallback track composited (it must not)"
    );
}

#[test]
fn launcher_and_notifications_render_through_css_not_the_retired_painters() {
    // The launcher overlay and notification cards must still appear when shown,
    // proving the CSS path covers the surfaces whose imperative painters were
    // retired. (Their absence would mean we deleted a live painter.)
    let mut shell = Shell::new(1280.0, 720.0);

    // Baseline: nothing shown.
    let base = flatten(&shell.build_scene()).len();

    shell.launcher.open();
    let _ = shell.notifications.notify(
        liquide_interop::notification::Notification::new("n1", "Hello"),
        1,
    );

    let shown = flatten(&shell.build_scene()).len();
    assert!(
        shown > base,
        "showing the launcher + a notification must add scene nodes via the CSS \
         path (base={base}, shown={shown})"
    );
    // Still single-path: no thread-remapped ids.
    assert!(max_thread_id(&shell.build_scene()) < THREAD_NODE_ID_BASE);
}
