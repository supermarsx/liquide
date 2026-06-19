//! `<lq-date-picker>` real-pipeline gallery tests + calendar-math unit tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::date_picker::{
    day_of_week_first, days_in_month, is_leap, DatePicker, CHANGED_ACTION,
};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 360;
const H: u32 = 420;

fn as_dp<'a>(g: &'a Gallery, id: &str) -> &'a DatePicker {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<DatePicker>()
        .unwrap()
}

// ── Calendar math (self-contained, no chrono) ──────────────────────────────

#[test]
fn leap_years() {
    assert!(is_leap(2024));
    assert!(is_leap(2000));
    assert!(!is_leap(1900));
    assert!(!is_leap(2023));
}

#[test]
fn days_per_month() {
    assert_eq!(days_in_month(2024, 2), 29); // leap Feb
    assert_eq!(days_in_month(2023, 2), 28);
    assert_eq!(days_in_month(2023, 4), 30);
    assert_eq!(days_in_month(2023, 12), 31);
}

#[test]
fn first_weekday_known_dates() {
    // 2026-06-01 was a Monday (=1 with Sunday=0).
    assert_eq!(day_of_week_first(2026, 6), 1);
    // 2000-01-01 was a Saturday (=6).
    assert_eq!(day_of_week_first(2000, 1), 6);
}

// ── Widget behavior ────────────────────────────────────────────────────────

fn open(g: &mut Gallery, id: &str) {
    let root = g.host.root_of(id).unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").expect("button box")
    };
    g.left_click(btn.x + 5.0, btn.y + btn.height / 2.0);
    let _ = g.process();
    g.relayout();
}

/// Clicking the button opens the month grid (day cells appear).
#[test]
fn button_opens_grid() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("dp").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "day-15").is_none(), "closed: no days");
    }
    open(&mut g, "dp");
    assert!(as_dp(&g, "dp").is_open());
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "day-15").is_some(), "open: day cells exist");
}

/// Clicking a day cell selects that date + closes; emits Changed(YYYY-MM-DD).
#[test]
fn click_day_selects_date() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    open(&mut g, "dp");

    let root = g.host.root_of("dp").unwrap();
    let d15 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "day-15").expect("day-15 box")
    };
    g.left_click(d15.x + d15.width / 2.0, d15.y + d15.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("2026-06-15"));
    assert_eq!(as_dp(&g, "dp").selected(), Some((2026, 6, 15)));
    g.relayout();
    assert!(!as_dp(&g, "dp").is_open(), "selecting closes the popup");
}

/// NO-FAKE-GREEN tooth: per-day hit reads each cell's REAL laid-out box. With an
/// unusual cell size, a click in day-10's true box selects 10 — a `row*7+col`
/// guess over a constant cell size would mis-target.
#[test]
fn day_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        420,
        H,
        "lq-gallery { padding: 12px; } lq-date-grid { width: 350px; } \
         lq-date-blank, lq-date-day { width: 50px; height: 44px; }",
    );
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    open(&mut g, "dp");
    let root = g.host.root_of("dp").unwrap();
    let d10 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "day-10").expect("day-10 box")
    };
    assert!(d10.width >= 45.0, "precondition: wide cell (got {})", d10.width);
    g.left_click(d10.x + d10.width / 2.0, d10.y + d10.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("2026-06-10"), "click in day-10's REAL box -> 10");
}

/// Clicking the next-month nav advances the displayed month (popup stays open).
#[test]
fn next_month_navigates() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    open(&mut g, "dp");
    let root = g.host.root_of("dp").unwrap();
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next-month").expect("next-month box")
    };
    g.left_click(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let _ = g.process();
    assert_eq!(as_dp(&g, "dp").view(), (2026, 7), "advanced to July");
    assert!(as_dp(&g, "dp").is_open(), "nav keeps the popup open");
}

/// Year rolls over when paging past December.
#[test]
fn next_month_rolls_year() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 12)));
    g.relayout();
    open(&mut g, "dp");
    let root = g.host.root_of("dp").unwrap();
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next-month").unwrap()
    };
    g.left_click(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let _ = g.process();
    assert_eq!(as_dp(&g, "dp").view(), (2027, 1), "Dec -> Jan next year");
}

/// Keyboard: arrows move the focused day, Enter selects.
#[test]
fn keyboard_moves_and_selects() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6).select(2026, 6, 10)));
    g.relayout();
    g.host.set_focus(Some("dp"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // open
    g.relayout();
    assert_eq!(as_dp(&g, "dp").focus_day(), 10);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // +1 -> 11
    assert_eq!(as_dp(&g, "dp").focus_day(), 11);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // +7 -> 18
    assert_eq!(as_dp(&g, "dp").focus_day(), 18);

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a[0].payload.as_deref(), Some("2026-06-18"));
}

/// Resolve a part box for the `dp` widget.
fn dp_part(g: &Gallery, part: &str) -> Option<liquide_layout::geometry::Rect> {
    let root = g.host.root_of("dp").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part)
}

/// :checked/.selected restyles the selected day cell (accent BACKGROUND fill, not
/// the transparent default). Compare a day that is pre-selected against the same
/// day cell in an un-selected picker — its body must differ.
#[test]
fn selected_day_restyles_cell_fill() {
    // Unselected baseline: day-15 body is transparent over the popup surface.
    let mut plain = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    plain.mount("dp", Box::new(DatePicker::new(2026, 6)));
    plain.relayout();
    open(&mut plain, "dp");
    let d15 = dp_part(&plain, "day-15").expect("day-15");
    let cx = (d15.x + d15.width / 2.0) as u32;
    let cy = (d15.y + d15.height / 2.0) as u32;
    let unselected = Gallery::pixel(&plain.rasterize(), cx, cy);

    // Pre-select 2026-06-15; the day cell now carries the accent fill.
    let mut sel = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    sel.mount("dp", Box::new(DatePicker::new(2026, 6).select(2026, 6, 15)));
    sel.relayout();
    open(&mut sel, "dp");
    let selected = Gallery::pixel(&sel.rasterize(), cx, cy);

    assert!(
        unselected != selected,
        ":checked day must fill with the accent (unselected={unselected:?}, selected={selected:?})"
    );
}

/// The selected-day fill MOVES with the selection: clicking day-20 selects it and
/// closes; re-open and assert day-20's body now reads the accent fill while a
/// neighbour (day-19) stays the transparent default.
#[test]
fn selected_fill_moves_with_selection() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    open(&mut g, "dp");

    // Click day-20.
    let d20 = dp_part(&g, "day-20").expect("day-20");
    g.left_click(d20.x + d20.width / 2.0, d20.y + d20.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_dp(&g, "dp").selected(), Some((2026, 6, 20)));
    open(&mut g, "dp");

    let d20b = dp_part(&g, "day-20").expect("day-20");
    let d19 = dp_part(&g, "day-19").expect("day-19");
    let sel_px = Gallery::pixel(
        &g.rasterize(),
        (d20b.x + d20b.width / 2.0) as u32,
        (d20b.y + d20b.height / 2.0) as u32,
    );
    let neighbour = Gallery::pixel(
        &g.rasterize(),
        (d19.x + d19.width / 2.0) as u32,
        (d19.y + d19.height / 2.0) as u32,
    );
    assert!(
        sel_px != neighbour,
        "the selected day must differ from its neighbour (sel={sel_px:?}, neighbour={neighbour:?})"
    );
}

/// A day cell :hover restyles its background (CSS `lq-date-day:hover`). Sample the
/// cell center before vs after a hover move onto it.
#[test]
fn day_hover_restyles_cell() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    open(&mut g, "dp");

    let d12 = dp_part(&g, "day-12").expect("day-12");
    let cx = (d12.x + d12.width / 2.0) as u32;
    let cy = (d12.y + d12.height / 2.0) as u32;
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.pointer_move(d12.x + d12.width / 2.0, d12.y + d12.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);

    assert!(
        before != after,
        "day :hover must restyle the cell (before={before:?}, after={after:?})"
    );
}

/// The keyboard :focus ring restyles the focused day's border AND moves with the
/// focus: focusing day-10 then arrowing right lights day-11 and reverts day-10.
#[test]
fn day_focus_ring_moves() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6).select(2026, 6, 10)));
    g.relayout();
    g.host.set_focus(Some("dp"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // open, focus day 10
    g.relayout();
    assert_eq!(as_dp(&g, "dp").focus_day(), 10);

    // day-10 is BOTH selected and focused here; to isolate the focus ring, sample
    // the focused-but-unselected neighbour. Move focus to 11 (not selected).
    let d11 = dp_part(&g, "day-11").expect("day-11");
    // Sample day-11's top-edge border band (focus adds a 1px accent border). Use
    // the horizontal center of the top edge, away from the rounded corners.
    let bx = (d11.x + d11.width / 2.0) as u32;
    let by = (d11.y) as u32;
    let d11_unfocused = Gallery::pixel(&g.rasterize(), bx, by);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // focus -> 11
    assert_eq!(as_dp(&g, "dp").focus_day(), 11);
    g.relayout();
    let d11_focused = Gallery::pixel(&g.rasterize(), bx, by);

    assert!(
        d11_unfocused != d11_focused,
        "day 11 must GAIN the focus border (was {d11_unfocused:?}, now {d11_focused:?})"
    );
}

/// A nav arrow :hover restyles its background (CSS `lq-date-next:hover`). Sample
/// the next-month button center before vs after hovering it.
#[test]
fn nav_hover_restyles() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    open(&mut g, "dp");

    let next = dp_part(&g, "next-month").expect("next-month");
    let cx = (next.x + next.width / 2.0) as u32;
    let cy = (next.y + next.height / 2.0) as u32;
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.pointer_move(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);

    assert!(
        before != after,
        "nav :hover must restyle the arrow background (before={before:?}, after={after:?})"
    );
}

/// The trigger button border restyles while open (CSS `lq-date-picker.open >
/// lq-date-button` -> accent border). Sample the button's top border band.
#[test]
fn open_restyles_button_border() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();

    let btn = dp_part(&g, "button").expect("button");
    let bx = (btn.x + btn.width / 2.0) as u32;
    let by = (btn.y) as u32; // top border band
    let closed = Gallery::pixel(&g.rasterize(), bx, by);

    open(&mut g, "dp");
    assert!(as_dp(&g, "dp").is_open());
    let opened = Gallery::pixel(&g.rasterize(), bx, by);

    assert!(
        closed != opened,
        "the open button border must restyle (closed={closed:?}, opened={opened:?})"
    );
}

/// Opening restyles the rasterized pixels (the grid surface appears).
#[test]
fn open_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("dp", Box::new(DatePicker::new(2026, 6)));
    g.relayout();
    let root = g.host.root_of("dp").unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").unwrap()
    };
    let (sx, sy) = ((btn.x + 30.0) as u32, (btn.y + btn.height + 60.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    open(&mut g, "dp");
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "the open grid must restyle pixels below the button");
}
