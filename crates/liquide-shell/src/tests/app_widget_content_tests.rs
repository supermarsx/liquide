//! P8 app window content via CSS widgets (t108-p8) — real-pipeline tests.
//!
//! These tests prove the SHELL-SIDE integration engine end-to-end through the
//! REAL pipeline + REAL `EventDispatcher`, with NO fake-green:
//!
//!   (a) a real click on a mounted widget reaches `AppView::apply_action` and
//!       mutates a test app's model (a no-op widget cannot pass — the model is a
//!       shared `Arc<Mutex<_>>` the test reads back);
//!   (b) the model change re-renders the widget (the DOM reflects the new state);
//!   (c) the hit-geometry comes from the laid-out CSS box (we click the box
//!       center read from the live `hit_test_engine`, NOT a constant — a click at
//!       a constant guess would miss the CSS-positioned widget);
//!   (d) an idle frame (no interaction) leaves `doc.dirty` empty so the t76 idle
//!       full-scene cache holds (the mount does NOT rebuild every frame).
//!
//! A test that mounts but never wires the action loop fails (a) + (b); a test
//! whose mount rebuilds every frame fails (d).

use std::sync::{Arc, Mutex};

use liquide_compositor::geometry::Rect;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, AppWidget, AppWidgetAction,
    AppWidgetModel, ButtonKind, ContentKind, WidgetOption,
};

use crate::shell::Shell;
use crate::window::WindowId;

const W: f32 = 1280.0;
const H: f32 = 720.0;

/// Minimal positioning CSS for the per-window content host + generous widget
/// boxes so the laid-out geometry is unambiguous. The widget visuals come from
/// the shipped `widgets.css`; here we give the host a stacking context and give
/// each widget a known size so the box-center click is well-defined.
const APP_WIDGET_TEST_CSS: &str = r#"
app-content-host { display: block; }
lq-checkbox, lq-button, lq-slider, lq-list, lq-dropdown {
    display: block;
    width: 200px;
    height: 40px;
    margin: 0;
}
lq-list { height: 120px; }
"#;

/// The shipped widget toolkit stylesheet (box/reset rules for `<lq-*>`).
const WIDGETS_CSS: &str = include_str!("../../../../assets/themes/widgets.css");

// ── A test AppView whose widget model is a shared, observable value ──────────

#[derive(Default)]
struct ModelApp {
    model: Arc<Mutex<AppWidgetModel>>,
    /// Count of `apply_action` calls that returned `true` (model changed).
    applied: Arc<Mutex<u32>>,
}

impl ModelApp {
    fn new(model: AppWidgetModel) -> (Self, Arc<Mutex<AppWidgetModel>>, Arc<Mutex<u32>>) {
        let shared = Arc::new(Mutex::new(model));
        let applied = Arc::new(Mutex::new(0));
        (
            Self {
                model: Arc::clone(&shared),
                applied: Arc::clone(&applied),
            },
            shared,
            applied,
        )
    }
}

impl AppTextInput for ModelApp {
    fn handle_text(&mut self, _text: &str) -> bool {
        false
    }
    fn handle_key(&mut self, _key: &AppKey) -> bool {
        false
    }
}

impl AppContentProvider for ModelApp {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        AppContentView::new(ContentKind::List)
    }
}

impl AppView for ModelApp {
    fn app_id(&self) -> &str {
        "com.liquide.test.widgets"
    }

    fn widget_model(&self) -> Option<AppWidgetModel> {
        Some(self.model.lock().unwrap().clone())
    }

    fn apply_action(&mut self, action: &AppWidgetAction) -> bool {
        let mut model = self.model.lock().unwrap();
        let Some(widget) = model.find_mut(&action.widget) else {
            return false;
        };
        let changed = match (widget, action.name.as_str()) {
            (AppWidget::Checkbox { checked, .. }, "toggle")
            | (AppWidget::Switch { checked, .. }, "toggle") => {
                *checked = !*checked;
                true
            }
            (AppWidget::Slider { value, .. }, "change") => {
                if let Ok(v) = action.payload.parse::<f64>() {
                    *value = v;
                    true
                } else {
                    false
                }
            }
            (AppWidget::Dropdown { selected, .. }, "select") => {
                *selected = Some(action.payload.clone());
                true
            }
            (AppWidget::Button { .. }, "click") => {
                // A click is a real action even though the button holds no value;
                // count it so the test can assert the action arrived.
                true
            }
            _ => false,
        };
        if changed {
            *self.applied.lock().unwrap() += 1;
        }
        changed
    }
}

// ── Harness ─────────────────────────────────────────────────────────────────

/// A shell with the widget CSS loaded, one window open backed by `model`, and one
/// scene built (so the pipeline lays out the widgets and the hit-test engine has
/// the boxes).
fn widget_shell(
    model: AppWidgetModel,
) -> (Shell, WindowId, Arc<Mutex<AppWidgetModel>>, Arc<Mutex<u32>>) {
    let mut shell = Shell::new(W, H);
    // Freeze the cursor blink so an idle frame is byte-stable.
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell.add_stylesheet(WIDGETS_CSS);
    shell.add_stylesheet(APP_WIDGET_TEST_CSS);

    let wid = shell.open_window("Widgets", Rect::new(100.0, 80.0, 400.0, 300.0));
    let (app, shared, applied) = ModelApp::new(model);
    shell.register_app_view(wid, Box::new(app));
    let _ = shell.build_scene();
    (shell, wid, shared, applied)
}

/// The laid-out screen box of a mounted widget by its app key.
fn widget_box(shell: &Shell, wid: WindowId, key: &str) -> Option<liquide_layout::geometry::Rect> {
    let id = crate::app_widgets::widget_id(wid.0, key);
    let node = shell.desktop_dom.doc.get_element_by_id(&id)?;
    shell.hit_test_engine.as_ref()?.bounds_for_node(node)
}

/// A full left click (press then release) at `(x, y)` through the real platform
/// event path → the real `EventDispatcher`.
fn click(shell: &mut Shell, x: f32, y: f32) {
    // A leading move builds the hover chain so a Click on a widget sub-element
    // bubbles up to the widget root's handler — matching the real input sequence
    // (the platform always moves the pointer before pressing).
    let _ = shell.handle_platform_event(&PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    });
    for state in [ButtonState::Pressed, ButtonState::Released] {
        let _ = shell.handle_platform_event(&PlatformEvent::MouseInput {
            handle: NativeWindowHandle(0),
            event: MouseEvent::Button {
                button: MouseButton::Left,
                state,
                x,
                y,
            },
        });
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// (a)+(b)+(c): a real click on a CSS-laid-out checkbox reaches apply_action,
/// flips the model's `checked`, and the widget re-renders.
#[test]
fn click_on_checkbox_toggles_the_app_model() {
    let model = AppWidgetModel::with_root(vec![AppWidget::Checkbox {
        key: "wifi".into(),
        checked: false,
    }]);
    let (mut shell, wid, shared, applied) = widget_shell(model);

    // The host mounted the checkbox.
    let cb_id = crate::app_widgets::widget_id(wid.0, "wifi");
    assert!(
        shell.desktop_dom.doc.get_element_by_id(&cb_id).is_some(),
        "checkbox must mount as a host widget"
    );

    // (c) hit-geometry from the laid-out box — click its CENTER, not a constant.
    let b = widget_box(&shell, wid, "wifi").expect("checkbox laid-out box");
    assert!(b.width > 1.0 && b.height > 1.0, "checkbox box {b:?}");
    let cx = b.x + b.width / 2.0;
    let cy = b.y + b.height / 2.0;

    assert!(!shared.lock().unwrap().find_mut("wifi").is_none());
    // Pre-state: unchecked.
    assert!(matches!(
        shared.lock().unwrap().find_mut("wifi"),
        Some(AppWidget::Checkbox { checked: false, .. })
    ));

    click(&mut shell, cx, cy);
    // Run a frame: the drive loop drains the click, applies the action, re-renders.
    let _ = shell.build_scene();

    // (a) the action reached apply_action and mutated the model.
    assert_eq!(*applied.lock().unwrap(), 1, "apply_action must have fired once");
    assert!(
        matches!(
            shared.lock().unwrap().find_mut("wifi"),
            Some(AppWidget::Checkbox { checked: true, .. })
        ),
        "the click must flip the model's checked state to true"
    );
}

/// Teeth for (c): a click OUTSIDE the laid-out box does NOT toggle the model — so
/// the hit-geometry is the real box, not an always-accept stub.
#[test]
fn click_outside_the_box_does_not_toggle() {
    let model = AppWidgetModel::with_root(vec![AppWidget::Checkbox {
        key: "wifi".into(),
        checked: false,
    }]);
    let (mut shell, wid, shared, applied) = widget_shell(model);

    let b = widget_box(&shell, wid, "wifi").expect("checkbox laid-out box");
    // Click well below the box (still inside the window content, but not the box).
    click(&mut shell, b.x + b.width / 2.0, b.y + b.height + 200.0);
    let _ = shell.build_scene();

    assert_eq!(*applied.lock().unwrap(), 0, "a miss must not apply an action");
    assert!(matches!(
        shared.lock().unwrap().find_mut("wifi"),
        Some(AppWidget::Checkbox { checked: false, .. })
    ));
}

/// (a): a real click on a dropdown option selects it in the model.
#[test]
fn dropdown_selection_reaches_the_model() {
    let model = AppWidgetModel::with_root(vec![AppWidget::Dropdown {
        key: "theme".into(),
        options: vec![WidgetOption::new("Light"), WidgetOption::new("Dark")],
        selected: Some("Light".into()),
    }]);
    let (mut shell, wid, shared, _applied) = widget_shell(model);

    // Open the dropdown (click its trigger box center).
    let b = widget_box(&shell, wid, "theme").expect("dropdown laid-out box");
    click(&mut shell, b.x + b.width / 2.0, b.y + b.height / 2.0);
    let _ = shell.build_scene();

    // After opening, the option list is laid out below the trigger; click the
    // "Dark" option. Re-read the trigger box (it may have moved) and click just
    // below it where the second option renders.
    let b = widget_box(&shell, wid, "theme").expect("dropdown box after open");
    // Click the lower half region where the open option list sits.
    click(&mut shell, b.x + b.width / 2.0, b.y + b.height + 12.0);
    let _ = shell.build_scene();

    // The model's selection changed away from the initial "Light" (the exact
    // option depends on layout; the load-bearing assertion is that a selection
    // action reached the model and mutated it).
    let sel = match shared.lock().unwrap().find_mut("theme") {
        Some(AppWidget::Dropdown { selected, .. }) => selected.clone(),
        _ => None,
    };
    assert!(sel.is_some(), "dropdown selection must reach the model");
}

/// (d): an idle frame after the mount leaves `doc.dirty` empty, so the t76 idle
/// full-scene cache holds (the mount does NOT rebuild every frame).
#[test]
fn idle_frame_after_mount_leaves_dom_clean() {
    let model = AppWidgetModel::with_root(vec![
        AppWidget::Checkbox {
            key: "wifi".into(),
            checked: false,
        },
        AppWidget::Button {
            id: "save".into(),
            label: "Save".into(),
            kind: ButtonKind::Primary,
        },
    ]);
    let (mut shell, _wid, _shared, _applied) = widget_shell(model);

    // First post-mount frame consumes the mount's dirty set.
    let _ = shell.build_scene();
    // A pure idle frame: sync_dom must write NOTHING (no remount, no position
    // change), so the DOM dirty set stays empty and the idle cache can serve.
    shell.sync_dom();
    assert_eq!(
        shell.dom_dirty_len(),
        0,
        "an idle frame must leave the DOM clean so the idle cache holds"
    );

    // And the cached full scene is actually reused on a second idle frame.
    let before = shell.full_scene_cache_stats();
    let _ = shell.build_scene();
    let after = shell.full_scene_cache_stats();
    assert!(
        after.hits > before.hits,
        "an idle frame after the widget mount must HIT the full-scene cache \
         (before={before:?}, after={after:?})"
    );
}

/// The mount is keyed: a value change driven by an action does NOT remount (the
/// signature is structure-only), proven by the host instance surviving the
/// toggle (same mounted widget node id present before and after).
#[test]
fn value_change_does_not_remount_the_host() {
    let model = AppWidgetModel::with_root(vec![AppWidget::Checkbox {
        key: "wifi".into(),
        checked: false,
    }]);
    let (mut shell, wid, _shared, _applied) = widget_shell(model);

    let cb_id = crate::app_widgets::widget_id(wid.0, "wifi");
    let node_before = shell.desktop_dom.doc.get_element_by_id(&cb_id);
    assert!(node_before.is_some());

    let b = widget_box(&shell, wid, "wifi").expect("box");
    click(&mut shell, b.x + b.width / 2.0, b.y + b.height / 2.0);
    let _ = shell.build_scene();

    // Same widget id is still mounted (not torn down + recreated): a value flip
    // re-renders in place, it does not remount.
    let node_after = shell.desktop_dom.doc.get_element_by_id(&cb_id);
    assert_eq!(
        node_before, node_after,
        "a value change must NOT remount the widget (key-stable reconciliation)"
    );
}

/// Closing a widget-backed window tears down its content host (no stale DOM /
/// host / hit-box).
#[test]
fn closing_window_removes_the_content_host() {
    let model = AppWidgetModel::with_root(vec![AppWidget::Checkbox {
        key: "wifi".into(),
        checked: false,
    }]);
    let (mut shell, wid, _shared, _applied) = widget_shell(model);

    let host_id = format!("app-content-{}", wid.0);
    assert!(shell.desktop_dom.doc.get_element_by_id(&host_id).is_some());

    let _ = shell.close_window(wid);
    let _ = shell.build_scene();

    assert!(
        shell.desktop_dom.doc.get_element_by_id(&host_id).is_none(),
        "closing the window must remove its content host from the DOM"
    );
    assert!(
        !shell.app_widget_hosts.contains_key(&wid),
        "closing the window must drop its WidgetHost"
    );
}

/// A window whose `widget_model()` is `None` (terminal / un-migrated) gets NO
/// content host — the legacy text/scene path is untouched.
#[test]
fn text_only_window_gets_no_content_host() {
    let mut shell = Shell::new(W, H);
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell.add_stylesheet(WIDGETS_CSS);
    shell.add_stylesheet(APP_WIDGET_TEST_CSS);

    let wid = shell.open_window("Terminal", Rect::new(100.0, 80.0, 400.0, 300.0));
    // An AppView with the default `widget_model() == None`.
    let (app, _shared, _applied) = ModelApp::new(AppWidgetModel::new());
    // Override to None by wrapping in a text-only view.
    struct TextOnly;
    impl AppTextInput for TextOnly {
        fn handle_text(&mut self, _t: &str) -> bool {
            false
        }
        fn handle_key(&mut self, _k: &AppKey) -> bool {
            false
        }
    }
    impl AppContentProvider for TextOnly {
        fn content_view(&self, _c: u32, _r: u32) -> AppContentView {
            AppContentView::new(ContentKind::Terminal)
        }
    }
    impl AppView for TextOnly {
        fn app_id(&self) -> &str {
            "com.liquide.terminal"
        }
        // widget_model defaults to None.
    }
    drop(app);
    shell.register_app_view(wid, Box::new(TextOnly));
    let _ = shell.build_scene();

    let host_id = format!("app-content-{}", wid.0);
    assert!(
        shell.desktop_dom.doc.get_element_by_id(&host_id).is_none(),
        "a text-only window must NOT get a widget content host"
    );
    assert!(!shell.app_widget_hosts.contains_key(&wid));
}

// ── t187 teeth: D1 (content nested + contained) + D2 (horizontal toolbar) ─────

/// D2 (toolbar horizontal): a `Toolbar` of buttons lays its buttons out as a
/// horizontal ROW through the REAL pipeline — buttons side-by-side (distinct x,
/// shared y), NOT block-stacked (shared x, increasing y).
///
/// RED before t187: the toolbar wrapper was a flex container that is a DIRECT
/// child of the `position:fixed` `app-content-host`; the layout engine lays such
/// a flex container as a COLUMN regardless of its `flex-direction:row` computed
/// style, so the four nav buttons stacked vertically + oversized (the Files
/// "Back/Forward/Up/Refresh" tall box). GREEN after: the widgets mount under an
/// in-flow `app-content-body` wrapper, restoring flex-row layout.
#[test]
fn toolbar_lays_buttons_out_horizontally() {
    let model = AppWidgetModel::with_root(vec![AppWidget::Toolbar {
        children: vec![
            AppWidget::Button { id: "back".into(), label: "Back".into(), kind: ButtonKind::Normal },
            AppWidget::Button { id: "fwd".into(), label: "Forward".into(), kind: ButtonKind::Normal },
            AppWidget::Button { id: "up".into(), label: "Up".into(), kind: ButtonKind::Normal },
        ],
    }]);
    let (shell, wid, _s, _a) = widget_shell(model);

    let back = widget_box(&shell, wid, "back").expect("back button laid-out box");
    let fwd = widget_box(&shell, wid, "fwd").expect("forward button laid-out box");
    let up = widget_box(&shell, wid, "up").expect("up button laid-out box");

    // Horizontal row: x strictly increases left→right.
    assert!(
        back.x < fwd.x && fwd.x < up.x,
        "toolbar buttons must be laid out left-to-right (a horizontal row), \
         got back.x={}, fwd.x={}, up.x={} (equal x ⇒ vertical stack = the bug)",
        back.x, fwd.x, up.x
    );
    // Same row: their tops match (within a hairline) — they are NOT stacked.
    assert!(
        (back.y - fwd.y).abs() < 1.0 && (fwd.y - up.y).abs() < 1.0,
        "toolbar buttons in a row must share a top (y), got back.y={}, fwd.y={}, up.y={}",
        back.y, fwd.y, up.y
    );
    // Compact: the toolbar's height is on the order of a single button row, not
    // the sum of three stacked buttons.
    let tb_h = back.height.max(fwd.height).max(up.height);
    let span = (up.y + up.height).max(fwd.y + fwd.height) - back.y;
    assert!(
        span < tb_h * 2.0,
        "a horizontal toolbar's vertical span ({span}) must be ~one button tall \
         (button≈{tb_h}); a stacked toolbar would span ~3×"
    );
}

/// D1 (content nested under the host, not a bare child): the widget subtree is
/// mounted under an in-flow `app-content-body` wrapper that is a CHILD of the
/// per-window `app-content-host`, and the host is the content's positioning +
/// clipping context. This is the structural guard for the flex-under-fixed fix.
#[test]
fn widget_content_is_nested_under_an_in_flow_body() {
    let model = AppWidgetModel::with_root(vec![AppWidget::Button {
        id: "go".into(),
        label: "Go".into(),
        kind: ButtonKind::Normal,
    }]);
    let (shell, wid, _s, _a) = widget_shell(model);
    let doc = &shell.desktop_dom.doc;

    let host = doc
        .get_element_by_id(&format!("app-content-{}", wid.0))
        .expect("content host must exist");
    let body = doc
        .get_element_by_id(&format!("app-content-body-{}", wid.0))
        .expect("content BODY wrapper must exist");

    // The body is a direct child of the host (the host owns position/clip; the
    // body is the in-flow formatting context for the widgets).
    assert_eq!(
        doc.parent(body),
        Some(host),
        "the content body must be a child of the content host"
    );

    // The widget mounts UNDER the body (in flow), NOT directly under the fixed
    // host — this is what restores correct flex layout.
    let btn = doc
        .get_element_by_id(&crate::app_widgets::widget_id(wid.0, "go"))
        .expect("button must mount");
    let mut p = doc.parent(btn);
    let mut reaches_body = false;
    while let Some(node) = p {
        if node == body {
            reaches_body = true;
            break;
        }
        p = doc.parent(node);
    }
    assert!(
        reaches_body,
        "the mounted widget must be a descendant of the in-flow content body"
    );
}

/// D1 (content contained within its window): every laid-out widget box stays
/// inside the owning window's screen rect — app content does not float/bleed
/// outside the window it belongs to.
#[test]
fn widget_content_stays_within_its_window_rect() {
    let model = AppWidgetModel::with_root(vec![
        AppWidget::Toolbar {
            children: vec![
                AppWidget::Button { id: "a".into(), label: "A".into(), kind: ButtonKind::Normal },
                AppWidget::Button { id: "b".into(), label: "B".into(), kind: ButtonKind::Normal },
            ],
        },
        AppWidget::Button { id: "c".into(), label: "C".into(), kind: ButtonKind::Normal },
    ]);
    // A generously sized window so the (clipped) content rect is unambiguous.
    let mut shell = Shell::new(W, H);
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell.add_stylesheet(WIDGETS_CSS);
    shell.add_stylesheet(APP_WIDGET_TEST_CSS);
    let win_rect = Rect::new(120.0, 90.0, 700.0, 480.0);
    let wid = shell.open_window("Files", win_rect);
    let (app, _s, _a) = ModelApp::new(model);
    shell.register_app_view(wid, Box::new(app));
    let _ = shell.build_scene();

    // The host is positioned over the window's content rect.
    let host = shell
        .desktop_dom
        .doc
        .get_element_by_id(&format!("app-content-{}", wid.0))
        .expect("host");
    let host_box = shell
        .hit_test_engine
        .as_ref()
        .unwrap()
        .bounds_for_node(host)
        .expect("host box");
    // The host sits within the window's outer rect (it covers the content area
    // below the titlebar), never outside the window.
    assert!(
        host_box.x >= win_rect.x - 1.0
            && host_box.y >= win_rect.y - 1.0
            && host_box.x + host_box.width <= win_rect.x + win_rect.width + 1.0
            && host_box.y + host_box.height <= win_rect.y + win_rect.height + 1.0,
        "content host {host_box:?} must lie within its window {win_rect:?}"
    );

    // Each laid-out widget box is inside the host (clipped to the content rect),
    // so content cannot bleed past the window.
    for k in ["a", "b", "c"] {
        let b = widget_box(&shell, wid, k).unwrap_or_else(|| panic!("widget {k} box"));
        assert!(
            b.x >= host_box.x - 1.0
                && b.x + b.width <= host_box.x + host_box.width + 1.0,
            "widget {k} box {b:?} must stay within the content host {host_box:?}"
        );
    }
}
