//! `<lq-month-calendar>` — a standalone month grid (Group GRID: G6).
//!
//! Distinct from the [`DatePicker`](crate::date_picker::DatePicker) popup: this
//! is an ALWAYS-VISIBLE month grid (the Win32 `SysMonthCal32`), with a weekday
//! header row, prev/next month navigation, a "today" marker, and day-cell
//! selection. Behavior:
//!
//! - **Click prev/next** (`data-part="prev"`/`"next"`): shift the displayed month.
//! - **Click a day cell** (`data-part="day-<n>"`): select that date — hit per-cell
//!   from the LAID-OUT box, never `row*7+col` over a constant cell size.
//! - **Keyboard**: arrows move the focused day (Left/Right ±1, Up/Down ±7,
//!   crossing month boundaries), Enter selects, Home/End jump to the first/last
//!   day of the month, PageUp/PageDown change the month.
//! - The cell matching `today` carries a `.today` class + `:active` marker.
//! - Emits `Changed(YYYY-MM-DD)` on selection.
//!
//! Calendar math is shared with the date-picker module (proleptic Gregorian; no
//! chrono): [`days_in_month`] / [`day_of_week_first`] / [`is_leap`].

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::date_picker::{day_of_week_first, days_in_month};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a date is selected (payload: `YYYY-MM-DD`).
pub const CHANGED_ACTION: &str = "changed";
/// Emitted when the displayed month changes (payload: `YYYY-MM`).
pub const MONTH_ACTION: &str = "month";

const MONTH_NAMES: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];

const WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

/// A standalone month-calendar widget.
#[derive(Debug, Clone)]
pub struct MonthCalendar {
    /// The displayed (year, month 1..=12).
    view: (i32, u32),
    /// Selected (year, month, day), if any.
    selected: Option<(i32, u32, u32)>,
    /// The keyboard-focused day in the displayed month.
    focus_day: u32,
    /// "Today" (year, month, day) for the today marker, if set.
    today: Option<(i32, u32, u32)>,
    /// Hovered day cell.
    hovered: Option<u32>,
    disabled: bool,
}

impl MonthCalendar {
    /// A calendar showing `year`/`month` (1..=12), nothing selected.
    pub fn new(year: i32, month: u32) -> Self {
        let month = month.clamp(1, 12);
        Self {
            view: (year, month),
            selected: None,
            focus_day: 1,
            today: None,
            hovered: None,
            disabled: false,
        }
    }

    /// Pre-select a date (also sets the displayed month + focus).
    pub fn select(mut self, year: i32, month: u32, day: u32) -> Self {
        let month = month.clamp(1, 12);
        let day = day.clamp(1, days_in_month(year, month));
        self.selected = Some((year, month, day));
        self.view = (year, month);
        self.focus_day = day;
        self
    }

    /// Mark a date as "today" (renders the today marker when in view).
    pub fn today(mut self, year: i32, month: u32, day: u32) -> Self {
        let month = month.clamp(1, 12);
        let day = day.clamp(1, days_in_month(year, month));
        self.today = Some((year, month, day));
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The displayed (year, month).
    pub fn view(&self) -> (i32, u32) {
        self.view
    }

    /// The selected date, if any.
    pub fn selected(&self) -> Option<(i32, u32, u32)> {
        self.selected
    }

    /// The keyboard-focused day.
    pub fn focus_day(&self) -> u32 {
        self.focus_day
    }

    fn fmt_date(y: i32, m: u32, d: u32) -> String {
        format!("{y:04}-{m:02}-{d:02}")
    }

    fn day_part(d: u32) -> String {
        format!("day-{d}")
    }

    fn shift_month(&mut self, delta: i32) -> WidgetOutcome {
        let (mut y, m) = self.view;
        let mut mi = m as i32 - 1 + delta;
        while mi < 0 {
            mi += 12;
            y -= 1;
        }
        while mi >= 12 {
            mi -= 12;
            y += 1;
        }
        let nm = (mi + 1) as u32;
        self.view = (y, nm);
        self.focus_day = self.focus_day.min(days_in_month(y, nm));
        self.hovered = None;
        WidgetOutcome::action_with(MONTH_ACTION, format!("{y:04}-{nm:02}"))
    }

    fn choose_day(&mut self, day: u32) -> WidgetOutcome {
        let (y, m) = self.view;
        let day = day.clamp(1, days_in_month(y, m));
        let changed = self.selected != Some((y, m, day));
        self.selected = Some((y, m, day));
        self.focus_day = day;
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, Self::fmt_date(y, m, day))
        } else {
            WidgetOutcome::Changed
        }
    }

    /// Move the focused day by `delta`, crossing month boundaries (the grid is a
    /// continuous calendar). Returns Changed (or Ignored if nothing moved).
    fn move_focus(&mut self, delta: i32) -> WidgetOutcome {
        let (mut y, mut m) = self.view;
        let mut d = self.focus_day as i32 + delta;
        // Underflow: roll back into the previous month(s).
        while d < 1 {
            let mut mi = m as i32 - 2; // previous month index (0-based)
            if mi < 0 {
                mi += 12;
                y -= 1;
            }
            m = (mi + 1) as u32;
            d += days_in_month(y, m) as i32;
        }
        // Overflow: roll forward into the next month(s).
        loop {
            let dim = days_in_month(y, m) as i32;
            if d <= dim {
                break;
            }
            d -= dim;
            let mut mi = m as i32; // next month index (0-based) = m (since m is 1-based)
            if mi >= 12 {
                mi -= 12;
                y += 1;
            }
            m = (mi + 1) as u32;
        }
        let nd = d as u32;
        let moved_month = (y, m) != self.view;
        if !moved_month && nd == self.focus_day {
            return WidgetOutcome::Ignored;
        }
        self.view = (y, m);
        self.focus_day = nd;
        self.hovered = None;
        if moved_month {
            WidgetOutcome::action_with(MONTH_ACTION, format!("{y:04}-{m:02}"))
        } else {
            WidgetOutcome::Changed
        }
    }

    fn day_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<u32> {
        let (y, m) = self.view;
        for d in 1..=days_in_month(y, m) {
            if let Some(r) = layout.box_of_part(root, &Self::day_part(d)) {
                if r.contains(p) {
                    return Some(d);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for MonthCalendar {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
            DomEventKind::MouseLeave,
            DomEventKind::Click {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
        ]
    }

    fn on_dom_event(
        &mut self,
        root: NodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                let hit = self.day_at(root, Point::new(*x, *y), layout);
                if hit == self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = hit;
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                if layout.box_of_part(root, "prev").map(|r| r.contains(p)).unwrap_or(false) {
                    return self.shift_month(-1);
                }
                if layout.box_of_part(root, "next").map(|r| r.contains(p)).unwrap_or(false) {
                    return self.shift_month(1);
                }
                if let Some(d) = self.day_at(root, p, layout) {
                    return self.choose_day(d);
                }
                WidgetOutcome::Ignored
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        let (y, m) = self.view;
        match key.key {
            keys::ARROW_LEFT => self.move_focus(-1),
            keys::ARROW_RIGHT => self.move_focus(1),
            keys::ARROW_UP => self.move_focus(-7),
            keys::ARROW_DOWN => self.move_focus(7),
            keys::HOME => {
                if self.focus_day == 1 {
                    WidgetOutcome::Ignored
                } else {
                    self.focus_day = 1;
                    WidgetOutcome::Changed
                }
            }
            keys::END => {
                let last = days_in_month(y, m);
                if self.focus_day == last {
                    WidgetOutcome::Ignored
                } else {
                    self.focus_day = last;
                    WidgetOutcome::Changed
                }
            }
            keys::PAGE_UP => self.shift_month(-1),
            keys::PAGE_DOWN => self.shift_month(1),
            keys::ENTER | keys::SPACE => self.choose_day(self.focus_day),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let (vy, vm) = self.view;
        let mut root = TemplateNode::el("lq-month-calendar")
            .attr("role", "grid")
            .attr("data-view", &format!("{vy:04}-{vm:02}"))
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        // Header: prev | "Month YYYY" | next.
        let header = TemplateNode::el("lq-cal-nav")
            .child(
                TemplateNode::el("lq-cal-prev")
                    .attr("data-part", "prev")
                    .attr("role", "button")
                    .attr("aria-label", "Previous month")
                    .child(TemplateNode::text("\u{2039}")), // ‹
            )
            .child(
                TemplateNode::el("lq-cal-title")
                    .attr("data-part", "title")
                    .child(TemplateNode::text(&format!(
                        "{} {vy}",
                        MONTH_NAMES[(vm - 1) as usize]
                    ))),
            )
            .child(
                TemplateNode::el("lq-cal-next")
                    .attr("data-part", "next")
                    .attr("role", "button")
                    .attr("aria-label", "Next month")
                    .child(TemplateNode::text("\u{203A}")), // ›
            );
        root = root.child(header);

        // Weekday header row.
        let mut wk = TemplateNode::el("lq-cal-weekdays").attr("data-part", "weekdays");
        for (i, name) in WEEKDAYS.iter().enumerate() {
            wk = wk.child(
                TemplateNode::el("lq-cal-weekday")
                    .key(&format!("wd-{i}"))
                    .child(TemplateNode::text(name)),
            );
        }
        root = root.child(wk);

        // Day grid.
        let mut grid = TemplateNode::el("lq-cal-grid").attr("data-part", "grid");
        let lead = day_of_week_first(vy, vm);
        for b in 0..lead {
            grid = grid.child(
                TemplateNode::el("lq-cal-blank")
                    .key(&format!("blank-{b}"))
                    .attr("data-part", "blank"),
            );
        }
        for d in 1..=days_in_month(vy, vm) {
            let is_sel = self.selected == Some((vy, vm, d));
            let is_focus = self.focus_day == d && !self.disabled;
            let is_today = self.today == Some((vy, vm, d));
            let cell = TemplateNode::el("lq-cal-day")
                .key(&format!("day-{vy}-{vm}-{d}"))
                .attr("data-part", &Self::day_part(d))
                .attr("data-day", &format!("{d}"))
                .attr("role", "gridcell")
                .attr("aria-selected", if is_sel { "true" } else { "false" })
                .class_if("selected", is_sel)
                .class_if("today", is_today)
                .pseudo_if(PseudoStateFlags::CHECKED, is_sel)
                .pseudo_if(PseudoStateFlags::FOCUS, is_focus)
                .pseudo_if(PseudoStateFlags::ACTIVE, is_today)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(d) && !self.disabled,
                )
                .child(TemplateNode::text(&d.to_string()));
            grid = grid.child(cell);
        }
        root = root.child(grid);

        if self.disabled {
            root = root.attr("disabled", "true");
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
