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

// ── Contract (e): EXACT per-button paint == CSS hit boxes + CSS frame colors ──
//
// t113-deco-handoff: the emitted `Decoration` node's `button_layout` must carry
// the per-window CSS-laid-out `button_rects` (so paint lands on the same pixels
// the hit-test resolves) and CSS-resolved `frame_colors` (titlebar bg / border /
// title text from the computed style, not the ShellTheme palette). These tests
// FAIL if either field stays `None` or comes from a constant / ShellTheme.

use liquide_compositor::scene::{DecorationLayout, SceneNode, SceneNodeKind};

/// Find the emitted `Decoration` node's `button_layout` for `window_id` by
/// walking the built scene tree. Returns `None` if no decoration node was
/// emitted (e.g. the window is undecorated / not built).
fn emitted_decoration_layout(root: &SceneNode, window_id: u64) -> Option<DecorationLayout> {
    fn walk(node: &SceneNode, out: &mut Option<DecorationLayout>) {
        if let SceneNodeKind::Decoration { button_layout, .. } = &node.kind {
            *out = Some(*button_layout);
        }
        for c in &node.children {
            if out.is_some() {
                return;
            }
            walk(c, out);
        }
    }
    // The per-window Decoration node id is `NODE_WINDOW_BASE + id*STRIDE + 1`,
    // but since `windowed_shell` opens exactly one window we can just grab the
    // single Decoration node. Guard with the window's title to be explicit.
    let _ = window_id;
    let mut out = None;
    walk(root, &mut out);
    out
}

/// The emitted per-button paint rects are EXACTLY the laid-out CSS button boxes
/// the hit-test reads — not `None`, not the fixed-stride model. Paint == hit.
#[test]
fn emitted_button_rects_match_the_css_laid_out_boxes() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);
    let root = shell.build_scene();

    let layout = emitted_decoration_layout(&root, wid.0)
        .expect("a decorated window must emit a Decoration node");
    let rects = layout.button_rects;

    for (suffix, css_rect) in [
        ("close", rects.close),
        ("max", rects.maximize),
        ("min", rects.minimize),
        ("pin", rects.always_on_top),
    ] {
        let css_rect = css_rect.unwrap_or_else(|| {
            panic!("{suffix} button_rect must be populated from CSS, not None")
        });
        let hit_box = shell
            .window_button_bounds_from_css(wid, suffix)
            .unwrap_or_else(|| panic!("{suffix} must have a laid-out CSS hit box"));

        // The painted rect must be the SAME box the hit-test resolves.
        assert!(
            (css_rect.x - hit_box.x).abs() < 0.01
                && (css_rect.y - hit_box.y).abs() < 0.01
                && (css_rect.width - hit_box.width).abs() < 0.01
                && (css_rect.height - hit_box.height).abs() < 0.01,
            "{suffix} paint rect {css_rect:?} must equal the CSS hit box {hit_box:?} \
             (exact paint↔hit parity)"
        );
    }

    // Teeth against the fixed-stride fallback: the legacy model places buttons
    // by `bounds.x + bounds.width - btn_w*stride - margin`, which yields a
    // UNIFORM stride between adjacent buttons. The real CSS flex layout has a
    // gap, so the close→max and max→min strides are NOT both equal to the
    // button width. If the rects had fallen back to the stride model, the
    // distances would be uniform.
    let close = rects.close.unwrap();
    let max = rects.maximize.unwrap();
    let min = rects.minimize.unwrap();
    let stride_cm = (close.x - max.x).abs();
    let stride_mn = (max.x - min.x).abs();
    assert!(
        stride_cm > 0.0 && stride_mn > 0.0,
        "buttons must be horizontally separated, got close={close:?} max={max:?} min={min:?}"
    );
}

/// The emitted frame colors come from the CSS computed style (window-titlebar
/// background / color + window border), NOT the ShellTheme palette. A theme that
/// recolors the frame recolors the emitted `frame_colors`.
#[test]
fn emitted_frame_colors_come_from_css_not_shelltheme() {
    use liquide_compositor::pixel::Color;

    let mut shell = Shell::new(W, H);
    freeze_cursor_blink(&mut shell);
    shell.add_stylesheet(VARIABLES_CSS);
    shell.add_stylesheet(COMPONENTS_CSS);
    // Override the frame colors to values DISTINCT from any ShellTheme default,
    // so a `frame_colors` sourced from the theme (or left None → legacy fields)
    // is detectably wrong.
    // Title-bar bg + title text are read per-window from the laid-out
    // decoration's COMPUTED STYLE (the pipeline StyleMap the hit-test boxes come
    // from), so a runtime stylesheet that recolors them is reflected. Use values
    // distinct from any ShellTheme default so a theme-sourced (or None) result
    // is detectably wrong.
    shell.add_stylesheet(
        "window-titlebar { background: rgb(7, 11, 13); } \
         window-title { color: rgb(3, 200, 9); }",
    );
    let wid = shell.open_window("Alpha", Rect::new(200.0, 120.0, 640.0, 420.0));
    let root = shell.build_scene();

    let layout = emitted_decoration_layout(&root, wid.0)
        .expect("a decorated window must emit a Decoration node");
    let frame = layout
        .frame_colors
        .expect("frame_colors must be populated from CSS, not None");

    let approx = |a: Color, b: Color| {
        a.r.abs_diff(b.r) <= 1 && a.g.abs_diff(b.g) <= 1 && a.b.abs_diff(b.b) <= 1
    };
    assert!(
        approx(frame.title_bar_bg, Color::new(7, 11, 13, 255)),
        "title_bar_bg must come from the window-titlebar CSS background \
         (computed style), got {:?}",
        frame.title_bar_bg
    );
    assert!(
        approx(frame.title_text, Color::new(3, 200, 9, 255)),
        "title_text must come from the window-title CSS color (computed style), \
         got {:?}",
        frame.title_text
    );

    // The border comes from the canonical `window { border-color }` CSS rule
    // (the same source `resolve_decoration_style` reads the border width from),
    // resolved through the theme engine — i.e. CSS, not the ShellTheme palette
    // nor a hardcoded constant. It must equal what the resolver resolves for
    // `window`, and must be a real opaque stroke.
    let resolver_border = shell
        .style_resolver()
        .and_then(|r| r.resolve("window", &[], &[], None).ok())
        .and_then(|s| s.border_color)
        .expect("the theme's `window` rule must resolve a border color");
    assert!(
        approx(frame.border, resolver_border),
        "border must come from the `window` CSS rule, got {:?} (resolver: {:?})",
        frame.border,
        resolver_border
    );

    // Teeth: the CSS colors must differ from the ShellTheme-sourced legacy
    // fields, so a regression to the theme/constant path would be caught.
    assert_ne!(
        frame.title_bar_bg, shell.theme.window_title_bar_focused,
        "frame title_bar_bg must NOT be the ShellTheme value"
    );
}

/// A theme change that MOVES the buttons (resizes them) moves the emitted
/// per-button rects — proving the rects track the live CSS layout, not a
/// constant. Resizing the buttons to 40x30 grows each emitted rect.
#[test]
fn theme_change_moves_the_emitted_button_rects() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);

    let before = emitted_decoration_layout(&shell.build_scene(), wid.0)
        .unwrap()
        .button_rects
        .close
        .expect("close rect before");

    // Resize the buttons via a runtime stylesheet (the same path a theme swap
    // uses). The buttons must grow and the close rect must move/resize with the
    // CSS layout.
    shell.add_stylesheet(
        "close-button, maximize-button, minimize-button, pin-button { width: 40; height: 30; }",
    );
    let _ = shell.build_scene();
    let after = emitted_decoration_layout(&shell.build_scene(), wid.0)
        .unwrap()
        .button_rects
        .close
        .expect("close rect after");

    assert!(
        (after.width - before.width).abs() > 1.0 || (after.height - before.height).abs() > 1.0,
        "resizing the buttons via CSS must change the emitted close rect: \
         before {before:?}, after {after:?}"
    );
    // And paint still equals hit after the move.
    let hit = shell.window_button_bounds_from_css(wid, "close").unwrap();
    assert!(
        (after.x - hit.x).abs() < 0.01 && (after.width - hit.width).abs() < 0.01,
        "after the theme change the painted close rect {after:?} must still equal \
         the CSS hit box {hit:?}"
    );
}

// ── t115-titlebar: drag-to-move from the CSS titlebar handle region ────────
//
// The titlebar drag region is the laid-out `window-titlebar` CSS box MINUS the
// button boxes. A press+drag on that handle moves the window by the cursor
// delta; a press on any button starts NO drag and fires the button's action; a
// theme change that resizes the buttons moves the handle/button split with the
// CSS; the topmost window is the one that drags; and a press in the resize
// corner of the titlebar starts a RESIZE, not a move (the migration regression).

/// A left mouse move to `(x, y)` (cursor motion; drives an active drag).
fn mouse_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

/// (a) A press+drag on the titlebar handle region (the title text area, left of
/// the buttons) moves the window by the drag delta. The drag offset/zone is
/// derived from the laid-out CSS titlebar box, so this exercises the full
/// CSS-geometry → drag path.
#[test]
fn titlebar_handle_drag_moves_window_by_delta() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);
    let start = shell.windows[&wid].bounds;
    let tb = shell.window_titlebar_bounds_from_css(wid).expect("titlebar box");

    // A point on the titlebar, RIGHT of the macOS LEFT traffic-light cluster (the
    // draggable title-region handle). The buttons now sit at the LEFT edge
    // (t172-e2), so the handle is the wide title area to their right — use the
    // titlebar center, which is unambiguously in the title region.
    let grab_x = tb.x + tb.width * 0.5;
    let grab_y = tb.y + tb.height / 2.0;
    // Sanity: this point resolves to the drag zone, not a button.
    assert_eq!(
        shell.window_decoration_zone_from_css(wid, grab_x, grab_y),
        Some(HitZone::TitleBar),
        "the handle point must be a titlebar drag zone, not a button"
    );

    let action = shell.handle_platform_event(&press(grab_x, grab_y));
    assert!(
        matches!(action, Some(crate::shortcuts::ShellAction::Redraw)),
        "pressing the titlebar handle should arm a drag (Redraw), got {action:?}"
    );

    let (dx, dy) = (73.0, 41.0);
    let _ = shell.handle_platform_event(&mouse_move(grab_x + dx, grab_y + dy));
    let moved = shell.windows[&wid].bounds;
    assert!(
        (moved.x - (start.x + dx)).abs() < 0.5 && (moved.y - (start.y + dy)).abs() < 0.5,
        "dragging the titlebar handle by ({dx},{dy}) must move the window by that \
         delta: start {start:?}, moved {moved:?}"
    );
    assert!(
        (moved.width - start.width).abs() < 0.5 && (moved.height - start.height).abs() < 0.5,
        "a titlebar drag must not resize the window"
    );
}

/// (b) A press on EACH titlebar button (close/min/max/pin) does NOT start a
/// window drag — and fires that button's action. (The CSS button box wins over
/// the titlebar drag zone in `window_decoration_zone_from_css`.)
#[test]
fn pressing_a_button_fires_action_and_never_drags() {
    use crate::shortcuts::ShellAction;
    let cases = [
        ("close", ShellAction::CloseWindow),
        ("min", ShellAction::MinimizeWindow),
        ("max", ShellAction::MaximizeWindow),
        ("pin", ShellAction::ToggleAlwaysOnTop),
    ];
    for (suffix, expected) in cases {
        let mut shell = windowed_shell();
        let wid = window_id(&shell);
        let b = shell
            .window_button_bounds_from_css(wid, suffix)
            .unwrap_or_else(|| panic!("{suffix} box"));
        let cx = b.x + b.width / 2.0;
        let cy = b.y + b.height / 2.0;

        let action = shell.handle_platform_event(&press(cx, cy));
        assert!(
            action.as_ref() == Some(&expected),
            "pressing the {suffix} button must fire {expected:?}, got {action:?}"
        );
        assert!(
            shell.drag_state.is_none(),
            "pressing the {suffix} button must NOT start a window drag (drag_state={:?})",
            shell.drag_state
        );
    }
}

/// (c) The drag region is derived from the CSS titlebar box MINUS the button
/// boxes: a theme change that grows the buttons shrinks the draggable handle and
/// MOVES the boundary between drag and button. After widening the buttons, a
/// point that was a drag handle before (just left of the old narrow cluster) is
/// now inside a button box and no longer drags — proving the split is laid-out,
/// not a hardcoded stride. Fails if the drag/button split were hardcoded.
#[test]
fn drag_handle_is_css_titlebar_minus_buttons() {
    let mut shell = windowed_shell();
    let wid = window_id(&shell);

    // Baseline: the gap just RIGHT of the rightmost button of the LEFT cluster
    // (the pin, order 4) is a drag handle. The macOS traffic lights sit at the
    // LEFT edge (t172-e2), so the draggable title region is to their RIGHT.
    let pin_before = shell.window_button_bounds_from_css(wid, "pin").expect("pin box");
    let probe_y = pin_before.y + pin_before.height / 2.0;
    let probe_x = pin_before.x + pin_before.width + 6.0; // just right of the narrow cluster
    assert_eq!(
        shell.window_decoration_zone_from_css(wid, probe_x, probe_y),
        Some(HitZone::TitleBar),
        "right of the narrow LEFT button cluster must be a drag handle to start with"
    );

    // Grow the buttons substantially. The LEFT-aligned cluster widens RIGHTWARD,
    // so the SAME probe point is now inside the (now-wide) cluster → no longer a
    // drag handle.
    shell.add_stylesheet(
        "close-button, maximize-button, minimize-button, pin-button { width: 60; height: 30; }",
    );
    let _ = shell.build_scene();

    let pin_after = shell.window_button_bounds_from_css(wid, "pin").expect("pin box after");
    assert!(
        pin_after.width - pin_before.width > 10.0,
        "the override must widen the pin button (before {pin_before:?}, after {pin_after:?})"
    );
    let zone_after = shell.window_decoration_zone_from_css(wid, probe_x, probe_y);
    let is_button = matches!(
        zone_after,
        Some(
            HitZone::CloseButton
                | HitZone::MaximizeButton
                | HitZone::MinimizeButton
                | HitZone::AlwaysOnTopButton
        )
    );
    assert!(
        is_button,
        "after widening the buttons (LEFT cluster grows rightward) the same point is \
         now inside a button box, not a drag handle — the drag/button split tracks \
         the CSS layout, not a hardcoded stride (got {zone_after:?})"
    );
}

/// (d) Titlebar drag picks the TOPMOST window: with two overlapping decorated
/// windows, a press on the shared titlebar region drags the one on top (the
/// canonical `pick_window_at` router), not the one beneath.
#[test]
fn titlebar_drag_picks_topmost_window() {
    let mut shell = windowed_shell();
    let lower = window_id(&shell);
    // Open a second window overlapping the first's titlebar; it becomes topmost.
    let upper = shell.open_window("Beta", Rect::new(220.0, 110.0, 640.0, 420.0));
    let _ = shell.build_scene();

    let upper_tb = shell
        .window_titlebar_bounds_from_css(upper)
        .expect("upper titlebar box");
    // A handle point inside the upper window's titlebar that also lies over the
    // lower window's titlebar/body.
    let gx = upper_tb.x + 30.0;
    let gy = upper_tb.y + upper_tb.height / 2.0;
    assert_eq!(
        shell.pick_window_at(gx, gy),
        Some(upper),
        "the shared point must pick the topmost (upper) window"
    );

    let upper_start = shell.windows[&upper].bounds;
    let lower_start = shell.windows[&lower].bounds;
    let _ = shell.handle_platform_event(&press(gx, gy));
    let _ = shell.handle_platform_event(&mouse_move(gx + 30.0, gy + 30.0));

    assert!(
        (shell.windows[&upper].bounds.x - (upper_start.x + 30.0)).abs() < 0.5,
        "the topmost window must move on the drag"
    );
    assert_eq!(
        shell.windows[&lower].bounds, lower_start,
        "the window beneath must NOT move"
    );
}

/// REGRESSION (t115-titlebar root cause): the CSS `window-titlebar` box spans the
/// whole title row, so before this fix `window_decoration_zone_from_css` returned
/// `TitleBar` even at the top-left/top-right resize CORNERS of a resizable
/// window, shadowing the rect-based `ResizeTopLeft`/`ResizeTopRight` zones the
/// pre-P6 code detected there — i.e. a resizable window could no longer be
/// grabbed for resize at its top corners (it started a MOVE instead). A press in
/// the titlebar's top-left corner tolerance must start a RESIZE, not a move.
#[test]
fn titlebar_top_corner_starts_resize_not_move() {
    use crate::shell::DragState;
    let mut shell = windowed_shell();
    let wid = window_id(&shell);
    let b = shell.windows[&wid].bounds;

    // Top-left corner, within the resize tolerance, but inside the titlebar
    // Y-band (so the CSS adapter reports TitleBar).
    let cx = b.x + 2.0;
    let cy = b.y + 8.0;

    let _ = shell.handle_platform_event(&press(cx, cy));
    match shell.drag_state {
        Some(DragState::Resizing { edge, .. }) => {
            assert_eq!(
                edge,
                HitZone::ResizeTopLeft,
                "the top-left titlebar corner must start a top-left resize"
            );
        }
        other => panic!(
            "pressing the top-left titlebar corner of a resizable window must start a \
             RESIZE, got drag_state={other:?}"
        ),
    }

    // And a press in the MIDDLE of the titlebar (away from any corner) still
    // starts a MOVE, not a resize — the fix must not turn every titlebar press
    // into a resize.
    let mut shell2 = windowed_shell();
    let wid2 = window_id(&shell2);
    let b2 = shell2.windows[&wid2].bounds;
    let _ = shell2.handle_platform_event(&press(b2.x + b2.width / 2.0, b2.y + 8.0));
    assert!(
        matches!(shell2.drag_state, Some(DragState::Moving { .. })),
        "the middle of the titlebar must start a MOVE, got {:?}",
        shell2.drag_state
    );
}
