//! Lock-screen full-CSS migration regressions (t95-p4 / t86 P4).
//!
//! The lock screen used to be drawn by an imperative painter
//! (`scene.rs::add_lockscreen_overlay`) with hardcoded rects/colors. It is now
//! a DOM/CSS overlay synced through `sync_lockscreen_template` and laid out by
//! the CSS pipeline (the `lockscreen*` rules in `assets/themes/components.css`).
//!
//! These tests have TEETH for the two contracts the migration must hold:
//!
//!   1. **Renders from DOM/CSS** — the lock surface is a real DOM subtree
//!      (`lockscreen-overlay` → clock / date / user / password field) and a CSS
//!      change MOVES it (the password-field box follows the stylesheet). If the
//!      surface reverted to hardcoded geometry, the CSS-driven assertions break.
//!
//!   2. **Hit-test from CSS geometry** — the password-field click/focus zone is
//!      read from the laid-out `#lockscreen-password` box, never a constant. A
//!      click inside the CSS-defined box focuses the field; a theme change that
//!      moves the box moves the click-zone with it (a click at the OLD location
//!      no longer focuses; a click at the NEW location does). This is the
//!      recurring hit-test-from-CSS-geometry requirement (t86).

use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_layout::geometry::Point;
use liquide_lockscreen::screen::ScreenPhase;
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::shell::Shell;

/// The REAL shipped component stylesheet — the production source of the
/// `lockscreen*` rules. Driving it through the pipeline (rather than an inline
/// stand-in) gives the tests teeth: if a `lockscreen-prompt` dimension regresses
/// on disk, the laid-out box moves and the geometry assertions fail.
const COMPONENTS_CSS: &str = include_str!("../../../../assets/themes/components.css");

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

/// A shell with the real component CSS loaded and the session locked.
fn locked_shell() -> Shell {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.add_stylesheet(COMPONENTS_CSS);
    shell.lock_session();
    assert!(shell.is_session_locked(), "session should be locked");
    // Build once so the DOM is synced and the pipeline lays out the overlay,
    // populating the hit-test engine with the lockscreen boxes.
    let _ = shell.build_scene();
    shell
}

/// Walk the lockscreen overlay subtree and collect (parent-tag -> text) pairs.
fn overlay_texts(shell: &Shell, overlay_id: &str) -> Vec<(String, String)> {
    let doc = &shell.desktop_dom.doc;
    let Some(overlay) = doc.get_element_by_id(overlay_id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut stack = vec![overlay];
    while let Some(node_id) = stack.pop() {
        if let Some(node) = doc.get(node_id) {
            if let Some(text) = node.text_content() {
                let parent_tag = doc
                    .parent(node_id)
                    .and_then(|p| doc.get(p))
                    .map(|p| p.tag_name())
                    .unwrap_or_default();
                out.push((parent_tag, text.to_string()));
            }
        }
        for &child in doc.children(node_id) {
            stack.push(child);
        }
    }
    out
}

// ── Contract 1: the lock screen is a DOM/CSS overlay ──────────────────────

/// Locking mounts the lock surface as a real DOM subtree — overlay scrim,
/// clock, user name, and a password field element — instead of an imperative
/// rect overlay. This is the structural proof the surface is DOM-driven.
#[test]
fn lockscreen_renders_as_dom_subtree() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // No overlay before locking.
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("lockscreen-overlay")
            .is_none(),
        "no lock overlay should exist before locking"
    );

    shell.lock_session();
    shell.sync_dom();

    let doc = &shell.desktop_dom.doc;
    assert!(
        doc.get_element_by_id("lockscreen-overlay").is_some(),
        "locking must mount the lockscreen-overlay DOM element"
    );
    assert!(
        doc.get_element_by_id("lockscreen-clock").is_some(),
        "lock overlay must contain a clock element"
    );
    assert!(
        doc.get_element_by_id("lockscreen-password").is_some(),
        "lock overlay must contain the password-field element"
    );

    // Unlocking removes the overlay from the DOM (no stale lock surface).
    // Drive the canonical state out of the locked phase directly (in-crate).
    shell.chrome_lockscreen.as_mut().unwrap().phase = ScreenPhase::Unlocking;
    assert!(!shell.is_session_locked());
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("lockscreen-overlay")
            .is_none(),
        "unlocking must remove the lock overlay from the DOM"
    );
}

/// The clock and user name paint as real text-bearing DOM nodes (the canonical
/// `layout_info()` content flows into the template), not opaque rects.
#[test]
fn lockscreen_clock_and_user_carry_real_text() {
    use liquide_lockscreen::{LockScreenAction, AuthBackend, AuthResult};

    // A fail-closed backend mirroring the shell's (auth is never reached here).
    #[derive(Default)]
    struct NoAuth;
    impl AuthBackend for NoAuth {
        fn authenticate(&self, _u: &str, _c: &str) -> AuthResult {
            AuthResult::Failed("n/a".into())
        }
    }

    let mut shell = Shell::new(1920.0, 1080.0);
    shell.lock_session();
    // The clock text is populated by the canonical Tick (the render/runtime
    // path drives this each frame). Drive one tick so the clock has content.
    shell
        .chrome_lockscreen
        .as_mut()
        .unwrap()
        .handle_action(LockScreenAction::Tick, &NoAuth);
    shell.sync_dom();

    let texts = overlay_texts(&shell, "lockscreen-overlay");
    // The default shell identity is display name "User".
    assert!(
        texts
            .iter()
            .any(|(tag, text)| tag == "lockscreen-user" && text == "User"),
        "user element must carry the display-name text, got {texts:?}"
    );
    // The clock element carries the canonical time text after a tick.
    assert!(
        texts
            .iter()
            .any(|(tag, text)| tag == "lockscreen-clock" && !text.is_empty()),
        "clock element must carry non-empty time text after a tick, got {texts:?}"
    );
}

/// The password field box comes from the CSS layout: with the real
/// `lockscreen-prompt` rule loaded the laid-out box matches the stylesheet
/// dimensions (width 280 / height 44). This proves the surface is laid out by
/// CSS, not painted at hardcoded geometry.
#[test]
fn password_field_box_comes_from_css_layout() {
    let shell = locked_shell();
    let bounds = shell
        .lockscreen_password_field_bounds()
        .expect("password field must have a laid-out CSS box while locked");

    // Dimensions are literals in the `lockscreen-prompt` rule; if that rule
    // regresses on disk these assertions move.
    assert!(
        (bounds.width - 280.0).abs() < 1.0,
        "password field width must come from the lockscreen-prompt CSS (280), got {}",
        bounds.width
    );
    assert!(
        (bounds.height - 44.0).abs() < 1.0,
        "password field height must come from the lockscreen-prompt CSS (44), got {}",
        bounds.height
    );
    // The cluster is centred, so the field sits roughly mid-screen, not at the
    // origin (a collapsed/unlaid-out box would be at 0,0).
    assert!(
        bounds.x > 100.0 && bounds.y > 100.0,
        "field must be laid out within the centred cluster, got {bounds:?}"
    );
}

// ── Contract 2: hit-test derives from the CSS box ─────────────────────────

/// A click INSIDE the CSS-defined password field focuses it (Clock →
/// PasswordEntry), while a click OUTSIDE the box does not. The focus zone is
/// the laid-out box, proving the hit-test reads CSS geometry.
#[test]
fn click_inside_css_password_box_focuses_field() {
    let mut shell = locked_shell();
    assert_eq!(
        shell.chrome_lockscreen.as_ref().unwrap().phase,
        ScreenPhase::Clock,
        "fresh lock starts in the clock phase (field unfocused)"
    );

    let field = shell
        .lockscreen_password_field_bounds()
        .expect("laid-out password box");
    let center = Point::new(
        field.x + field.width / 2.0,
        field.y + field.height / 2.0,
    );

    // A click well outside the box must NOT focus the field.
    shell.handle_platform_event(&press(field.x - 300.0, field.y - 300.0));
    assert_eq!(
        shell.chrome_lockscreen.as_ref().unwrap().phase,
        ScreenPhase::Clock,
        "a click outside the CSS password box must not focus the field"
    );

    // A click inside the CSS box focuses the field.
    shell.handle_platform_event(&press(center.x, center.y));
    assert_eq!(
        shell.chrome_lockscreen.as_ref().unwrap().phase,
        ScreenPhase::PasswordEntry,
        "a click inside the CSS password box must focus the field"
    );
}

/// THE geometry-from-CSS tooth: a theme override that MOVES/resizes the
/// password-field box moves the click-zone with it. A click at the box's NEW
/// location focuses the field; a click at the OLD location no longer does.
///
/// If the hit-test used a hardcoded constant instead of the laid-out box, the
/// click-zone would NOT follow the stylesheet and this test would fail.
#[test]
fn theme_change_moves_the_password_click_zone() {
    let mut shell = locked_shell();

    let before = shell
        .lockscreen_password_field_bounds()
        .expect("baseline password box");

    // Override the field to a distinctly different size. `add_stylesheet`
    // appends with higher precedence, so this widens/heightens the box.
    shell.add_stylesheet("lockscreen-prompt { width: 120; height: 120; }");
    let _ = shell.build_scene();

    let after = shell
        .lockscreen_password_field_bounds()
        .expect("overridden password box");

    // The box actually changed (the CSS override took effect).
    assert!(
        (after.width - 120.0).abs() < 1.0 && (after.height - 120.0).abs() < 1.0,
        "the override must resize the laid-out box, got {after:?}"
    );
    assert!(
        (after.width - before.width).abs() > 1.0,
        "the box must have a different width after the theme override"
    );

    // A point that was inside the OLD wide box but is now OUTSIDE the narrower
    // box must NOT focus the field — proving the zone tracks the NEW CSS box.
    // (Old width 280 → right edge ~ before.x+280; new width 120.)
    let old_only_x = before.x + before.width - 5.0; // inside old, outside new
    let mid_y = after.y + after.height / 2.0;
    if old_only_x > after.x + after.width {
        shell.handle_platform_event(&press(old_only_x, mid_y));
        assert_eq!(
            shell.chrome_lockscreen.as_ref().unwrap().phase,
            ScreenPhase::Clock,
            "a click in the OLD-but-not-NEW box region must not focus (zone moved with CSS)"
        );
    }

    // A click inside the NEW box focuses the field.
    let new_center = Point::new(after.x + after.width / 2.0, after.y + after.height / 2.0);
    shell.handle_platform_event(&press(new_center.x, new_center.y));
    assert_eq!(
        shell.chrome_lockscreen.as_ref().unwrap().phase,
        ScreenPhase::PasswordEntry,
        "a click inside the NEW CSS box must focus the field"
    );
}

/// While locked, a press anywhere is swallowed by the lock surface (modal): it
/// must not leak to windows/chrome behind the scrim. A press outside the field
/// returns a redraw and never focuses the field or opens chrome.
#[test]
fn locked_screen_swallows_presses_outside_the_field() {
    let mut shell = locked_shell();

    // A press far from the field (top-left corner) is consumed, not leaked.
    let action = shell.handle_platform_event(&press(5.0, 5.0));
    assert!(
        action.is_some(),
        "a press on the locked screen must be handled (swallowed), got {action:?}"
    );
    // The field stays unfocused (the corner is outside the CSS box).
    assert_eq!(
        shell.chrome_lockscreen.as_ref().unwrap().phase,
        ScreenPhase::Clock,
        "a corner press must not focus the field"
    );
    // No context menu / launcher leaked through the scrim.
    assert!(
        !shell.context_menu_visible,
        "a press while locked must not open the desktop context menu"
    );
}
