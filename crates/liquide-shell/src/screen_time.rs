//! Screen time tracking — iOS/Android Digital Wellbeing-style usage statistics.
//!
//! Aggregates focus and open events into persistent daily and hourly summaries,
//! supports app categories, usage limits, pickup detection, and daily comparison.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::window::WindowId;

const US_PER_SECOND: u64 = 1_000_000;
const US_PER_HOUR: u64 = 3_600 * US_PER_SECOND;
const US_PER_DAY: u64 = 24 * US_PER_HOUR;

/// Hourly usage slot within a day.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HourlySlot {
    pub screen_time_us: u64,
    pub launch_count: u32,
    pub focus_switches: u32,
}

impl fmt::Display for HourlySlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HourlySlot(screen={}us, launches={}, switches={})",
            self.screen_time_us, self.launch_count, self.focus_switches,
        )
    }
}

/// Per-app screen time for a single day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppScreenTime {
    pub app_id: String,
    pub screen_time_us: u64,
    pub background_time_us: u64,
    pub launch_count: u32,
    pub session_count: u32,
    pub avg_session_us: u64,
    pub longest_session_us: u64,
    pub first_used_us: u64,
    pub last_used_us: u64,
}

impl fmt::Display for AppScreenTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AppScreenTime(app={}, screen={}us, launches={}, sessions={})",
            self.app_id, self.screen_time_us, self.launch_count, self.session_count,
        )
    }
}

/// Per-category aggregated screen time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryScreenTime {
    pub category: String,
    pub screen_time_us: u64,
    pub app_count: u32,
    pub launch_count: u32,
}

impl fmt::Display for CategoryScreenTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CategoryScreenTime(cat={}, screen={}us, apps={}, launches={})",
            self.category, self.screen_time_us, self.app_count, self.launch_count,
        )
    }
}

/// What a usage limit applies to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LimitTarget {
    App(String),
    Category(String),
    AllApps,
}

impl fmt::Display for LimitTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::App(id) => write!(f, "App({id})"),
            Self::Category(cat) => write!(f, "Category({cat})"),
            Self::AllApps => write!(f, "AllApps"),
        }
    }
}

/// A daily usage limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageLimit {
    pub target: LimitTarget,
    pub daily_limit_us: u64,
}

impl fmt::Display for UsageLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UsageLimit({}, limit={}us)",
            self.target, self.daily_limit_us,
        )
    }
}

/// Alert when a usage limit is approached or exceeded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenTimeAlert {
    pub target: LimitTarget,
    pub limit_us: u64,
    pub used_us: u64,
    pub exceeded: bool,
    pub percent_used: f64,
}

impl fmt::Display for ScreenTimeAlert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.exceeded { "EXCEEDED" } else { "ok" };
        write!(
            f,
            "Alert({}, {}/{} us, {:.0}%, {})",
            self.target, self.used_us, self.limit_us, self.percent_used, status,
        )
    }
}

/// Comparison between two calendar days.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyComparison {
    pub day_a: u32,
    pub day_b: u32,
    pub screen_time_a_us: u64,
    pub screen_time_b_us: u64,
    pub delta_us: i64,
    pub percent_change: f64,
    pub launches_a: u32,
    pub launches_b: u32,
    pub pickups_a: u32,
    pub pickups_b: u32,
}

impl fmt::Display for DailyComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.delta_us >= 0 { "+" } else { "" };
        write!(
            f,
            "DailyComparison(day {} vs {}, {sign}{}us, {:.1}%)",
            self.day_a, self.day_b, self.delta_us, self.percent_change,
        )
    }
}

/// Weekly summary of usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklySummary {
    pub days_tracked: u32,
    pub total_screen_time_us: u64,
    pub daily_average_us: u64,
    pub most_used_app: Option<(String, u64)>,
    pub total_pickups: u32,
    pub total_launches: u32,
}

impl fmt::Display for WeeklySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WeeklySummary(days={}, total={}us, avg={}us/day, pickups={}, launches={})",
            self.days_tracked,
            self.total_screen_time_us,
            self.daily_average_us,
            self.total_pickups,
            self.total_launches,
        )
    }
}

/// Full daily usage report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyReport {
    pub day_key: u32,
    pub total_screen_time_us: u64,
    pub total_background_time_us: u64,
    pub total_app_launches: u32,
    pub pickup_count: u32,
    pub first_active_us: u64,
    pub last_active_us: u64,
    pub hourly: [HourlySlot; 24],
    pub apps: HashMap<String, AppScreenTime>,
}

impl DailyReport {
    /// Create an empty daily report.
    #[must_use]
    pub fn new(day_key: u32) -> Self {
        Self {
            day_key,
            total_screen_time_us: 0,
            total_background_time_us: 0,
            total_app_launches: 0,
            pickup_count: 0,
            first_active_us: 0,
            last_active_us: 0,
            hourly: std::array::from_fn(|_| HourlySlot::default()),
            apps: HashMap::new(),
        }
    }

    /// Category breakdown using the provided category mapping.
    #[must_use]
    pub fn category_breakdown(
        &self,
        categories: &HashMap<String, String>,
    ) -> HashMap<String, CategoryScreenTime> {
        let mut result: HashMap<String, CategoryScreenTime> = HashMap::new();
        for (app_id, app_st) in &self.apps {
            let cat = categories
                .get(app_id)
                .cloned()
                .unwrap_or_else(|| "Uncategorized".to_string());
            let entry = result
                .entry(cat.clone())
                .or_insert_with(|| CategoryScreenTime {
                    category: cat,
                    screen_time_us: 0,
                    app_count: 0,
                    launch_count: 0,
                });
            entry.screen_time_us += app_st.screen_time_us;
            entry.app_count += 1;
            entry.launch_count += app_st.launch_count;
        }
        result
    }

    /// Top N apps by screen time (descending).
    #[must_use]
    pub fn top_apps(&self, n: usize) -> Vec<&AppScreenTime> {
        let mut apps: Vec<&AppScreenTime> = self.apps.values().collect();
        apps.sort_by(|a, b| b.screen_time_us.cmp(&a.screen_time_us));
        apps.truncate(n);
        apps
    }

    /// The hour with the most screen time.
    #[must_use]
    pub fn peak_hour(&self) -> Option<(u8, u64)> {
        let mut best: Option<(u8, u64)> = None;
        for (h, slot) in self.hourly.iter().enumerate() {
            if slot.screen_time_us > 0 {
                match best {
                    None => best = Some((h as u8, slot.screen_time_us)),
                    Some((_, prev)) if slot.screen_time_us > prev => {
                        best = Some((h as u8, slot.screen_time_us));
                    }
                    _ => {}
                }
            }
        }
        best
    }

    fn touch_activity(&mut self, wall_us: u64) {
        if self.first_active_us == 0 || wall_us < self.first_active_us {
            self.first_active_us = wall_us;
        }
        if wall_us > self.last_active_us {
            self.last_active_us = wall_us;
        }
    }

    fn get_app_mut(&mut self, app_id: &str) -> &mut AppScreenTime {
        self.apps
            .entry(app_id.to_string())
            .or_insert_with(|| AppScreenTime {
                app_id: app_id.to_string(),
                screen_time_us: 0,
                background_time_us: 0,
                launch_count: 0,
                session_count: 0,
                avg_session_us: 0,
                longest_session_us: 0,
                first_used_us: 0,
                last_used_us: 0,
            })
    }
}

impl fmt::Display for DailyReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DailyReport(day={}, screen={}us, launches={}, pickups={}, apps={})",
            self.day_key,
            self.total_screen_time_us,
            self.total_app_launches,
            self.pickup_count,
            self.apps.len(),
        )
    }
}

/// Tracks the currently focused window/app.
struct FocusSession {
    app_id: String,
    #[allow(dead_code)]
    window_id: WindowId,
    start_wall_us: u64,
    start_day_key: u32,
    #[allow(dead_code)]
    start_hour: u8,
}

/// iOS/Android-style screen time tracker.
///
/// Receives events from Shell and maintains aggregated daily/hourly summaries.
pub struct ScreenTimeTracker {
    wall_anchor_us: u64,
    mono_anchor: u64,
    tick_duration_us: u64,
    days: HashMap<u32, DailyReport>,
    categories: HashMap<String, String>,
    limits: Vec<UsageLimit>,
    current_focus: Option<FocusSession>,
    idle_threshold_us: u64,
    last_activity_wall_us: u64,
    max_days_retained: usize,
}

impl ScreenTimeTracker {
    /// Create a new tracker anchored to the given wall-clock and monotonic timestamps.
    #[must_use]
    pub fn new(wall_anchor_us: u64, mono_anchor: u64) -> Self {
        Self {
            wall_anchor_us,
            mono_anchor,
            tick_duration_us: 1,
            days: HashMap::new(),
            categories: HashMap::new(),
            limits: Vec::new(),
            current_focus: None,
            idle_threshold_us: 30 * US_PER_SECOND,
            last_activity_wall_us: wall_anchor_us,
            max_days_retained: 90,
        }
    }

    /// Create with a custom tick duration (us per monotonic tick).
    #[must_use]
    pub fn with_tick_duration(
        wall_anchor_us: u64,
        mono_anchor: u64,
        tick_duration_us: u64,
    ) -> Self {
        let mut t = Self::new(wall_anchor_us, mono_anchor);
        t.tick_duration_us = tick_duration_us.max(1);
        t
    }

    /// Convert a monotonic timestamp to wall-clock microseconds.
    #[must_use]
    pub fn to_wall_clock(&self, mono_ts: u64) -> u64 {
        let delta = mono_ts.saturating_sub(self.mono_anchor);
        self.wall_anchor_us + delta * self.tick_duration_us
    }

    /// Convert wall-clock us to day key (days since Unix epoch).
    #[must_use]
    pub fn day_key(wall_us: u64) -> u32 {
        (wall_us / US_PER_DAY) as u32
    }

    /// Convert wall-clock us to hour of day (0-23).
    #[must_use]
    pub fn hour_of_day(wall_us: u64) -> u8 {
        ((wall_us % US_PER_DAY) / US_PER_HOUR) as u8
    }

    /// Wall-clock us at start of a day.
    fn day_start_us(day_key: u32) -> u64 {
        day_key as u64 * US_PER_DAY
    }

    /// Record an app window open.
    pub fn feed_open(&mut self, app_id: &str, _window_id: WindowId, mono_ts: u64) {
        if app_id.is_empty() {
            return;
        }
        let wall_us = self.to_wall_clock(mono_ts);
        let dk = Self::day_key(wall_us);
        let hour = Self::hour_of_day(wall_us);

        // Pickup detection
        if wall_us.saturating_sub(self.last_activity_wall_us) > self.idle_threshold_us {
            self.ensure_day(dk).pickup_count += 1;
        }

        let day = self.ensure_day(dk);
        day.touch_activity(wall_us);
        day.total_app_launches += 1;
        day.hourly[hour as usize].launch_count += 1;

        let app = day.get_app_mut(app_id);
        app.launch_count += 1;
        if app.first_used_us == 0 {
            app.first_used_us = wall_us;
        }
        app.last_used_us = wall_us;

        self.last_activity_wall_us = wall_us;
        self.evict_old_days();
    }

    /// Record an app window close.
    pub fn feed_close(&mut self, app_id: &str, _window_id: WindowId, mono_ts: u64) {
        if app_id.is_empty() {
            return;
        }
        let wall_us = self.to_wall_clock(mono_ts);
        let dk = Self::day_key(wall_us);

        let day = self.ensure_day(dk);
        day.touch_activity(wall_us);

        let app = day.get_app_mut(app_id);
        app.session_count += 1;
        app.last_used_us = wall_us;

        // Recompute avg_session_us
        if app.session_count > 0 && app.screen_time_us > 0 {
            app.avg_session_us = app.screen_time_us / app.session_count as u64;
        }

        self.last_activity_wall_us = wall_us;
    }

    /// Record that a window/app gained focus.
    pub fn feed_focus(&mut self, app_id: &str, window_id: WindowId, mono_ts: u64) {
        if app_id.is_empty() {
            return;
        }
        // Flush any existing focus session first
        self.flush_focus(mono_ts);

        let wall_us = self.to_wall_clock(mono_ts);
        let dk = Self::day_key(wall_us);
        let hour = Self::hour_of_day(wall_us);

        // Pickup detection
        if wall_us.saturating_sub(self.last_activity_wall_us) > self.idle_threshold_us {
            self.ensure_day(dk).pickup_count += 1;
        }

        let day = self.ensure_day(dk);
        day.touch_activity(wall_us);
        day.hourly[hour as usize].focus_switches += 1;

        self.current_focus = Some(FocusSession {
            app_id: app_id.to_string(),
            window_id,
            start_wall_us: wall_us,
            start_day_key: dk,
            start_hour: hour,
        });

        self.last_activity_wall_us = wall_us;
    }

    /// Record that the focused window lost focus.
    pub fn feed_unfocus(&mut self, mono_ts: u64) {
        self.flush_focus(mono_ts);
        let wall_us = self.to_wall_clock(mono_ts);
        self.last_activity_wall_us = wall_us;
    }

    /// Flush the current focus session, attributing screen time to the correct days/hours.
    fn flush_focus(&mut self, mono_ts: u64) {
        let session = match self.current_focus.take() {
            Some(s) => s,
            None => return,
        };

        let end_wall_us = self.to_wall_clock(mono_ts);
        if end_wall_us <= session.start_wall_us {
            return;
        }

        let total_duration = end_wall_us - session.start_wall_us;
        let start_dk = session.start_day_key;
        let _end_dk = Self::day_key(end_wall_us);

        // Attribute screen time, potentially splitting across days and hours
        let mut cursor = session.start_wall_us;

        // Walk from start to end, day by day
        let mut current_dk = start_dk;
        while cursor < end_wall_us {
            let next_day_start = Self::day_start_us(current_dk + 1);
            let segment_end = end_wall_us.min(next_day_start);
            let segment_duration = segment_end - cursor;

            if segment_duration > 0 {
                let day = self.ensure_day(current_dk);
                day.total_screen_time_us += segment_duration;
                day.touch_activity(cursor);

                // Attribute to hourly slots within this day
                let mut hour_cursor = cursor;
                while hour_cursor < segment_end {
                    let h = Self::hour_of_day(hour_cursor);
                    let next_hour_start =
                        Self::day_start_us(current_dk) + (h as u64 + 1) * US_PER_HOUR;
                    let hour_end = segment_end.min(next_hour_start);
                    let hour_duration = hour_end - hour_cursor;
                    if hour_duration > 0 {
                        day.hourly[h as usize].screen_time_us += hour_duration;
                    }
                    hour_cursor = hour_end;
                }

                // Attribute to app
                let app = day.get_app_mut(&session.app_id);
                app.screen_time_us += segment_duration;
                app.last_used_us = segment_end;
                if app.first_used_us == 0 {
                    app.first_used_us = cursor;
                }
            }

            cursor = next_day_start;
            current_dk += 1;
        }

        // Update longest_session across relevant days
        // The total_duration represents the full focus session length
        let primary_dk = start_dk;
        if let Some(day) = self.days.get_mut(&primary_dk) {
            if let Some(app) = day.apps.get_mut(&session.app_id) {
                if total_duration > app.longest_session_us {
                    app.longest_session_us = total_duration;
                }
            }
        }
    }

    /// Set the app category.
    pub fn set_category(&mut self, app_id: &str, category: &str) {
        self.categories
            .insert(app_id.to_string(), category.to_string());
    }

    /// Remove an app's category assignment.
    pub fn remove_category(&mut self, app_id: &str) {
        self.categories.remove(app_id);
    }

    /// Add a usage limit.
    pub fn add_limit(&mut self, limit: UsageLimit) {
        self.limits.push(limit);
    }

    /// Remove a usage limit by index.
    pub fn remove_limit(&mut self, index: usize) {
        if index < self.limits.len() {
            self.limits.remove(index);
        }
    }

    /// Get the configured usage limits.
    #[must_use]
    pub fn limits(&self) -> &[UsageLimit] {
        &self.limits
    }

    /// Set the idle threshold for pickup detection.
    pub fn set_idle_threshold(&mut self, us: u64) {
        self.idle_threshold_us = us;
    }

    /// Get the daily report for a specific day.
    #[must_use]
    pub fn daily_report(&self, day_key: u32) -> Option<&DailyReport> {
        self.days.get(&day_key)
    }

    /// Get the daily report for the current day based on a monotonic timestamp.
    #[must_use]
    pub fn today(&self, now_mono: u64) -> Option<&DailyReport> {
        let wall = self.to_wall_clock(now_mono);
        let dk = Self::day_key(wall);
        self.days.get(&dk)
    }

    /// Compare two calendar days.
    #[must_use]
    pub fn compare_days(&self, day_a: u32, day_b: u32) -> Option<DailyComparison> {
        let a = self.days.get(&day_a)?;
        let b = self.days.get(&day_b)?;

        let delta = b.total_screen_time_us as i64 - a.total_screen_time_us as i64;
        let pct = if a.total_screen_time_us > 0 {
            delta as f64 / a.total_screen_time_us as f64 * 100.0
        } else {
            0.0
        };

        Some(DailyComparison {
            day_a,
            day_b,
            screen_time_a_us: a.total_screen_time_us,
            screen_time_b_us: b.total_screen_time_us,
            delta_us: delta,
            percent_change: pct,
            launches_a: a.total_app_launches,
            launches_b: b.total_app_launches,
            pickups_a: a.pickup_count,
            pickups_b: b.pickup_count,
        })
    }

    /// Check all usage limits for a given day.
    #[must_use]
    pub fn check_limits(&self, day_key: u32) -> Vec<ScreenTimeAlert> {
        let day = match self.days.get(&day_key) {
            Some(d) => d,
            None => return Vec::new(),
        };

        self.limits
            .iter()
            .map(|limit| {
                let used = match &limit.target {
                    LimitTarget::App(id) => day.apps.get(id).map_or(0, |a| a.screen_time_us),
                    LimitTarget::Category(cat) => day
                        .apps
                        .iter()
                        .filter(|(aid, _)| self.categories.get(*aid) == Some(cat))
                        .map(|(_, a)| a.screen_time_us)
                        .sum(),
                    LimitTarget::AllApps => day.total_screen_time_us,
                };

                let pct = if limit.daily_limit_us > 0 {
                    used as f64 / limit.daily_limit_us as f64 * 100.0
                } else {
                    0.0
                };

                ScreenTimeAlert {
                    target: limit.target.clone(),
                    limit_us: limit.daily_limit_us,
                    used_us: used,
                    exceeded: used > limit.daily_limit_us,
                    percent_used: pct,
                }
            })
            .collect()
    }

    /// Compute weekly summary for 7 days ending at `end_day`.
    #[must_use]
    pub fn weekly_average(&self, end_day: u32) -> WeeklySummary {
        let start_day = end_day.saturating_sub(6);
        let mut total_st: u64 = 0;
        let mut total_pickups: u32 = 0;
        let mut total_launches: u32 = 0;
        let mut days_tracked: u32 = 0;
        let mut app_totals: HashMap<String, u64> = HashMap::new();

        for dk in start_day..=end_day {
            if let Some(day) = self.days.get(&dk) {
                days_tracked += 1;
                total_st += day.total_screen_time_us;
                total_pickups += day.pickup_count;
                total_launches += day.total_app_launches;
                for (app_id, app_st) in &day.apps {
                    *app_totals.entry(app_id.clone()).or_insert(0) += app_st.screen_time_us;
                }
            }
        }

        let daily_avg = if days_tracked > 0 {
            total_st / days_tracked as u64
        } else {
            0
        };

        let most_used = app_totals
            .into_iter()
            .max_by_key(|(_, st)| *st)
            .filter(|(_, st)| *st > 0);

        WeeklySummary {
            days_tracked,
            total_screen_time_us: total_st,
            daily_average_us: daily_avg,
            most_used_app: most_used,
            total_pickups,
            total_launches,
        }
    }

    /// Number of days currently tracked.
    #[must_use]
    pub fn tracked_days(&self) -> usize {
        self.days.len()
    }

    /// Get the category mapping.
    #[must_use]
    pub fn categories(&self) -> &HashMap<String, String> {
        &self.categories
    }

    fn ensure_day(&mut self, dk: u32) -> &mut DailyReport {
        self.days.entry(dk).or_insert_with(|| DailyReport::new(dk))
    }

    fn evict_old_days(&mut self) {
        if self.days.len() > self.max_days_retained {
            if let Some(&oldest) = self.days.keys().min() {
                self.days.remove(&oldest);
            }
        }
    }
}

impl fmt::Display for ScreenTimeTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ScreenTimeTracker(days={}, categories={}, limits={})",
            self.days.len(),
            self.categories.len(),
            self.limits.len(),
        )
    }
}
