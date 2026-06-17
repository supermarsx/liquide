//! `<lq-date-picker>` — a trigger button + month-grid popup (Group D: D8 part 1).
//!
//! State: a currently-selected (year, month, day) plus the month currently being
//! browsed in the popup. The widget is a single subtree: a `data-part="button"`
//! trigger showing the selected date, and — when open — a `data-part="popup"`
//! month grid (`prev-month`/`next-month` nav + 7×N `day-<n>` cells for the days
//! of the displayed month). Behavior:
//!
//! - **Click the button**: toggles the popup.
//! - **Click prev/next month**: shifts the displayed month (popup stays open).
//! - **Click a day cell's LAID-OUT box** (`data-part="day-<n>"`): selects that
//!   date + closes; emits `Changed`(YYYY-MM-DD). Hit per-cell from layout, never
//!   `row*7+col` over a constant cell size.
//! - **Keyboard** (open): arrows move the focused day across the grid (Left/Right
//!   ±1 day, Up/Down ±7), Enter selects, Esc closes.
//!
//! Calendar math is self-contained (proleptic Gregorian; no chrono dependency):
//! [`days_in_month`] + [`day_of_week_first`] compute the grid; only the date math
//! is in Rust, the grid GEOMETRY comes from CSS/layout.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a date is selected (payload: `YYYY-MM-DD`).
pub const CHANGED_ACTION: &str = "changed";

const MONTH_NAMES: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];

/// Whether `year` is a leap year (proleptic Gregorian).
pub fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Number of days in `month` (1..=12) of `year`.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// The weekday (0=Sunday..6=Saturday) of the 1st of `month`/`year`, via Zeller's
/// congruence (self-contained, no chrono).
pub fn day_of_week_first(year: i32, month: u32) -> u32 {
    // Treat Jan/Feb as months 13/14 of the previous year (Zeller).
    let (m, y) = if month < 3 {
        (month + 12, year - 1)
    } else {
        (month, year)
    };
    let k = y % 100;
    let j = y / 100;
    let q = 1i32; // 1st of the month
    let h = (q + (13 * (m as i32 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    // Zeller: 0=Saturday,1=Sunday,...; convert to 0=Sunday..6=Saturday.
    ((h + 6) % 7) as u32
}

/// A date-picker widget.
#[derive(Debug, Clone)]
pub struct DatePicker {
    /// Selected (year, month 1..=12, day 1..=31), if any.
    selected: Option<(i32, u32, u32)>,
    /// The month currently displayed in the popup (year, month).
    view: (i32, u32),
    /// The keyboard-focused day in the displayed month (1..=days), if open.
    focus_day: u32,
    /// The hovered day cell.
    hovered: Option<u32>,
    open: bool,
}

impl DatePicker {
    /// A date picker initialised to view `year`/`month` (1..=12), nothing selected.
    pub fn new(year: i32, month: u32) -> Self {
        let month = month.clamp(1, 12);
        Self {
            selected: None,
            view: (year, month),
            focus_day: 1,
            hovered: None,
            open: false,
        }
    }

    /// Pre-select a date (also sets the displayed month).
    pub fn select(mut self, year: i32, month: u32, day: u32) -> Self {
        let month = month.clamp(1, 12);
        let day = day.clamp(1, days_in_month(year, month));
        self.selected = Some((year, month, day));
        self.view = (year, month);
        self.focus_day = day;
        self
    }

    /// Whether the popup is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The selected date, if any.
    pub fn selected(&self) -> Option<(i32, u32, u32)> {
        self.selected
    }

    /// The currently displayed (year, month).
    pub fn view(&self) -> (i32, u32) {
        self.view
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

    fn open_popup(&mut self) -> WidgetOutcome {
        if self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = true;
        if let Some((y, m, d)) = self.selected {
            self.view = (y, m);
            self.focus_day = d;
        }
        WidgetOutcome::Changed
    }

    fn close_popup(&mut self) -> WidgetOutcome {
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = false;
        self.hovered = None;
        WidgetOutcome::Changed
    }

    fn shift_month(&mut self, delta: i32) -> WidgetOutcome {
        let (mut y, mut m) = self.view;
        let mut mi = m as i32 - 1 + delta;
        while mi < 0 {
            mi += 12;
            y -= 1;
        }
        while mi >= 12 {
            mi -= 12;
            y += 1;
        }
        m = (mi + 1) as u32;
        self.view = (y, m);
        // Clamp the focused day into the new month.
        self.focus_day = self.focus_day.min(days_in_month(y, m));
        WidgetOutcome::Changed
    }

    fn choose_day(&mut self, day: u32) -> WidgetOutcome {
        let (y, m) = self.view;
        let day = day.clamp(1, days_in_month(y, m));
        let changed = self.selected != Some((y, m, day));
        self.selected = Some((y, m, day));
        self.focus_day = day;
        self.open = false;
        self.hovered = None;
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, Self::fmt_date(y, m, day))
        } else {
            WidgetOutcome::Changed
        }
    }

    /// Move the keyboard-focused day by `delta`, clamping into the month.
    fn move_focus(&mut self, delta: i32) -> WidgetOutcome {
        let (y, m) = self.view;
        let n = days_in_month(y, m) as i32;
        let nd = (self.focus_day as i32 + delta).clamp(1, n) as u32;
        if nd == self.focus_day {
            return WidgetOutcome::Ignored;
        }
        self.focus_day = nd;
        WidgetOutcome::Changed
    }

    fn day_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<u32> {
        let (y, m) = self.view;
        for d in 1..=days_in_month(y, m) {
            if let Some(r) = layout.box_of_part(root, &Self::day_part(d)) {
                if r.contains(point) {
                    return Some(d);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for DatePicker {
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
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                if !self.open {
                    return WidgetOutcome::Ignored;
                }
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
                if !self.open {
                    // Open only if the trigger button was hit.
                    if layout
                        .box_of_part(root, "button")
                        .map(|r| r.contains(p))
                        .unwrap_or(false)
                    {
                        return self.open_popup();
                    }
                    return WidgetOutcome::Ignored;
                }
                // Popup open: nav arrows, day cells, or dismiss.
                if layout
                    .box_of_part(root, "prev-month")
                    .map(|r| r.contains(p))
                    .unwrap_or(false)
                {
                    return self.shift_month(-1);
                }
                if layout
                    .box_of_part(root, "next-month")
                    .map(|r| r.contains(p))
                    .unwrap_or(false)
                {
                    return self.shift_month(1);
                }
                if let Some(d) = self.day_at(root, p, layout) {
                    return self.choose_day(d);
                }
                self.close_popup()
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
        match key.key {
            keys::ARROW_DOWN if !self.open => self.open_popup(),
            keys::ENTER if !self.open => self.open_popup(),
            keys::ARROW_LEFT => self.move_focus(-1),
            keys::ARROW_RIGHT => self.move_focus(1),
            keys::ARROW_UP => self.move_focus(-7),
            keys::ARROW_DOWN => self.move_focus(7),
            keys::ENTER => self.choose_day(self.focus_day),
            keys::ESCAPE => self.close_popup(),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn render(&self) -> TemplateNode {
        let (vy, vm) = self.view;
        let mut root = TemplateNode::el("lq-date-picker")
            .attr(FOCUSABLE_ATTR, "true")
            .attr("aria-expanded", if self.open { "true" } else { "false" })
            .class_if("open", self.open)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.open);

        let button_text = match self.selected {
            Some((y, m, d)) => Self::fmt_date(y, m, d),
            None => "Pick a date…".to_string(),
        };
        root = root.child(
            TemplateNode::el("lq-date-button")
                .attr("data-part", "button")
                .class_if("placeholder", self.selected.is_none())
                .child(TemplateNode::text(&button_text)),
        );

        if self.open {
            let mut popup = TemplateNode::el("lq-popup").attr("data-part", "popup");

            // Month nav header.
            let header = TemplateNode::el("lq-date-nav")
                .child(
                    TemplateNode::el("lq-date-prev")
                        .attr("data-part", "prev-month")
                        .child(TemplateNode::text("‹")),
                )
                .child(
                    TemplateNode::el("lq-date-title")
                        .attr("data-part", "title")
                        .child(TemplateNode::text(&format!(
                            "{} {vy}",
                            MONTH_NAMES[(vm - 1) as usize]
                        ))),
                )
                .child(
                    TemplateNode::el("lq-date-next")
                        .attr("data-part", "next-month")
                        .child(TemplateNode::text("›")),
                );
            popup = popup.child(header);

            // The day grid: leading blanks for the 1st's weekday, then days.
            let mut grid = TemplateNode::el("lq-date-grid");
            let lead = day_of_week_first(vy, vm);
            for b in 0..lead {
                grid = grid.child(
                    TemplateNode::el("lq-date-blank")
                        .key(&format!("blank-{b}"))
                        .attr("data-part", "blank"),
                );
            }
            for d in 1..=days_in_month(vy, vm) {
                let is_sel = self.selected == Some((vy, vm, d));
                let is_focus = self.focus_day == d;
                let cell = TemplateNode::el("lq-date-day")
                    .key(&format!("day-{vy}-{vm}-{d}"))
                    .attr("data-part", &Self::day_part(d))
                    .attr("data-day", &format!("{d}"))
                    .attr("role", "button")
                    .class_if("selected", is_sel)
                    .pseudo_if(PseudoStateFlags::CHECKED, is_sel)
                    .pseudo_if(PseudoStateFlags::FOCUS, is_focus)
                    .pseudo_if(PseudoStateFlags::HOVER, self.hovered == Some(d))
                    .child(TemplateNode::text(&d.to_string()));
                grid = grid.child(cell);
            }
            popup = popup.child(grid);
            root = root.child(popup);
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
