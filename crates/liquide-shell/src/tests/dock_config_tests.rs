//! t73-shell: shell-level application of the configurable dock
//! ([`Shell::set_dock_config`] / `work_area` per position / DOM attrs) and the
//! CSS-driven cursor seam ([`Shell::cursor_theme`]).
//!
//! These prove the *shell wiring* on top of the crate-level dock behavior
//! covered by `dock_tests` and `liquide-dock`'s own suite.

use liquide_compositor::pixel::Color;
use liquide_dock::{AutoHideMode, DockAlignment, DockConfig, DockPosition};
use liquide_input::mouse::MouseEvent;
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::shell::Shell;

// ───────────────────────── Dock magnification (t172-e5) ─────────────────────────

/// Read the per-item `transform: scale(f)` the magnification seam wrote onto the
/// dock items, in index order. Resolves the dock-item nodes the SAME way the
/// runtime does (the children of `#shell-dock`), so the test reads what actually
/// renders, not a constant.
fn dock_item_scales(shell: &Shell) -> Vec<f32> {
    let doc = &shell.desktop_dom.doc;
    let dock = doc
        .get_element_by_id(crate::desktop_dom::element_ids::DOCK)
        .expect("dock element present");
    doc.children(dock)
        .iter()
        .map(|&item| {
            let style = doc
                .get_inline_style(item, "transform")
                .unwrap_or_else(|| "scale(1.0)".to_string());
            // Parse `scale(<f>)`.
            let inner = style
                .trim()
                .strip_prefix("scale(")
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or("1.0");
            inner.parse::<f32>().unwrap_or(1.0)
        })
        .collect()
}

/// Index of the largest scale.
fn peak_index(scales: &[f32]) -> usize {
    scales
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap()
}

/// Build a 6-item bottom dock and return the laid-out item-center X coordinates
/// (resolved from the LAID-OUT boxes, not constants) plus the dock's vertical
/// center for placing the cursor inside the dock.
fn wide_dock_shell() -> (Shell, Vec<f32>, f32) {
    let mut shell = Shell::new(1920.0, 1080.0);
    // The default Shell seeds 4 items; add two more for a wider dock so the
    // magnification peak has clear room to move.
    shell.dock_mut().add_pinned("app.a", "Alpha", "a");
    shell.dock_mut().add_pinned("app.b", "Bravo", "b");
    shell.sync_dom();

    let rects = shell.dock().compute_item_rects(shell.screen_rect());
    let centers: Vec<f32> = rects.iter().map(|(_, r)| r.x + r.width / 2.0).collect();
    let bounds = shell.dock().compute_bounds(shell.screen_rect());
    let cursor_y = bounds.y + bounds.height / 2.0;
    (shell, centers, cursor_y)
}

#[test]
fn dock_magnification_peak_is_nearest_item_and_falls_off() {
    let (mut shell, centers, cy) = wide_dock_shell();
    assert!(centers.len() >= 6, "expected a wide dock: {centers:?}");

    // Park the cursor exactly over item index 1.
    move_mouse(&mut shell, centers[1], cy);
    shell.sync_dom();
    let scales = dock_item_scales(&shell);
    assert_eq!(scales.len(), centers.len(), "one scale per item");

    // (a) The item NEAREST the cursor (index 1) gets the LARGEST scale.
    assert_eq!(
        peak_index(&scales),
        1,
        "the cursor-nearest item must be the magnification peak: {scales:?}"
    );
    assert!(
        scales[1] > 1.3,
        "the peak should be strongly magnified (~factor 1.5): {scales:?}"
    );

    // (b) Monotonic falloff: each step away from the peak is no larger than the
    // one before it (smooth taper), and the farthest item is ~1.0.
    assert!(scales[2] > scales[3], "scale must fall off with distance: {scales:?}");
    assert!(scales[3] >= scales[4] - 1e-3, "monotone falloff: {scales:?}");
    let last = *scales.last().unwrap();
    assert!(
        (last - 1.0).abs() < 0.05,
        "items far from the cursor stay ~1.0: last={last} scales={scales:?}"
    );
}

#[test]
fn dock_magnification_peak_follows_the_cursor() {
    // Anti-constant tooth: the peak must MOVE with the cursor. A no-op / constant
    // scale (every item the same, or a fixed magnified item) fails this — the
    // peak index would not change when the cursor moves to a different item.
    let (mut shell, centers, cy) = wide_dock_shell();

    move_mouse(&mut shell, centers[1], cy);
    shell.sync_dom();
    let peak_a = peak_index(&dock_item_scales(&shell));

    move_mouse(&mut shell, centers[4], cy);
    shell.sync_dom();
    let scales_b = dock_item_scales(&shell);
    let peak_b = peak_index(&scales_b);

    assert_eq!(peak_a, 1, "peak under item 1");
    assert_eq!(peak_b, 4, "peak moved to item 4 with the cursor: {scales_b:?}");
    assert_ne!(
        peak_a, peak_b,
        "the magnification peak must FOLLOW the cursor, not stay fixed"
    );
}

#[test]
fn dock_magnification_resets_to_one_off_dock() {
    let (mut shell, centers, cy) = wide_dock_shell();

    // Over the dock → magnified.
    move_mouse(&mut shell, centers[2], cy);
    shell.sync_dom();
    let on = dock_item_scales(&shell);
    assert!(on[2] > 1.2, "item under cursor is magnified on-dock: {on:?}");

    // Move the cursor far from the dock (top-left corner) → all reset to 1.0.
    move_mouse(&mut shell, 4.0, 4.0);
    shell.sync_dom();
    let off = dock_item_scales(&shell);
    for (i, s) in off.iter().enumerate() {
        assert!(
            (s - 1.0).abs() < 1e-3,
            "off-dock item {i} must reset to scale(1.0): got {s} ({off:?})"
        );
    }
}

// The REAL shipped CSS — drive the production cascade so the test proves the
// dock CSS actually RENDERS (post the t181 cascade-loader fix), not a stand-in.
const VARIABLES_CSS: &str = include_str!("../../../../assets/themes/variables.css");
const DOCK_CSS: &str = include_str!("../../../../assets/themes/components/dock.css");
const MACOS_DARK_CSS: &str = include_str!("../../../../assets/themes/macos_dark.css");

/// Collect every `Glass` node's (tint, blur) and every `BackgroundFill` color
/// whose laid-out box sits in the bottom dock band of the screen.
fn dock_band_materials(
    node: &liquide_compositor::scene::SceneNode,
    band_top: f32,
    out: &mut Vec<(liquide_compositor::pixel::Color, u32)>,
) {
    use liquide_compositor::scene::SceneNodeKind;
    let b = node.properties.bounds;
    let in_band = b.y + b.height >= band_top && b.height > 0.0 && b.width > 0.0;
    if in_band {
        match &node.kind {
            SceneNodeKind::Glass(p) => out.push((p.tint_color, p.blur_radius)),
            SceneNodeKind::BackgroundFill { background } => {
                if let Some(c) = background.color {
                    out.push((c, 0));
                }
            }
            SceneNodeKind::Tint { color } => out.push((*color, 0)),
            _ => {}
        }
    }
    for child in &node.children {
        dock_band_materials(child, band_top, out);
    }
}

#[test]
fn dock_css_renders_dark_styled_material_post_cascade_fix() {
    use liquide_dock::DockConfig;

    let mut shell = Shell::new(1920.0, 1080.0);
    // Drive the production cascade in load order: base variables → split dock
    // component → macos_dark theme (last, wins). If the cascade loader were
    // still broken (pre-t181) these would not take effect and the dock would
    // fall back to an unstyled (transparent / light) fill, failing the asserts.
    shell.add_stylesheet(VARIABLES_CSS);
    shell.add_stylesheet(DOCK_CSS);
    shell.add_stylesheet(MACOS_DARK_CSS);
    // Make sure the dock has items + is laid out, then build the scene.
    shell.set_dock_config(DockConfig::default());
    shell.dock_mut().add_pinned("app.a", "Alpha", "a");
    shell.sync_dom();
    let root = shell.build_scene();

    let bounds = shell.dock().compute_bounds(shell.screen_rect());
    let band_top = bounds.y; // dock band = the dock's own top downward
    let mut mats = Vec::new();
    dock_band_materials(&root, band_top, &mut mats);

    assert!(
        !mats.is_empty(),
        "the dock must emit a styled material in its band — got none (dock CSS not rendering?)"
    );
    // macos_dark dock material is DARK (low RGB) and TRANSLUCENT (alpha < 255):
    // `--dock-bg`/glass-tint = rgba(40,40,42,0.45). Prove at least one such
    // material renders in the dock band.
    let dark_translucent = mats.iter().any(|(c, _)| {
        c.r < 90 && c.g < 90 && c.b < 90 && c.a > 10 && c.a < 250
    });
    assert!(
        dark_translucent,
        "dock must render a DARK translucent material (macos_dark glass tint); got {mats:?}"
    );
    // And the dock is a glass surface — at least one blurred backdrop in the band.
    let has_blur = mats.iter().any(|(_, blur)| *blur > 0);
    assert!(
        has_blur,
        "dock must render a blurred glass backdrop (backdrop-filter); got {mats:?}"
    );
}

#[test]
fn dock_magnification_resolves_item_positions_from_laid_out_boxes() {
    // The seam must read item centers from the LAID-OUT dock boxes, not a
    // constant stride. Prove it by changing the icon size (which changes the
    // laid-out box geometry) and asserting the peak still tracks the box the
    // cursor is actually over.
    let mut shell = Shell::new(1600.0, 1000.0);
    shell.dock_mut().add_pinned("app.a", "Alpha", "a");
    shell.set_dock_config(DockConfig {
        icon_size: 72,
        ..DockConfig::default()
    });
    shell.sync_dom();

    let rects = shell.dock().compute_item_rects(shell.screen_rect());
    let centers: Vec<f32> = rects.iter().map(|(_, r)| r.x + r.width / 2.0).collect();
    let bounds = shell.dock().compute_bounds(shell.screen_rect());
    let cy = bounds.y + bounds.height / 2.0;
    let target = centers.len() - 1; // last item

    move_mouse(&mut shell, centers[target], cy);
    shell.sync_dom();
    let scales = dock_item_scales(&shell);
    assert_eq!(
        peak_index(&scales),
        target,
        "peak must track the laid-out box under the cursor (icon_size=72): {scales:?}"
    );
}

fn move_mouse(shell: &mut Shell, x: f32, y: f32) {
    shell.handle_platform_event(&PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    });
}

// ───────────────────────── Dock position / size ─────────────────────────

#[test]
fn dock_honors_non_default_position_and_size() {
    let mut shell = Shell::new(1920.0, 1080.0);

    // Default = bottom dock; work area reserves vertical space only.
    let default_work = shell.work_area();
    assert!(
        (default_work.width - 1920.0).abs() < 0.5,
        "bottom dock should not eat horizontal space: {default_work:?}"
    );

    // Apply a non-default LEFT dock with an explicit thickness.
    let cfg = DockConfig {
        position: DockPosition::Left,
        thickness: Some(96),
        icon_size: 64,
        alignment: DockAlignment::Justified,
        ..DockConfig::default()
    };
    shell.set_dock_config(cfg);

    // The live dock took the new config...
    assert_eq!(shell.dock().config().position, DockPosition::Left);
    assert_eq!(shell.dock().config().icon_size, 64);

    // ...the dock bounds now span the LEFT edge (96px wide, full-ish height)...
    let bounds = shell.dock().compute_bounds(shell.screen_rect());
    assert!(
        (bounds.width - 96.0).abs() < 0.5,
        "left dock should be 96px wide: {bounds:?}"
    );
    assert!(bounds.x < 1.0, "left dock anchors at the left edge: {bounds:?}");

    // ...and the work area now reserves HORIZONTAL space (width shrinks by the
    // dock width) and shifts its x past the dock, not vertical space.
    let work = shell.work_area();
    assert!(
        (work.width - (1920.0 - 96.0)).abs() < 0.5,
        "left dock should reserve its width from the work area: {work:?}"
    );
    assert!(
        work.x >= 96.0 - 0.5,
        "work area should start past the left dock: {work:?}"
    );
}

#[test]
fn dock_right_position_reserves_width_without_x_shift() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.set_dock_config(DockConfig {
        position: DockPosition::Right,
        thickness: Some(80),
        ..DockConfig::default()
    });
    let work = shell.work_area();
    assert!(
        (work.width - (1920.0 - 80.0)).abs() < 0.5,
        "right dock reserves its width: {work:?}"
    );
    // A right dock does not push the work-area origin rightward.
    assert!(work.x < 1.0, "right dock keeps work-area x at 0: {work:?}");
}

#[test]
fn set_dock_config_persists_into_shell_config() {
    let mut shell = Shell::new(1280.0, 720.0);
    shell.set_dock_config(DockConfig {
        position: DockPosition::Top,
        show_labels: true,
        ..DockConfig::default()
    });
    // Mirrored into the canonical ShellConfig so a host save round-trips it.
    assert_eq!(shell.config().dock.position, DockPosition::Top);
    assert!(shell.config().dock.show_labels);
}

#[test]
fn dock_attributes_reflect_config_on_dom_sync() {
    let mut shell = Shell::new(1280.0, 720.0);
    shell.set_dock_config(DockConfig {
        position: DockPosition::Right,
        alignment: DockAlignment::Justified,
        show_labels: true,
        icon_size: 56,
        ..DockConfig::default()
    });
    // Drive the render-path DOM sync, then read the attrs off `#shell-dock`.
    shell.sync_dom();

    let doc = &shell.desktop_dom.doc;
    let dock = doc
        .get_element_by_id(crate::desktop_dom::element_ids::DOCK)
        .expect("dock element present");
    assert_eq!(
        doc.get_attribute(dock, "data-position").as_deref(),
        Some("right")
    );
    assert_eq!(
        doc.get_attribute(dock, "data-alignment").as_deref(),
        Some("justified")
    );
    assert_eq!(
        doc.get_attribute(dock, "data-show-labels").as_deref(),
        Some("true")
    );
    let style = doc.get_attribute(dock, "style").unwrap_or_default();
    assert!(
        style.contains("--dock-icon-size:56px"),
        "icon-size CSS var should be set: {style:?}"
    );
}

// ───────────────────────── Auto-hide reveal ─────────────────────────

#[test]
fn always_hidden_dock_reveals_on_edge_cursor() {
    let mut shell = Shell::new(1000.0, 800.0);
    shell.set_dock_config(DockConfig {
        position: DockPosition::Bottom,
        auto_hide_mode: AutoHideMode::AlwaysHidden,
        ..DockConfig::default()
    });
    // Starts hidden.
    assert!(!shell.dock().is_visible());

    // Cursor in the middle keeps it hidden.
    move_mouse(&mut shell, 500.0, 400.0);
    assert!(!shell.dock().is_visible());

    // Cursor at the very bottom edge reveals it.
    move_mouse(&mut shell, 500.0, 799.5);
    assert!(
        shell.dock().is_visible(),
        "bottom-edge cursor should reveal an always-hidden dock"
    );
}

// ───────────────────────── Cursor CSS seam ─────────────────────────

#[test]
fn cursor_theme_fed_from_css_color() {
    let mut shell = Shell::new(800.0, 600.0);

    // The CSS-resolved theme cursor color is what the seam feeds the renderer's
    // CursorTheme.fill — NOT the renderer's hardcoded white default.
    let css_color = shell.theme().cursor_color;
    let seam = shell.cursor_theme();
    assert_eq!(
        seam.fill, css_color,
        "cursor seam fill must come from the CSS-resolved theme color"
    );
    // No shape override: the live per-frame cursor shape (hover/drag driven) is
    // carried on the scene node, so the seam leaves it to the node.
    assert!(seam.shape_override.is_none());

    // The CSS-resolved cursor scale also flows into the seam (CursorTheme.scale).
    assert_eq!(
        shell.cursor_theme().scale,
        shell.theme().cursor_scale,
        "cursor seam scale must come from the CSS-resolved theme scale"
    );

    // Swapping the theme's cursor color + scale flows through to the seam.
    let mut theme = crate::theme::ShellTheme::default_dark();
    theme.cursor_color = Color::new(255, 0, 0, 255);
    theme.cursor_scale = 1.75;
    shell.set_theme(theme);
    assert_eq!(shell.cursor_theme().fill, Color::new(255, 0, 0, 255));
    assert_eq!(
        shell.cursor_theme().scale,
        1.75,
        "a restyled cursor scale must reach the renderer-facing CursorTheme"
    );
}
