//! Do-Not-Disturb (DND) scheduling.
//!
//! [`DndSchedule`] manages time-based DND rules. Each [`DndTimeRange`] defines
//! a time window and the days of the week it applies to. The schedule can
//! handle ranges that cross midnight (e.g. 22:00 – 07:00).

use serde::{Deserialize, Serialize};

/// A time range during which Do-Not-Disturb is active.
///
/// Hours are 0..=23, minutes are 0..=59. Days of the week use 0=Sunday
/// through 6=Saturday (matching `chrono::Weekday::num_days_from_sunday()`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DndTimeRange {
    /// Start hour (0..=23).
    pub start_hour: u8,
    /// Start minute (0..=59).
    pub start_minute: u8,
    /// End hour (0..=23).
    pub end_hour: u8,
    /// End minute (0..=59).
    pub end_minute: u8,
    /// Days of the week this range applies to. Empty = every day.
    /// 0=Sunday, 1=Monday, ... 6=Saturday.
    pub days: Vec<u8>,
}

impl DndTimeRange {
    /// Creates a new range that applies every day.
    pub fn new(start_hour: u8, start_minute: u8, end_hour: u8, end_minute: u8) -> Self {
        Self {
            start_hour,
            start_minute,
            end_hour,
            end_minute,
            days: Vec::new(),
        }
    }

    /// Builder: restrict this range to specific days of the week.
    pub fn with_days(mut self, days: Vec<u8>) -> Self {
        self.days = days;
        self
    }

    /// Converts a time to minutes since midnight for comparison.
    fn to_minutes(hour: u8, minute: u8) -> u16 {
        hour as u16 * 60 + minute as u16
    }

    /// Checks if a given time falls within this range on the given day.
    ///
    /// For ranges that cross midnight (start > end), the check is split:
    /// the "start day" covers start..23:59 and the "next day" covers 00:00..end.
    pub fn contains(&self, hour: u8, minute: u8, day_of_week: u8) -> bool {
        let now = Self::to_minutes(hour, minute);
        let start = Self::to_minutes(self.start_hour, self.start_minute);
        let end = Self::to_minutes(self.end_hour, self.end_minute);

        if crosses_midnight(self) {
            // Range wraps around midnight.
            // Two sub-ranges:
            //   (a) start..23:59 on `day_of_week`
            //   (b) 00:00..end on `(day_of_week + 1) % 7`
            //
            // We need to check both perspectives:
            //   - Are we in the evening portion (a)? Then `day_of_week` must match.
            //   - Are we in the morning portion (b)? Then `(day_of_week - 1 + 7) % 7` must match.
            if now >= start {
                // Evening portion — the schedule's day must include today.
                self.day_matches(day_of_week)
            } else if now < end {
                // Morning portion — the schedule's day must include yesterday.
                let yesterday = (day_of_week + 6) % 7;
                self.day_matches(yesterday)
            } else {
                false
            }
        } else {
            // Normal range (does not cross midnight).
            if now >= start && now < end {
                self.day_matches(day_of_week)
            } else {
                false
            }
        }
    }

    /// Checks if a day matches this range's day filter.
    fn day_matches(&self, day: u8) -> bool {
        if self.days.is_empty() {
            true // No day filter = every day.
        } else {
            self.days.contains(&day)
        }
    }
}

/// Returns true if the time range crosses midnight (e.g. 22:00–07:00).
pub fn crosses_midnight(range: &DndTimeRange) -> bool {
    let start = DndTimeRange::to_minutes(range.start_hour, range.start_minute);
    let end = DndTimeRange::to_minutes(range.end_hour, range.end_minute);
    start >= end
}

/// A Do-Not-Disturb schedule consisting of zero or more time ranges.
///
/// When DND is active, notifications with urgency below Critical are
/// silently logged but not displayed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DndSchedule {
    /// Master toggle. If false, the schedule is ignored regardless of time ranges.
    pub enabled: bool,
    /// Time ranges during which DND is active.
    pub schedules: Vec<DndTimeRange>,
    /// If true, DND is forced on regardless of schedules (manual override).
    pub manual_override: bool,
}

impl DndSchedule {
    /// Creates a new disabled DND schedule.
    pub fn new() -> Self {
        Self {
            enabled: false,
            schedules: Vec::new(),
            manual_override: false,
        }
    }

    /// Checks whether DND is active at the given time.
    ///
    /// Returns true if:
    /// - `manual_override` is true, OR
    /// - `enabled` is true AND any schedule range matches the given time.
    pub fn is_active(&self, hour: u8, minute: u8, day_of_week: u8) -> bool {
        if self.manual_override {
            return true;
        }
        if !self.enabled {
            return false;
        }
        self.schedules
            .iter()
            .any(|r| r.contains(hour, minute, day_of_week))
    }

    /// Adds a time range to the schedule.
    pub fn add_schedule(&mut self, range: DndTimeRange) {
        self.schedules.push(range);
    }

    /// Removes a time range by index. Returns the removed range, or `None` if
    /// the index is out of bounds.
    pub fn remove_schedule(&mut self, index: usize) -> Option<DndTimeRange> {
        if index < self.schedules.len() {
            Some(self.schedules.remove(index))
        } else {
            None
        }
    }

    /// Returns the number of configured time ranges.
    pub fn schedule_count(&self) -> usize {
        self.schedules.len()
    }

    /// Enables the schedule (time ranges will be checked).
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables the schedule (no automatic DND).
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Sets the manual override. When true, DND is always active.
    pub fn set_manual_override(&mut self, on: bool) {
        self.manual_override = on;
    }
}
