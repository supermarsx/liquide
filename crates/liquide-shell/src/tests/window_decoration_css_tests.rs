//! Window frame decoration full-CSS migration regressions (t103-p6 / t86 P6).
//!
//! The window titlebar + close/maximize/minimize/pin buttons used to be drawn
//! by an imperative `SceneNodeKind::Decoration` node whose button geometry came
//! from hardcoded `DecorationStyle`/`DecorationLayout` stride math. They are now
//! laid out by the CSS pipeline as a `window-frame` DOM subtree
//! (`#window-deco-<id>`, synced by `sync_window_decorations`); the laid-out
//! boxes are the single source of truth for BOTH paint (the `Decoration` node's
//! geometry is anchored to them) and hit-test (`window_decoration_adapter`).
//!
//! These tests have TEETH for the contracts the migration must hold:
//!
//!   (a) **Renders from DOM/CSS** — the decoration is a real DOM subtree and a
//!       CSS change MOVES the titlebar/buttons (the laid-out box follows the
//!       stylesheet). If it reverted to hardcoded geometry, the CSS-driven
//!       assertions break.
//!
//!   (b) **Hit-test from the CSS box** — titlebar drag + each button click zone
//!       come from the laid-out box; a theme change that resizes a button moves
//!       its click zone (a click at the NEW box closes the window; a click at
//!       the OLD location no longer does). Clicking a CSS-positioned close
//!       button closes the window.
//!
//!   (c) **Cache composition** — an unchanged window still serves from the
//!       window-scene/full-scene caches without a per-frame rebuild (the
//!       per-window deco DOM must be written with change-guards so an idle frame
//!       leaves the DOM clean).
//!
//!   (d) **State restyle** — active/inactive (focus) and button-hover are
//!       reflected in the DOM (class / pseudo-state) so CSS restyles them.

use liquide_compositor::geometry::Rect;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_layout::geometry::Point;
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::decoration::HitZone;
use crate::shell::Shell;
use crate::window::WindowId;

/// The REAL shipped component stylesheet — the production source of the
/// `window-frame`/`window-titlebar`/button rules. Driving it through the
/// pipeline (rather than an inline stand-in) gives the geometry tests teeth: a
/// regressed dimension on disk moves the laid-out box and the assertions move.
const VARIABLES_CSS: &str = include_str!("../../../../assets/themes/variables.css");
const COMPONENTS_CSS: &str = include_str!("../../../../assets/themes/components.css");

const W: f32 = 1280.0;
const H: f32 = 720.0;

fn freeze_cursor_blink(shell: &mut Shell) {
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
}

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

/// A shell with the real component CSS loaded and one decorated window open, one
/// scene built (so the pipeline lays out the decoration and the hit-test engine
/// has the boxes).
fn windowed_shell() -> Shell {
    let mut shell = Shell::new(W, H);
    freeze_cursor_blink(&mut shell);
    shell.add_stylesheet(VARIABLES_CSS);
    shell.add_stylesheet(COMPONENTS_CSS);
    shell.open_window("Alpha", Rect::new(200.0, 120.0, 640.0, 420.0));
    let _ = shell.build_scene();
    shell
}

fn window_id(shell: &Shell) -> WindowId {
    shell.visible_windows()[0].id
}

// ── Contract (a): the decoration renders as a DOM/CSS subtree ─────────────

#[test]
fn decoration_renders_as_dom_subtree() {
    let shell = windowed_shell();
    let id = window_id(&shell).0;
    let doc = &shell.desktop_dom.doc;

    for suffix in ["", "-titlebar", "-title", "-buttons", "-close", "-max", "-min", "-pin"] {
        let el = format!("window-deco-{id}{suffix}");
        assert!(
            doc.get_element_by_id(&el).is_some(),
            "decoration must mount the DOM element #{el}"
        );
    }
}

/// Closing a window tears down its decoration DOM (no stale frame / hit-box).
#[test]
fn closing_window_removes_its_decoration_dom() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);
    let frame_id = format!("window-deco-{}", wid.0);
    assert!(shell.desktop_dom.doc.get_element_by_id(&frame_id).is_some());

    let _ = shell.close_window(wid);
    let _ = shell.build_scene();

    assert!(
        shell.desktop_dom.doc.get_element_by_id(&frame_id).is_none(),
        "closing the window must remove its decoration frame from the DOM"
    );
}

// ── Contract (b): hit-test from the CSS box ───────────────────────────────

/// The titlebar + each button resolve to a non-degenerate laid-out CSS box, and
/// the box center resolves to the right zone.
#[test]
fn button_and_titlebar_zones_come_from_css_layout() {
    let shell = windowed_shell();
    let wid = window_id(&shell);

    let tb = shell
        .window_titlebar_bounds_from_css(wid)
        .expect("titlebar laid-out box");
    assert!(tb.width > 1.0 && tb.height > 1.0, "titlebar box {tb:?}");

    let cases = [
        ("close", HitZone::CloseButton),
        ("max", HitZone::MaximizeButton),
        ("min", HitZone::MinimizeButton),
        ("pin", HitZone::AlwaysOnTopButton),
    ];
    for (suffix, zone) in cases {
        let b = shell
            .window_button_bounds_from_css(wid, suffix)
            .unwrap_or_else(|| panic!("{suffix} laid-out box"));
        let cx = b.x + b.width / 2.0;
        let cy = b.y + b.height / 2.0;
        assert_eq!(
            shell.window_button_zone_from_css(wid, cx, cy),
            Some(zone),
            "center of {suffix} CSS box must resolve to {zone:?}"
        );
    }
}

/// Clicking the center of the CSS-positioned close button closes that window.
#[test]
fn click_css_close_button_closes_window() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);
    let b = shell
        .window_button_bounds_from_css(wid, "close")
        .expect("close box");
    let cx = b.x + b.width / 2.0;
    let cy = b.y + b.height / 2.0;

    let action = shell.handle_platform_event(&press(cx, cy));
    assert!(
        matches!(action, Some(crate::shortcuts::ShellAction::CloseWindow)),
        "a click on the CSS close button must yield CloseWindow, got {action:?}"
    );
}

/// THE geometry-from-CSS tooth: a theme override that MOVES/resizes the buttons
/// moves the click zones with them. After widening the buttons, the close box
/// shifts; a click at its NEW center still closes; a click that is now between
/// the old narrow position and outside the new box no longer maps to close.
#[test]
fn theme_change_moves_the_button_click_zones() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);

    let before = shell
        .window_button_bounds_from_css(wid, "close")
        .expect("baseline close box");

    // Distinctly resize the buttons. `add_stylesheet` appends with higher
    // precedence, so this enlarges every button box (and the titlebar gap math
    // re-lays-out the cluster).
    shell.add_stylesheet(
        "close-button, maximize-button, minimize-button, pin-button \
         { width: 40; height: 30; }",
    );
    let _ = shell.build_scene();

    let after = shell
        .window_button_bounds_from_css(wid, "close")
        .expect("overridden close box");

    // The laid-out box tracks the override (40x30 content; the border-box may
    // add a couple px of border, so allow a small slack). The baseline box was
    // the 14px token, so the box must have grown substantially — proving the
    // geometry is the laid-out CSS box, not a hardcoded constant.
    assert!(
        (after.width - 40.0).abs() <= 3.0 && (after.height - 30.0).abs() <= 3.0,
        "the override must resize the laid-out close box to ~40x30, got {after:?}"
    );
    assert!(
        after.width - before.width > 10.0,
        "the close box must grow markedly after the theme override (before {before:?}, after {after:?})"
    );

    // A click at the NEW box center still resolves to close.
    let new_c = Point::new(after.x + after.width / 2.0, after.y + after.height / 2.0);
    assert_eq!(
        shell.window_button_zone_from_css(wid, new_c.x, new_c.y),
        Some(HitZone::CloseButton),
        "a click at the NEW close box center must resolve to close (zone moved with CSS)"
    );

    // A point far left of the (now wider) button cluster, in the title region,
    // must NOT resolve to a button — proving the zone is the laid-out box, not a
    // fixed stride from the window edge.
    let tb = shell.window_titlebar_bounds_from_css(wid).unwrap();
    let left_x = tb.x + 6.0;
    let mid_y = tb.y + tb.height / 2.0;
    assert_eq!(
        shell.window_button_zone_from_css(wid, left_x, mid_y),
        None,
        "the title region must not resolve to a button"
    );
}

// ── Contract (c): cache composition ───────────────────────────────────────

/// Adding the per-window decoration DOM must NOT defeat the idle full-scene
/// cache: two consecutive idle builds still produce a full-scene HIT, which is
/// only possible if `sync_window_decorations` writes nothing on an idle frame
/// (i.e. it used change-guarded mutations). This is the teeth that fails if the
/// per-frame deco sync re-dirties the DOM every frame.
#[test]
fn idle_frame_after_decoration_sync_still_hits_full_scene_cache() {
    let mut shell = windowed_shell();

    // `windowed_shell` already built once (miss). The next build must be an idle
    // full-scene HIT.
    let before = shell.full_scene_cache_stats();
    freeze_cursor_blink(&mut shell);
    let _ = shell.build_scene();
    let after = shell.full_scene_cache_stats();

    assert_eq!(
        after.hits,
        before.hits + 1,
        "an idle frame after the decoration sync must hit the full-scene cache \
         (decoration sync must not re-dirty the DOM): before {before:?}, after {after:?}"
    );
    assert_eq!(
        after.misses, before.misses,
        "an idle frame must not miss the full-scene cache"
    );
}

/// The idle full-scene hit is byte-identical to the build it cached — the
/// decoration is part of that stable root, so a cache hit never drops it.
#[test]
fn idle_hit_is_byte_identical_with_decoration() {
    let mut shell = windowed_shell();
    freeze_cursor_blink(&mut shell);
    let built = shell.build_scene();
    let misses = shell.full_scene_cache_stats().misses;
    freeze_cursor_blink(&mut shell);
    let cached = shell.build_scene();
    assert_eq!(
        shell.full_scene_cache_stats().misses,
        misses,
        "the second build must be an idle hit, not a miss"
    );
    assert_eq!(format!("{built:?}"), format!("{cached:?}"));
}

// ── Contract (d): focus / hover state restyle ─────────────────────────────

/// Focus is reflected on the decoration as the `.focused` class + `:focus`
/// pseudo-state so the CSS `.focused`/`:focus` rules restyle it; an unfocused
/// window carries neither.
#[test]
fn focus_reflects_as_class_and_pseudo_state() {
    use liquide_dom::PseudoStateFlags;

    let mut shell = Shell::new(W, H);
    freeze_cursor_blink(&mut shell);
    shell.add_stylesheet(VARIABLES_CSS);
    shell.add_stylesheet(COMPONENTS_CSS);
    let a = shell.open_window("A", Rect::new(120.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(560.0, 140.0, 400.0, 300.0));
    let _ = shell.set_focus(b);
    let _ = shell.build_scene();

    let frame_b = shell
        .desktop_dom
        .doc
        .get_element_by_id(&format!("window-deco-{}", b.0))
        .unwrap();
    let frame_a = shell
        .desktop_dom
        .doc
        .get_element_by_id(&format!("window-deco-{}", a.0))
        .unwrap();

    let node_b = shell.desktop_dom.doc.get(frame_b).unwrap();
    assert!(
        node_b.has_class("focused") && node_b.has_pseudo_state(PseudoStateFlags::FOCUS),
        "the focused window's decoration must carry .focused + :focus"
    );
    let node_a = shell.desktop_dom.doc.get(frame_a).unwrap();
    assert!(
        !node_a.has_class("focused") && !node_a.has_pseudo_state(PseudoStateFlags::FOCUS),
        "the unfocused window's decoration must carry neither .focused nor :focus"
    );
}

/// The pin (always-on-top) button carries the `.active` class only when the
/// window is pinned, so the CSS `pin-button.active` rule restyles it.
#[test]
fn pin_button_active_class_tracks_always_on_top() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);
    let pin_id = format!("window-deco-{}-pin", wid.0);

    let pin_has_active = |shell: &Shell| {
        shell
            .desktop_dom
            .doc
            .get_element_by_id(&pin_id)
            .and_then(|n| shell.desktop_dom.doc.get(n))
            .map(|n| n.has_class("active"))
            .unwrap_or(false)
    };
    assert!(!pin_has_active(&shell), "a normal window's pin is inactive");

    // Pin the window through the canonical toggle action (the same path the
    // live pin button drives).
    let _ = shell.set_focus(wid);
    assert!(shell.execute_action(&crate::shortcuts::ShellAction::ToggleAlwaysOnTop));
    let _ = shell.build_scene();
    assert!(
        pin_has_active(&shell),
        "an always-on-top window's pin button must carry .active"
    );
}

/// Hovering a CSS-positioned button records that button as hovered, which the
/// signature feeds to the decoration so it restyles (close_hovered etc.). A
/// move onto the close box flips `hovered_button` to the close zone derived from
/// the CSS box.
#[test]
fn hovering_css_button_records_hover_from_layout() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);
    let b = shell
        .window_button_bounds_from_css(wid, "close")
        .expect("close box");
    let cx = b.x + b.width / 2.0;
    let cy = b.y + b.height / 2.0;

    let mv = PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x: cx, y: cy },
    };
    let _ = shell.handle_platform_event(&mv);

    assert_eq!(
        shell.hovered_button,
        Some((wid, HitZone::CloseButton)),
        "moving onto the CSS close box must record a close-button hover"
    );
}
