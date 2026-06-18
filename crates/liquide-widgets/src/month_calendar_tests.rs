//! `<lq-month-calendar>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
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
