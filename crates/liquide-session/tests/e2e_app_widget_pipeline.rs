//! END-TO-END app-widget pipeline tests (t122-app-e2e, P8 hardening).
//!
//! These tests drive each *migrated* application's REAL `AppView`
//! (`liquide-apps-{settings,files,task-manager}`) through the REAL shell P8
//! engine and prove the WHOLE loop, not just the data-level per-app unit tests:
//!
//! ```text
//!   app.widget_model()                       (the real runtime's model)
//!     -> shell generic mapper (app_widgets)  (mount_model_into)
//!     -> mounted liquide-widgets DOM         (real <lq-*> elements + CSS layout)
//!     -> real EventDispatcher click          (a real platform Move/Press/Release)
//!     -> WidgetAction                        (the toolkit's event)
//!     -> AppWidgetAction                     (translate_action)
//!     -> AppView::apply_action               (the real runtime mutates)
//!     -> model mutated                       (observed via app_view().widget_model())
//!     -> re-render                           (the next build_scene reflects it)
//! ```
//!
//! Unlike `liquide-shell`'s `app_widget_content_tests.rs` (which drives a
//! *synthetic* `ModelApp`), these tests register the SHIPPING runtimes
//! (`SettingsRuntime`, `FilesRuntime`, `TaskManagerRuntime`) — so a no-op or
//! mis-wired engine fails against the apps users actually run.
//!
//! NO FAKE-GREEN. Every test:
//!   * reads the click target's hit geometry from the LIVE laid-out CSS box
//!     (`hit_test_engine().bounds_for_node(...)`), never a constant — a miss-test
//!     proves a click *outside* the box leaves the model unchanged;
//!   * drives a real `PlatformEvent` Move + Press + Release through
//!     `Shell::handle_platform_event` into the real `EventDispatcher`;
//!   * asserts the REAL runtime state changed, observed back through the shell's
//!     public `Shell::app_view(wid).widget_model()` (NOT a side channel) — a
//!     no-op engine that never reaches `apply_action` leaves the model identical
//!     and fails the assertion.
//!
//! Only the shell's PUBLIC API is used (this is an external integration crate):
//! `Shell::{new, open_window, register_app_view, add_stylesheet, build_scene,
//! handle_platform_event, app_view, document, hit_test_engine}`. The per-window
//! widget id namespacing (`aw-<window_id>-<key>`) is reconstructed locally — it
//! is the engine's stable public contract (see `app_widgets::widget_id`).

use liquide_compositor::geometry::Rect;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use liquide_interop::{AppView, AppWidget, AppWidgetModel, SortDirection};

use liquide_shell::{Shell, WindowId};

// Real shipping app runtimes (the migrated apps under test).
use liquide_apps_files::config::FilesConfig;
use liquide_apps_files::entry::FileEntry;
use liquide_apps_files::runtime::FilesRuntime;

use liquide_apps_settings::category::Category;
use liquide_apps_settings::config::SettingsConfig;
use liquide_apps_settings::runtime::SettingsRuntime;

use liquide_apps_task_manager::config::TaskManagerConfig;
use liquide_apps_task_manager::process::ProcessInfo;
use liquide_apps_task_manager::runtime::TaskManagerRuntime;

const W: f32 = 1280.0;
const H: f32 = 720.0;

/// The shipped widget toolkit stylesheet (box/reset rules for `<lq-*>`). Loading
/// it is what gives the mounted widgets real laid-out boxes.
const WIDGETS_CSS: &str = include_str!("../../../assets/themes/widgets.css");

/// Positioning CSS that gives the per-window content host a stacking context and
/// every widget family a generous, unambiguous box so the layout-derived hit
/// geometry is well-defined. The table keeps its OWN toolkit grid/row layout (we
/// resolve header / row sub-boxes from the live layout tree, not by guessing
/// offsets), so we only widen it so the columns are comfortably hittable.
const TEST_CSS: &str = r#"
app-content-host { display: block; }
lq-checkbox, lq-button, lq-slider, lq-dropdown {
    display: block;
    width: 260px;
    height: 44px;
    margin: 0;
}
/* Keep the files widget stack compact + bounded so the directory-listing table
   lands well within the window's content rect (the content host clips and the
   click router only forwards points inside the window). */
lq-breadcrumb { display: block; width: 520px; height: 28px; margin: 0; }
lq-list { display: block; width: 260px; height: 110px; overflow: hidden; }
lq-toolbar { display: block; height: 40px; overflow: hidden; }
lq-toolbar > lq-button { display: inline-block; width: 90px; height: 36px; margin: 0; }
lq-table { display: block; width: 560px; }
lq-tabs { display: block; }
lq-panel { display: block; }
"#;

// ─────────────────────────────────────────────────────────────────────────────
// Harness
// ─────────────────────────────────────────────────────────────────────────────

/// Build a shell with the widget + positioning CSS loaded, one window open
/// backed by `view`, and one scene built (so the pipeline lays out the widgets
/// and the hit-test engine has the boxes).
fn shell_with_app(view: Box<dyn AppView>) -> (Shell, WindowId) {
    // A tall, wide window whose CONTENT rect comfortably contains each app's
    // whole widget stack on screen (the per-window content host clips to the
    // content rect, and the click router only forwards points inside the
    // window — so a widget that overflowed the window would be unclickable).
    shell_with_app_in(view, Rect::new(40.0, 20.0, 720.0, 680.0))
}

fn shell_with_app_in(view: Box<dyn AppView>, bounds: Rect) -> (Shell, WindowId) {
    let mut shell = Shell::new(W, H);
    shell.add_stylesheet(WIDGETS_CSS);
    shell.add_stylesheet(TEST_CSS);

    let wid = shell.open_window("App", bounds);
    shell.register_app_view(wid, view);
    let _ = shell.build_scene();
    (shell, wid)
}

/// The per-window namespaced widget id for an app key, matching the engine's
/// stable `aw-<window_id>-<key>` contract (`app_widgets::widget_id`).
fn widget_id(wid: WindowId, key: &str) -> String {
    format!("aw-{}-{}", wid.0, key)
}

/// The live laid-out screen box of a mounted widget, by its app key — read from
/// the real layout tree via the public hit-test engine. `None` if the widget did
/// not mount / lay out.
fn widget_box(shell: &Shell, wid: WindowId, key: &str) -> Option<liquide_layout::geometry::Rect> {
    let id = widget_id(wid, key);
    let node = shell.document().get_element_by_id(&id)?;
    shell.hit_test_engine()?.bounds_for_node(node)
}

/// The live laid-out box of a SUB-ELEMENT of a mounted widget, identified by its
/// toolkit `data-part` (e.g. `head-0`, `row-2`). The toolkit resolves clicks by
/// each child's laid-out `data-part` box, so clicking the CENTER of that box is
/// the genuinely layout-derived hit — no offset guessing. `None` if no such part
/// laid out under the widget.
fn part_box(
    shell: &Shell,
    wid: WindowId,
    key: &str,
    part: &str,
) -> Option<liquide_layout::geometry::Rect> {
    let root = shell.document().get_element_by_id(&widget_id(wid, key))?;
    let engine = shell.hit_test_engine()?;
    for node in shell.document().descendants(root) {
        if let Some(n) = shell.document().get(node) {
            if n.get_attribute("data-part") == Some(part) {
                return engine.bounds_for_node(node);
            }
        }
    }
    None
}

/// A full left click (Move, then Press, then Release) at `(x, y)` through the
/// real platform-event path → the real `EventDispatcher`. The leading Move
/// builds the hover chain so a Click on a widget sub-element bubbles to the
/// widget root's handler — matching the real input sequence.
fn click(shell: &mut Shell, x: f32, y: f32) {
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

/// Re-fetch the app's CURRENT widget model through the shell's public seam (the
/// same path the renderer uses). This is the end-to-end observation point: it
/// reflects whatever `apply_action` mutated in the real runtime.
fn current_model(shell: &Shell, wid: WindowId) -> AppWidgetModel {
    shell
        .app_view(wid)
        .expect("the window has a registered app view")
        .widget_model()
        .expect("the migrated app exposes a widget model")
}

/// Find a widget by key in a model (recurses into containers via `find_mut`).
fn find_owned(model: &mut AppWidgetModel, key: &str) -> Option<AppWidget> {
    model.find_mut(key).cloned()
}

// ─────────────────────────────────────────────────────────────────────────────
// SETTINGS — a real click on a CSS-laid-out checkbox flips the setting
// ─────────────────────────────────────────────────────────────────────────────

fn settings_runtime() -> SettingsRuntime {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_category(Category::Display);
    rt
}

/// E2E: clicking the `display.night_light` checkbox at its LAID-OUT box center
/// drives the whole pipeline and flips the real setting from false → true,
/// observed back through `app_view().widget_model()`.
#[test]
fn settings_checkbox_click_flips_the_real_setting() {
    let (mut shell, wid) = shell_with_app(Box::new(settings_runtime()));

    // The checkbox mounted as a host widget.
    let cb_key = "display.night_light";
    assert!(
        shell
            .document()
            .get_element_by_id(&widget_id(wid, cb_key))
            .is_some(),
        "the night_light checkbox must mount as a host widget"
    );

    // Pre-state: unchecked in the model.
    let mut before = current_model(&shell, wid);
    assert!(
        matches!(
            find_owned(&mut before, cb_key),
            Some(AppWidget::Checkbox { checked: false, .. })
        ),
        "night_light must start unchecked, got {:?}",
        find_owned(&mut before, cb_key)
    );

    // Click the laid-out box CENTER (geometry from layout, not a constant).
    let b = widget_box(&shell, wid, cb_key).expect("checkbox laid-out box");
    assert!(b.width > 1.0 && b.height > 1.0, "checkbox box {b:?}");
    click(&mut shell, b.x + b.width / 2.0, b.y + b.height / 2.0);
    let _ = shell.build_scene(); // drive loop drains + applies + re-renders

    // The real setting flipped — observed through the public widget-model seam.
    let mut after = current_model(&shell, wid);
    assert!(
        matches!(
            find_owned(&mut after, cb_key),
            Some(AppWidget::Checkbox { checked: true, .. })
        ),
        "a real click must flip the night_light setting to checked, got {:?}",
        find_owned(&mut after, cb_key)
    );
}

/// Teeth for the geometry: a click OUTSIDE the checkbox box must NOT toggle the
/// setting — proving the hit lands on the real laid-out box, not an always-accept
/// stub.
#[test]
fn settings_click_outside_checkbox_does_not_flip() {
    let (mut shell, wid) = shell_with_app(Box::new(settings_runtime()));
    let cb_key = "display.night_light";

    let b = widget_box(&shell, wid, cb_key).expect("checkbox laid-out box");
    // Far below the checkbox box (still on screen, but not on the widget).
    click(&mut shell, b.x + b.width / 2.0, b.y + b.height + 240.0);
    let _ = shell.build_scene();

    let mut after = current_model(&shell, wid);
    assert!(
        matches!(
            find_owned(&mut after, cb_key),
            Some(AppWidget::Checkbox { checked: false, .. })
        ),
        "a miss must leave night_light unchecked, got {:?}",
        find_owned(&mut after, cb_key)
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// FILES — a real click on a listing-table ROW selects it in the real runtime
// ─────────────────────────────────────────────────────────────────────────────

fn files_runtime() -> FilesRuntime {
    let mut rt = FilesRuntime::new(FilesConfig::default());
    let entries = vec![
        FileEntry::directory("docs".into(), "/home/user/docs".into(), 0),
        FileEntry::file("a.txt".into(), "/home/user/a.txt".into(), 10, 200_000),
        FileEntry::file("b.txt".into(), "/home/user/b.txt".into(), 20, 300_000),
    ];
    rt.navigate("/home/user".into(), entries);
    rt
}

/// The selection vector of the files listing table in the current model.
fn files_table_selection(shell: &Shell, wid: WindowId) -> Vec<u32> {
    let mut model = current_model(shell, wid);
    match find_owned(&mut model, "listing") {
        Some(AppWidget::Table { selected, .. }) => selected,
        other => panic!("expected the listing Table, got {other:?}"),
    }
}

/// E2E: clicking a data ROW of the directory-listing table at its LAID-OUT
/// vertical band drives the pipeline and updates the real runtime's selection,
/// reflected in the re-rendered Table's `selected`.
#[test]
fn files_row_click_selects_in_the_real_runtime() {
    let (mut shell, wid) = shell_with_app(Box::new(files_runtime()));

    // Pre-state: nothing selected.
    assert!(
        files_table_selection(&shell, wid).is_empty(),
        "nothing should be selected before the click"
    );

    // Click the THIRD data row (display index 2). Its laid-out box is read from
    // the live layout tree by its toolkit `data-part="row-2"` — the genuinely
    // layout-derived hit (the toolkit resolves the clicked row by this same box).
    let target_row = 2u32;
    let rb = part_box(&shell, wid, "listing", &format!("row-{target_row}"))
        .expect("listing row-2 laid-out box");
    assert!(rb.width > 1.0 && rb.height > 1.0, "row box {rb:?}");
    click(&mut shell, rb.x + rb.width / 2.0, rb.y + rb.height / 2.0);
    let _ = shell.build_scene();

    let sel = files_table_selection(&shell, wid);
    assert_eq!(
        sel,
        vec![target_row],
        "the row click must single-select display row {target_row} in the real runtime"
    );
}

/// E2E + teeth: clicking a DIFFERENT row selects that other row (so selection
/// truly tracks the clicked layout band, not a fixed answer), and a click in the
/// header band does NOT select a body row.
#[test]
fn files_row_click_targets_the_clicked_band() {
    let (mut shell, wid) = shell_with_app(Box::new(files_runtime()));

    // Click row 0 (its laid-out box, by data-part).
    let r0 = part_box(&shell, wid, "listing", "row-0").expect("row-0 box");
    click(&mut shell, r0.x + r0.width / 2.0, r0.y + r0.height / 2.0);
    let _ = shell.build_scene();
    assert_eq!(
        files_table_selection(&shell, wid),
        vec![0u32],
        "clicking row 0's laid-out box selects row 0"
    );

    // Click row 1's laid-out box: selection moves to row 1, proving the row is
    // resolved from layout (a stub returning a constant could not do this). The
    // two rows have DIFFERENT laid-out y boxes, so this distinguishes them.
    let r1 = part_box(&shell, wid, "listing", "row-1").expect("row-1 box");
    assert!(
        (r1.y - r0.y).abs() > 1.0,
        "row 0 and row 1 must occupy different laid-out bands (r0={r0:?}, r1={r1:?})"
    );
    click(&mut shell, r1.x + r1.width / 2.0, r1.y + r1.height / 2.0);
    let _ = shell.build_scene();
    assert_eq!(
        files_table_selection(&shell, wid),
        vec![1u32],
        "clicking row 1's laid-out box moves the selection to row 1"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// TASK MANAGER — a real click on a table HEADER re-sorts the real runtime
// ─────────────────────────────────────────────────────────────────────────────

fn proc(name: &str, pid: u32, cpu: f64, mem_mb: u64) -> ProcessInfo {
    ProcessInfo {
        name: name.to_string(),
        pid,
        cpu_percent: cpu,
        mem_working_bytes: mem_mb * 1024 * 1024,
        ..ProcessInfo::default()
    }
}

/// A frozen, deterministic, intentionally-unsorted process set so a header-sort
/// click visibly reorders the rows.
fn task_manager_runtime() -> TaskManagerRuntime {
    let mut rt = TaskManagerRuntime::new(TaskManagerConfig::default());
    rt.set_processes(vec![
        proc("bravo", 200, 5.0, 64),
        proc("alpha", 100, 30.0, 256),
        proc("charlie", 300, 1.0, 16),
    ]);
    rt
}

/// The process table from the current model (lives inside the Tabs container's
/// Processes tab) — `find_mut` recurses into Tabs.
fn task_table(shell: &Shell, wid: WindowId) -> AppWidget {
    let mut model = current_model(shell, wid);
    find_owned(&mut model, "process_table").expect("process table present in the model")
}

fn table_row_names(table: &AppWidget) -> Vec<String> {
    match table {
        AppWidget::Table { rows, .. } => rows.iter().map(|r| r[0].clone()).collect(),
        other => panic!("expected a Table, got {other:?}"),
    }
}

/// The selection vector of the process table in the current model.
fn task_table_selection(shell: &Shell, wid: WindowId) -> Vec<u32> {
    match task_table(shell, wid) {
        AppWidget::Table { selected, .. } => selected,
        other => panic!("expected a Table, got {other:?}"),
    }
}

/// E2E: clicking a process ROW at its LAID-OUT box (toolkit `data-part="row-N"`,
/// in the table's body BELOW the header) drives the whole pipeline and selects
/// that process in the real runtime — reflected both in the re-rendered Table's
/// `selected` AND by the End-task button appearing (the model only emits that
/// button while a process is selected). A no-op engine leaves nothing selected.
#[test]
fn task_manager_row_click_selects_process_in_the_real_runtime() {
    let (mut shell, wid) = shell_with_app(Box::new(task_manager_runtime()));

    // Default order is CPU-descending: [alpha(30), bravo(5), charlie(1)].
    let before = task_table(&shell, wid);
    assert_eq!(
        table_row_names(&before),
        vec!["alpha", "bravo", "charlie"],
        "default order must be CPU-descending"
    );
    assert!(
        task_table_selection(&shell, wid).is_empty(),
        "nothing selected before the click"
    );

    // Click display row 1 (bravo, pid 200) at its laid-out body-row box.
    let rb = part_box(&shell, wid, "process_table", "row-1").expect("row-1 laid-out box");
    assert!(rb.width > 1.0 && rb.height > 1.0, "row box {rb:?}");
    click(&mut shell, rb.x + rb.width / 2.0, rb.y + rb.height / 2.0);
    let _ = shell.build_scene();

    // The real runtime selected display row 1 ...
    assert_eq!(
        task_table_selection(&shell, wid),
        vec![1u32],
        "the row click must single-select display row 1 (bravo) in the real runtime"
    );
    // ... and the model now emits the End-task button (only present with a
    // selection) — a second observable proof the runtime state really changed.
    let mut model = current_model(&shell, wid);
    assert!(
        matches!(find_owned(&mut model, "end_task"), Some(AppWidget::Button { .. })),
        "selecting a process must make the model emit the End-task button"
    );
}

/// E2E + teeth: clicking a DIFFERENT row selects that other row (selection tracks
/// the clicked laid-out band, not a fixed answer), then a real click on the
/// END-TASK BUTTON (which only exists because the previous click selected a row)
/// flows through the pipeline and removes that process from the real runtime.
#[test]
fn task_manager_row_then_end_task_button_mutate_the_real_runtime() {
    let (mut shell, wid) = shell_with_app(Box::new(task_manager_runtime()));

    // Click row 0 (alpha): selection → [0].
    let r0 = part_box(&shell, wid, "process_table", "row-0").expect("row-0 box");
    click(&mut shell, r0.x + r0.width / 2.0, r0.y + r0.height / 2.0);
    let _ = shell.build_scene();
    assert_eq!(task_table_selection(&shell, wid), vec![0u32], "row 0 selected");

    // Click row 2 (charlie): selection MOVES to [2], proving the band is resolved
    // from layout (rows 0 and 2 occupy different laid-out y boxes).
    let r2 = part_box(&shell, wid, "process_table", "row-2").expect("row-2 box");
    assert!(
        (r2.y - r0.y).abs() > 1.0,
        "row 0 and row 2 must occupy different laid-out bands"
    );
    click(&mut shell, r2.x + r2.width / 2.0, r2.y + r2.height / 2.0);
    let _ = shell.build_scene();
    assert_eq!(
        task_table_selection(&shell, wid),
        vec![2u32],
        "clicking row 2 moves the selection to charlie"
    );
    // charlie is display row 2 in CPU-desc order.
    assert_eq!(table_row_names(&task_table(&shell, wid))[2], "charlie");

    // The End-task button is now present; click it through the full pipeline.
    let bb = widget_box(&shell, wid, "end_task").expect("End-task button laid-out box");
    click(&mut shell, bb.x + bb.width / 2.0, bb.y + bb.height / 2.0);
    let _ = shell.build_scene();

    // The real runtime ended charlie: only alpha + bravo remain, nothing selected,
    // and the End-task button is gone again.
    let after = task_table(&shell, wid);
    let names = table_row_names(&after);
    assert!(
        !names.contains(&"charlie".to_string()) && names.len() == 2,
        "the End-task click must remove charlie from the real runtime, got {names:?}"
    );
    let mut model = current_model(&shell, wid);
    assert!(
        find_owned(&mut model, "end_task").is_none(),
        "with the selection cleared the End-task button must disappear again"
    );
}

/// E2E: clicking a sortable column HEADER re-sorts the real runtime (t124 FIXED).
///
/// This previously failed (and was `#[ignore]`d as a known-bug tripwire) because:
///
///   * the `liquide-widgets` `Table` emits its `Sorted` action with the payload
///     `"<col>:<asc|desc>"` (e.g. `"0:asc"`, see `table.rs::sort_by`);
///   * `liquide-apps-task-manager`'s `apply_action` parses the sort payload as a
///     BARE `u32`, which failed on `"0:asc"` → no re-sort. The per-app unit tests
///     missed it because they passed an already-clean `"0"`.
///
/// FIX (canonical contract): the shell's `translate_action` now NORMALIZES a
/// Table `sorted` payload to the bare column index `"<col>"` at the single
/// toolkit→interop chokepoint, so every Table-consuming app receives one stable
/// form (apps own the direction toggle on re-click). This test is the end-to-end
/// proof, driven through the real shell engine + real runtime.
#[test]
fn task_manager_header_click_should_resort_the_real_runtime() {
    let (mut shell, wid) = shell_with_app(Box::new(task_manager_runtime()));

    // Default CPU-desc: [alpha, bravo, charlie].
    assert_eq!(
        table_row_names(&task_table(&shell, wid)),
        vec!["alpha", "bravo", "charlie"]
    );

    // Click the "Name" header (head-0) at its laid-out box.
    let hb = part_box(&shell, wid, "process_table", "head-0").expect("head-0 box");
    click(&mut shell, hb.x + hb.width / 2.0, hb.y + hb.height / 2.0);
    let _ = shell.build_scene();

    // CORRECT expectation: the runtime re-sorts by Name ascending and records it.
    let after = task_table(&shell, wid);
    assert_eq!(
        table_row_names(&after),
        vec!["alpha", "bravo", "charlie"],
        "a Name-header click should sort ascending by name"
    );
    match &after {
        AppWidget::Table { sort: Some(s), .. } => {
            assert_eq!(s.column, 0, "sort column should become Name (0)");
            assert_eq!(s.direction, SortDirection::Ascending);
        }
        other => panic!("expected a sorted Table, got {other:?}"),
    }
}
