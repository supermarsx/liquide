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
