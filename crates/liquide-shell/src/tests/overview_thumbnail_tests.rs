//! t93-e6 (gap #1) — cheap window-thumbnail overview tiles.
//!
//! These tests prove the overview paints REAL captured thumbnails (a `Surface`
//! node carrying the captured framebuffer snapshot) when a capture exists, and
//! falls back to the placeholder glass+solid tile when one does not. They FAIL
//! if the overview reverts to always emitting placeholders (no `Surface` node).

use crate::shell::Shell;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::scene::{SceneNode, SceneNodeKind};

const W: f32 = 1280.0;
const H: f32 = 720.0;

/// The REAL shipped component stylesheet — the overview tile boxes are laid out
/// by its `overview*` rules (t101-p5). The thumbnails are painted onto those
/// laid-out tile boxes, so the overview CSS must be loaded for the scene to
/// emit any tile / thumbnail. Driving the real on-disk CSS keeps these tests
/// honest (a regressed tile rule would collapse the boxes and drop the tiles).
const COMPONENTS_CSS: &str = include_str!("../../../../assets/themes/components.css");

fn test_shell() -> Shell {
    let mut shell = Shell::new(W, H);
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell.add_stylesheet(COMPONENTS_CSS);
    shell
}

/// A framebuffer the size of the screen, painted with a deterministic gradient
/// so any captured sub-rect carries real (non-uniform, non-zero) content.
fn painted_framebuffer() -> FrameBuffer {
    let mut fb = FrameBuffer::new(W as u32, H as u32, PixelFormat::Bgra8);
    for y in 0..fb.height {
        for x in 0..fb.width {
            fb.set_pixel(
                x,
                y,
                Color::new((x % 256) as u8, (y % 256) as u8, 128, 255),
            );
        }
    }
    fb
}

/// Count `Surface` nodes (real thumbnails) carrying a non-empty buffer.
fn count_thumbnail_surfaces(node: &SceneNode) -> usize {
    let here = matches!(
        &node.kind,
        SceneNodeKind::Surface { buffer: Some(buf), .. } if !buf.pixels.is_empty()
    ) as usize;
    here + node
        .children
        .iter()
        .map(count_thumbnail_surfaces)
        .sum::<usize>()
}

/// Find the overview tile fill node (the solid placeholder Background id =
/// NODE_WINDOW_BASE + id*STRIDE + 7 + 1) — its presence means a placeholder, not
/// a thumbnail. We detect placeholders structurally by the absence of any
/// Surface and the presence of the overview glass tiles.
fn count_glass_tiles(node: &SceneNode) -> usize {
    let here = matches!(&node.kind, SceneNodeKind::Glass(_)) as usize;
    here + node.children.iter().map(count_glass_tiles).sum::<usize>()
}

#[test]
fn overview_emits_thumbnail_surface_per_window_when_capture_exists() {
    let mut shell = test_shell();
    shell.open_window("Alpha", Rect::new(100.0, 80.0, 400.0, 300.0));
    shell.open_window("Beta", Rect::new(560.0, 120.0, 360.0, 260.0));

    let fb = painted_framebuffer();
    // Capture thumbnails from the composited framebuffer (the host seam).
    shell.capture_overview_thumbnails(&fb, 256);
    assert!(
        shell.has_overview_thumbnails(),
        "capture must store thumbnails for the two on-screen windows"
    );

    // Open the overview and build the scene.
    shell.overview_visible = true;
    let scene = shell.build_scene();

    // One real thumbnail Surface per window (2), not placeholders.
    let surfaces = count_thumbnail_surfaces(&scene);
    assert_eq!(
        surfaces, 2,
        "overview must emit one real thumbnail Surface node per window, got {surfaces}"
    );

    // The glass backing is still kept under each tile (one per window).
    assert!(
        count_glass_tiles(&scene) >= 2,
        "glass tile backing must remain under each thumbnail"
    );
}

#[test]
fn overview_falls_back_to_placeholder_when_no_capture() {
    let mut shell = test_shell();
    shell.open_window("Alpha", Rect::new(100.0, 80.0, 400.0, 300.0));
    shell.open_window("Beta", Rect::new(560.0, 120.0, 360.0, 260.0));

    // No capture pass run → no thumbnails.
    assert!(!shell.has_overview_thumbnails());

    shell.overview_visible = true;
    let scene = shell.build_scene();

    // ZERO thumbnail Surfaces — the placeholder path is used.
    assert_eq!(
        count_thumbnail_surfaces(&scene),
        0,
        "with no capture the overview must use placeholder tiles (no Surface nodes)"
    );
    // But the overview is still painted (glass tiles present).
    assert!(
        count_glass_tiles(&scene) >= 2,
        "placeholder overview still paints glass tiles"
    );
}

#[test]
fn overview_thumbnail_buffer_matches_captured_window_content() {
    // The thumbnail must carry the REAL pixels under the window rect, not a
    // uniform placeholder. Capture a window over a known gradient region and
    // assert the stored thumbnail is non-uniform and dimensioned sanely.
    let mut shell = test_shell();
    let bounds = Rect::new(200.0, 150.0, 400.0, 300.0);
    shell.open_window("Gradient", bounds);

    let fb = painted_framebuffer();
    shell.capture_overview_thumbnails(&fb, 256);

    let wid = shell.visible_windows()[0].id;
    let thumb = shell
        .overview_thumbnails
        .get(&wid)
        .expect("thumbnail captured for the window");

    // Bounded to tile_max on the longer edge (400 -> 256), aspect-preserving.
    assert!(thumb.width <= 256 && thumb.height <= 256);
    assert!(thumb.width > 1 && thumb.height > 1, "real, non-degenerate thumbnail");

    // Non-uniform: the source under the window rect is a gradient, so at least
    // two distinct pixels must exist (proves it is not a uniform placeholder).
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = (y * thumb.stride + x * 4) as usize;
        [
            thumb.pixels[i],
            thumb.pixels[i + 1],
            thumb.pixels[i + 2],
            thumb.pixels[i + 3],
        ]
    };
    let a = px(0, 0);
    let b = px(thumb.width - 1, thumb.height - 1);
    assert_ne!(a, b, "captured thumbnail must carry the real (non-uniform) window content");
}

#[test]
fn clearing_thumbnails_reverts_to_placeholders() {
    let mut shell = test_shell();
    shell.open_window("Alpha", Rect::new(100.0, 80.0, 400.0, 300.0));

    let fb = painted_framebuffer();
    shell.capture_overview_thumbnails(&fb, 256);
    shell.overview_visible = true;
    assert_eq!(count_thumbnail_surfaces(&shell.build_scene()), 1);

    // Closing the overview (host calls clear) drops the thumbnails so a later
    // session does not paint a stale snapshot.
    shell.clear_overview_thumbnails();
    assert!(!shell.has_overview_thumbnails());
    assert_eq!(
        count_thumbnail_surfaces(&shell.build_scene()),
        0,
        "after clear the overview falls back to placeholders"
    );
}
