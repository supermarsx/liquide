//! `<lq-month-calendar>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::{KeyInput, WidgetBehavior};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::month_calendar::{MonthCalendar, CHANGED_ACTION, MONTH_ACTION};

const W: u32 = 320;
const H: u32 = 360;

fn as_cal<'a>(g: &'a Gallery, id: &str) -> &'a MonthCalendar {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<MonthCalendar>()
        .unwrap()
}

fn day_box(g: &Gallery, root: liquide_dom::NodeId, d: u32) -> liquide_layout::geometry::Rect {
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("day-{d}")).expect("day box")
}

/// The calendar is always visible: day cells exist without any open step.
#[test]
fn days_are_always_visible() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "day-1").is_some());
    assert!(q.box_of_part(root, "day-30").is_some(), "June has 30 days");
    assert!(q.box_of_part(root, "day-31").is_none(), "June has no 31st");
}

/// Clicking a day cell selects that date; emits Changed(YYYY-MM-DD).
#[test]
fn click_day_selects() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let d15 = day_box(&g, root, 15);
    g.left_click(d15.x + d15.width / 2.0, d15.y + d15.height / 2.0);
    let a = g.process();
    let c = a.iter().find(|a| a.name == CHANGED_ACTION).expect("changed");
    assert_eq!(c.payload.as_deref(), Some("2026-06-15"));
    assert_eq!(as_cal(&g, "cal").selected(), Some((2026, 6, 15)));
}

/// ANTI-CONSTANT: per-day hit reads each cell's REAL laid-out box. With unusual
/// cell sizes, a click in day-10's true box selects 10 — a row*7+col guess over a
/// constant cell size would mis-target.
#[test]
fn day_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        420,
        H,
        "lq-cal-grid { width: 364px; } lq-cal-blank, lq-cal-day { width: 52px; height: 46px; }",
    );
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let d10 = day_box(&g, root, 10);
    assert!(d10.width >= 48.0, "precondition: wide cell (got {})", d10.width);
    g.left_click(d10.x + d10.width / 2.0, d10.y + d10.height / 2.0);
    let a = g.process();
    assert_eq!(
        a.iter().find(|a| a.name == CHANGED_ACTION).unwrap().payload.as_deref(),
        Some("2026-06-10"),
        "click in day-10's REAL box -> 10"
    );
}

/// Clicking next/prev navigates the month; the year rolls over at Dec/Jan.
#[test]
fn nav_changes_month_and_rolls_year() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 12)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next").expect("next box")
    };
    g.left_click(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let a = g.process();
    assert!(a.iter().any(|a| a.name == MONTH_ACTION));
    assert_eq!(as_cal(&g, "cal").view(), (2027, 1), "Dec -> Jan next year");
}

/// Keyboard arrows move the focused day across month boundaries; Enter selects.
#[test]
fn keyboard_moves_across_boundaries_and_selects() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6).select(2026, 6, 30)));
    g.relayout();
    g.host.set_focus(Some("cal"), &mut g.doc, &mut g.dispatcher);
    assert_eq!(as_cal(&g, "cal").focus_day(), 30);

    // +7 from June 30 crosses into July (30+7 = 37 -> July 7).
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert_eq!(as_cal(&g, "cal").view(), (2026, 7), "crossed into July");
    assert_eq!(as_cal(&g, "cal").focus_day(), 7);

    // Enter selects July 7.
    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(
        a.iter().find(|a| a.name == CHANGED_ACTION).unwrap().payload.as_deref(),
        Some("2026-07-07")
    );
}

/// The today cell carries the .today marker (rendered + restyled in pixels).
#[test]
fn today_marker_is_present() {
    let mut g = Gallery::new(W, H, "");
    g.mount(
        "cal",
        Box::new(MonthCalendar::new(2026, 6).today(2026, 6, 18)),
    );
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    // The day-18 cell exists; assert its rendered node carries the today class via
    // the document attribute path.
    let node = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.find_part(root, "day-18").expect("day-18 node")
    };
    assert!(
        g.doc().get(node).map(|n| n.has_class("today")).unwrap_or(false),
        "the today cell carries the .today class"
    );
}

/// PIXELS: selecting a day restyles its rasterized pixels (the accent fill).
#[test]
fn selection_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let d12 = day_box(&g, root, 12);
    let (sx, sy) = ((d12.x + d12.width / 2.0) as u32, (d12.y + d12.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    g.left_click(d12.x + d12.width / 2.0, d12.y + d12.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "selecting a day must restyle its pixels");
}

/// PIXELS :hover — hovering a day cell restyles its pixels (the hover fill); the
/// delta lands on the hovered day only.
#[test]
fn hovered_day_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    // day-1 is the default focus cell; use days 15 & 16 (no selection/focus/today).
    let (d15, d16) = (day_box(&g, root, 15), day_box(&g, root, 16));
    let (hx, hy) = ((d15.x + d15.width / 2.0) as u32, (d15.y + d15.height / 2.0) as u32);
    let (nx, ny) = ((d16.x + d16.width / 2.0) as u32, (d16.y + d16.height / 2.0) as u32);
    let before_h = Gallery::pixel(&g.rasterize(), hx, hy);
    let before_n = Gallery::pixel(&g.rasterize(), nx, ny);
    g.pointer_move(d15.x + d15.width / 2.0, d15.y + d15.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after_h = Gallery::pixel(&g.rasterize(), hx, hy);
    let after_n = Gallery::pixel(&g.rasterize(), nx, ny);
    assert!(before_h != after_h, "hovered day must restyle (before {before_h:?} after {after_h:?})");
    assert_eq!(before_n, after_n, "a non-hovered day must not change");
}

/// PIXELS :focus — the keyboard-focused day paints a focus border, and it MOVES
/// with the focus. day-1 is the default focus; drive Right to day 2.
#[test]
fn focus_border_moves_with_focused_day() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    g.host.set_focus(Some("cal"), &mut g.doc, &mut g.dispatcher);
    let root = g.host.root_of("cal").unwrap();
    let d2 = day_box(&g, root, 2);
    // Sample the top border edge of day 2 (away from glyph ink).
    let (sx, sy) = ((d2.x + d2.width / 2.0) as u32, (d2.y + 1.0) as u32);
    let unfocused = Gallery::pixel(&g.rasterize(), sx, sy);
    assert_eq!(as_cal(&g, "cal").focus_day(), 1, "focus starts on day 1");

    // Move focus onto day 2.
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_cal(&g, "cal").focus_day(), 2, "focus moved to day 2");
    g.relayout();
    let focused = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        unfocused != focused,
        "the focus border must paint on day 2 (before {unfocused:?} after {focused:?})"
    );

    // Move focus off day 2 (Left back to day 1) — day 2 returns to baseline.
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_cal(&g, "cal").focus_day(), 1);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert_eq!(after, unfocused, "the focus border must leave day 2 when focus moves");
}

/// PIXELS ::active/today — the today cell paints the today ring (an inset
/// box-shadow via `.today, :active`). Compare the today cell's border pixels to an
/// identical non-today cell. The existing today test only asserts the .today class;
/// this proves the ring lands in PIXELS.
#[test]
fn today_ring_paints_pixels() {
    // Put "today" on a day that is NOT the default focus (1) and NOT selected, so
    // the today ring is the only restyle on that cell.
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6).today(2026, 6, 20)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let (today, plain) = (day_box(&g, root, 20), day_box(&g, root, 21));
    // Sample the top inset edge (1px in) where the inset ring lands.
    let ty = (today.y + 1.0) as u32;
    let tx = (today.x + today.width / 2.0) as u32;
    let py = (plain.y + 1.0) as u32;
    let px = (plain.x + plain.width / 2.0) as u32;
    let fb = g.rasterize();
    let today_px = Gallery::pixel(&fb, tx, ty);
    let plain_px = Gallery::pixel(&fb, px, py);
    assert!(
        today_px != plain_px,
        "the today ring must paint on the today cell, distinct from a plain cell \
         (today {today_px:?}, plain {plain_px:?})"
    );
}

/// PIXELS :hover (nav) — hovering the prev/next nav button restyles its pixels.
#[test]
fn nav_button_hover_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next").expect("next box")
    };
    // Sample a corner of the button (away from the chevron glyph centre).
    let (sx, sy) = ((next.x + 3.0) as u32, (next.y + 3.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    g.pointer_move(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        before != after,
        "hovering the next-month button must restyle it (before {before:?} after {after:?})"
    );
}

/// SELECTION MOVES: selecting day B after day A clears A's accent fill (selection
/// rides the current day; it does not accumulate).
#[test]
fn selection_moves_off_previous_day() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let (d10, d20) = (day_box(&g, root, 10), day_box(&g, root, 20));
    let (ax, ay) = ((d10.x + d10.width / 2.0) as u32, (d10.y + d10.height / 2.0) as u32);
    let baseline = Gallery::pixel(&g.rasterize(), ax, ay);

    g.left_click(d10.x + d10.width / 2.0, d10.y + d10.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_cal(&g, "cal").selected(), Some((2026, 6, 10)));
    let selected = Gallery::pixel(&g.rasterize(), ax, ay);
    assert!(selected != baseline, "day 10 selected differs from baseline");

    g.left_click(d20.x + d20.width / 2.0, d20.y + d20.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_cal(&g, "cal").selected(), Some((2026, 6, 20)));
    let after = Gallery::pixel(&g.rasterize(), ax, ay);
    assert_eq!(after, baseline, "day 10 must lose the accent fill when selection moves");
}

/// DISABLED: a disabled calendar swallows a day click — no selection, no action,
/// and is not focusable.
#[test]
fn disabled_calendar_swallows_click() {
    let mut g = Gallery::new(W, H, "");
    g.mount("cal", Box::new(MonthCalendar::new(2026, 6).disabled(true)));
    g.relayout();
    let root = g.host.root_of("cal").unwrap();
    let d15 = day_box(&g, root, 15);
    g.left_click(d15.x + d15.width / 2.0, d15.y + d15.height / 2.0);
    let acts = g.process();
    assert!(acts.is_empty(), "disabled calendar must emit nothing");
    assert_eq!(as_cal(&g, "cal").selected(), None, "disabled calendar selects nothing");
    assert!(!as_cal(&g, "cal").focusable(), "disabled calendar is not focusable");
}
