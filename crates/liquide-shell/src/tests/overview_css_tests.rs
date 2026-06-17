//! Overview / exposé full-CSS migration regressions (t101-p5 / t86 P5).
//!
//! The overview used to be drawn by an imperative painter
//! (`scene.rs::add_overview_overlay`) with `cols=sqrt(count)` grid math and
//! literal rects. It is now a DOM/CSS overlay synced through
//! `sync_overview_template` and laid out by the CSS pipeline (the `overview*`
//! rules in `assets/themes/components.css`); only the per-tile window THUMBNAIL
//! (a `Surface` carrying the captured framebuffer) is painted manually, keyed
//! off each tile's LAID-OUT CSS box.
//!
//! These tests have TEETH for the contracts the migration must hold:
//!
//!   1. **Renders as a DOM grid** — the overview is a real DOM subtree
//!      (`overview-overlay` → `overview-grid` → `overview-tile` per window) and
//!      a CSS change MOVES the tiles. If the surface reverted to hardcoded grid
//!      geometry, the CSS-driven tile-box assertions break.
//!
//!   2. **Hit-test from the CSS tile box** — clicking a CSS-positioned tile
//!      resolves to the right window; a theme change that moves the tiles moves
//!      the click-zones with them (a click at a tile's NEW box focuses its
//!      window). This is the recurring hit-test-from-CSS-geometry requirement.
//!
//!   3. **Thumbnails still paint into tiles with placeholder fallback** — a
//!      captured thumbnail paints as a `Surface` onto the tile's laid-out box;
//!      with no capture the placeholder is used (covered jointly here + in
//!      `overview_thumbnail_tests`).

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{SceneNode, SceneNodeKind};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::shell::Shell;
use crate::window::WindowId;

/// The REAL shipped component stylesheet — the production source of the
/// `overview*` rules. Driving it through the pipeline (rather than an inline
/// stand-in) gives the tests teeth: if an `overview-tile` dimension regresses
/// on disk, the laid-out boxes move and the geometry assertions fail.
const COMPONENTS_CSS: &str = include_str!("../../../../assets/themes/components.css");

const W: f32 = 1280.0;
const H: f32 = 720.0;

/// A left mouse press at `(x, y)`.
fn press(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}

/// A shell with the real component CSS loaded, two windows open, the overview
/// toggled, and one scene built (so the pipeline lays out the tiles and the
/// hit-test engine has the overview boxes).
fn overview_shell() -> Shell {
    let mut shell = Shell::new(W, H);
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell.add_stylesheet(COMPONENTS_CSS);
    shell.open_window("Alpha", Rect::new(100.0, 80.0, 400.0, 300.0));
    shell.open_window("Beta", Rect::new(560.0, 120.0, 360.0, 260.0));
    shell.overview_visible = true;
    let _ = shell.build_scene();
    shell
}

/// Absolute laid-out box of a window's overview tile, read from the live CSS
/// layout tree (the same source the hit-test uses).
fn tile_box(shell: &Shell, id: WindowId) -> Option<liquide_layout::geometry::Rect> {
    let el = format!("overview-tile-{}", id.0);
    let node = shell.desktop_dom.doc.get_element_by_id(&el)?;
    shell.hit_test_engine.as_ref()?.bounds_for_node(node)
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

// ── Contract 1: the overview renders as a DOM grid ────────────────────────

/// Toggling the overview mounts the surface as a real DOM subtree — overlay
/// scrim, a grid, and one `overview-tile` per visible window carrying its
/// `data-window-id` — instead of an imperative grid of rects.
#[test]
fn overview_renders_as_dom_grid() {
    let mut shell = Shell::new(W, H);
    shell.open_window("Alpha", Rect::new(100.0, 80.0, 400.0, 300.0));
    shell.open_window("Beta", Rect::new(560.0, 120.0, 360.0, 260.0));

    // No overlay before toggling.
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("overview-overlay")
            .is_none(),
        "no overview overlay should exist before toggling"
    );

    shell.overview_visible = true;
    shell.sync_dom();

    let doc = &shell.desktop_dom.doc;
    assert!(
        doc.get_element_by_id("overview-overlay").is_some(),
        "toggling must mount the overview-overlay DOM element"
    );
    assert!(
        doc.get_element_by_id("overview-grid").is_some(),
        "overview must contain a grid element"
    );
    let ids: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    for id in &ids {
        assert!(
            shell
                .desktop_dom
                .doc
                .get_element_by_id(&format!("overview-tile-{}", id.0))
                .is_some(),
            "a tile element must exist per visible window ({id:?})"
        );
    }

    // Closing the overview removes the overlay from the DOM.
    shell.overview_visible = false;
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("overview-overlay")
            .is_none(),
        "closing must remove the overview overlay from the DOM"
    );
}

/// The tile boxes come from the CSS layout: with the real `overview-tile` rule
/// loaded the laid-out box matches the stylesheet dimensions (320 x 220), and
/// distinct windows get distinct, non-overlapping tile boxes. This proves the
/// grid is laid out by CSS, not painted at hardcoded grid geometry.
#[test]
fn tile_boxes_come_from_css_layout() {
    let shell = overview_shell();
    let ids: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    assert_eq!(ids.len(), 2);

    let a = tile_box(&shell, ids[0]).expect("tile A laid out");
    let b = tile_box(&shell, ids[1]).expect("tile B laid out");

    // Dimensions are literals in the `overview-tile` rule; if that rule
    // regresses on disk these assertions move.
    for (id, r) in [(ids[0], a), (ids[1], b)] {
        assert!(
            (r.width - 320.0).abs() < 1.0,
            "tile {id:?} width must come from the overview-tile CSS (320), got {}",
            r.width
        );
        assert!(
            (r.height - 220.0).abs() < 1.0,
            "tile {id:?} height must come from the overview-tile CSS (220), got {}",
            r.height
        );
    }

    // Distinct windows occupy distinct tile boxes (not collapsed at the origin).
    assert!(
        (a.x - b.x).abs() > 1.0 || (a.y - b.y).abs() > 1.0,
        "two windows must get two distinct tile boxes, got {a:?} and {b:?}"
    );
    assert!(
        a.x > 1.0 && a.y > 1.0,
        "tiles must be laid out inside the padded grid, not at the origin, got {a:?}"
    );
}

// ── Contract 2: hit-test derives from the CSS tile box ────────────────────

/// Clicking the center of a CSS-positioned tile focuses + raises its window
/// (and closes the overview). The click resolves through the laid-out tile box,
/// proving the hit-test reads CSS geometry.
#[test]
fn click_on_css_tile_focuses_its_window() {
    let mut shell = overview_shell();
    let ids: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    let target = ids[1];

    let r = tile_box(&shell, target).expect("target tile laid out");
    let cx = r.x + r.width / 2.0;
    let cy = r.y + r.height / 2.0;

    shell.handle_platform_event(&press(cx, cy));

    assert!(!shell.overview_visible, "clicking a tile closes the overview");
    assert_eq!(
        shell.focus.focused(),
        Some(target),
        "clicking tile {target:?} must focus its window"
    );
}

/// THE geometry-from-CSS tooth: a theme override that MOVES/resizes the tiles
/// moves the click-zones with them. After widening the tiles, a click at a
/// tile's NEW center still focuses its window; the resolved zone is the NEW CSS
/// box, not a stale grid cell.
///
/// If the hit-test used hardcoded grid math instead of the laid-out box, the
/// click-zone would NOT follow the stylesheet and this test would fail.
#[test]
fn theme_change_moves_the_tile_click_zones() {
    let mut shell = overview_shell();
    let ids: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    let target = ids[0];

    let before = tile_box(&shell, target).expect("baseline tile box");

    // Override the tiles to a distinctly different size. `add_stylesheet`
    // appends with higher precedence, so this resizes the boxes.
    shell.add_stylesheet("overview-tile { width: 180; height: 140; }");
    let _ = shell.build_scene();

    let after = tile_box(&shell, target).expect("overridden tile box");

    // The box actually changed (the CSS override took effect).
    assert!(
        (after.width - 180.0).abs() < 1.0 && (after.height - 140.0).abs() < 1.0,
        "the override must resize the laid-out tile box, got {after:?}"
    );
    assert!(
        (after.width - before.width).abs() > 1.0,
        "the tile must have a different width after the theme override"
    );

    // A click at the NEW center focuses the window — the zone tracks the NEW CSS
    // box. (The overview was rebuilt above, so it is still open.)
    let new_center_x = after.x + after.width / 2.0;
    let new_center_y = after.y + after.height / 2.0;
    assert_eq!(
        shell.overview_tile_window_at(new_center_x, new_center_y),
        Some(target),
        "a point in the NEW CSS tile box must resolve to the window (zone moved with CSS)"
    );
}

// ── Contract 3: thumbnails paint into the tiles with placeholder fallback ──

/// A captured thumbnail paints as a `Surface` onto the tile's laid-out box; the
/// glass placeholder is the fallback when no capture exists. This proves the
/// E6 thumbnail capture still works through the migrated (DOM/CSS) overview.
#[test]
fn thumbnails_paint_into_css_tiles_with_placeholder_fallback() {
    use liquide_compositor::framebuffer::FrameBuffer;
    use liquide_compositor::pixel::{Color, PixelFormat};

    // No capture → placeholder path: zero thumbnail Surfaces, overview still
    // painted (tiles laid out).
    let mut shell = overview_shell();
    assert!(!shell.has_overview_thumbnails());
    assert_eq!(
        count_thumbnail_surfaces(&shell.build_scene()),
        0,
        "with no capture the overview uses placeholder tiles (no Surface nodes)"
    );

    // Capture from a painted framebuffer → one real thumbnail Surface per
    // window, painted onto the laid-out tile boxes.
    let mut fb = FrameBuffer::new(W as u32, H as u32, PixelFormat::Bgra8);
    for y in 0..fb.height {
        for x in 0..fb.width {
            fb.set_pixel(x, y, Color::new((x % 256) as u8, (y % 256) as u8, 128, 255));
        }
    }
    shell.capture_overview_thumbnails(&fb, 256);
    let scene = shell.build_scene();
    assert_eq!(
        count_thumbnail_surfaces(&scene),
        2,
        "each window's captured thumbnail must paint as a Surface onto its tile"
    );

    // The painted thumbnail sits on the tile's laid-out CSS box (not at the
    // origin / a recomputed grid cell): a Surface must overlap each tile box.
    let ids: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    for id in &ids {
        let tb = tile_box(&shell, *id).expect("tile laid out");
        assert!(
            scene_has_surface_within(&scene, tb),
            "a thumbnail Surface must be painted within tile {id:?}'s CSS box {tb:?}"
        );
    }
}

/// Whether the scene has any thumbnail `Surface` whose bounds lie within (or
/// touching) the given tile box — the thumbnail is centred & fitted inside it.
fn scene_has_surface_within(node: &SceneNode, tile: liquide_layout::geometry::Rect) -> bool {
    let here = match &node.kind {
        SceneNodeKind::Surface { buffer: Some(_), .. } => {
            let b = node.properties.bounds;
            b.x >= tile.x - 1.0
                && b.y >= tile.y - 1.0
                && b.x + b.width <= tile.x + tile.width + 1.0
                && b.y + b.height <= tile.y + tile.height + 1.0
        }
        _ => false,
    };
    here || node
        .children
        .iter()
        .any(|c| scene_has_surface_within(c, tile))
}
