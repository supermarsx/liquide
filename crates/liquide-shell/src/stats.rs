//! Usage statistics — computed on-demand from window and app history.

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::app_history::AppHistory;
use crate::history::{WindowEventKind, WindowHistory};
use crate::window::{WindowId, WindowState};

/// Per-window computed statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowStats {
    pub window_id: WindowId,
    pub opened_at: Option<u64>,
    pub closed_at: Option<u64>,
    pub runtime_us: Option<u64>,
    pub focus_time_us: u64,
    pub focus_count: u32,
    pub move_count: u32,
    pub resize_count: u32,
    pub state_change_count: u32,
    pub title_change_count: u32,
    pub z_order_change_count: u32,
    pub visibility_change_count: u32,
    pub flags_change_count: u32,
    pub total_event_count: u32,
    pub total_distance_moved: f64,
    pub last_state: Option<WindowState>,
    pub time_in_state: HashMap<String, u64>,
}

impl fmt::Display for WindowStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WindowStats({}, events={}, runtime={:?}, focus={}us, moves={}, resizes={})",
            self.window_id,
            self.total_event_count,
            self.runtime_us,
            self.focus_time_us,
            self.move_count,
            self.resize_count,
        )
    }
}

/// Per-application aggregate statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppStats {
    pub app_id: String,
    pub total_runtime_us: u64,
    pub avg_session_duration_us: u64,
    pub min_session_duration_us: Option<u64>,
    pub max_session_duration_us: Option<u64>,
    pub total_sessions: u32,
    pub closed_sessions: u32,
    pub active_sessions: u32,
    pub total_focus_time_us: u64,
    pub total_move_count: u32,
    pub total_resize_count: u32,
    pub total_event_count: u32,
    pub first_seen: u64,
    pub last_seen: u64,
}

impl fmt::Display for AppStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AppStats(app={}, sessions={}, runtime={}us, focus={}us)",
            self.app_id, self.total_sessions, self.total_runtime_us, self.total_focus_time_us,
        )
    }
}

/// System-wide aggregate statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemStats {
    pub total_windows_opened: u32,
    pub total_windows_closed: u32,
    pub currently_open: u32,
    pub total_events: u32,
    pub total_runtime_us: u64,
    pub avg_window_runtime_us: u64,
    pub total_focus_switches: u32,
    pub total_moves: u32,
    pub total_resizes: u32,
    pub unique_apps: u32,
    pub most_focused_window: Option<(WindowId, u64)>,
    pub most_active_app: Option<(String, u64)>,
    pub longest_session: Option<(WindowId, u64)>,
    pub shortest_session: Option<(WindowId, u64)>,
    pub timestamp_range: Option<(u64, u64)>,
    pub events_per_window_avg: f64,
}

impl fmt::Display for SystemStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SystemStats(windows={}/{} open, events={}, runtime={}us, apps={})",
            self.currently_open,
            self.total_windows_opened,
            self.total_events,
            self.total_runtime_us,
            self.unique_apps,
        )
    }
}

/// On-demand statistics collector over window and application history.
pub struct StatsCollector<'a> {
    window_history: &'a WindowHistory,
    app_history: &'a AppHistory,
}

impl<'a> StatsCollector<'a> {
    /// Create a new stats collector.
    #[must_use]
    pub fn new(window_history: &'a WindowHistory, app_history: &'a AppHistory) -> Self {
        Self {
            window_history,
            app_history,
        }
    }

    /// Compute statistics for a single window.
    #[must_use]
    pub fn window_stats(&self, id: WindowId) -> WindowStats {
        let events = self.window_history.events_for_window(id);

        let mut stats = WindowStats {
            window_id: id,
            opened_at: None,
            closed_at: None,
            runtime_us: None,
            focus_time_us: 0,
            focus_count: 0,
            move_count: 0,
            resize_count: 0,
            state_change_count: 0,
            title_change_count: 0,
            z_order_change_count: 0,
            visibility_change_count: 0,
            flags_change_count: 0,
            total_event_count: events.len() as u32,
            total_distance_moved: 0.0,
            last_state: None,
            time_in_state: HashMap::new(),
        };

        let mut focused_at: Option<u64> = None;
        let mut last_state_ts: Option<u64> = None;
        let mut current_state: Option<WindowState> = None;

        for event in &events {
            match &event.kind {
                WindowEventKind::Opened => {
                    if stats.opened_at.is_none() {
                        stats.opened_at = Some(event.timestamp_us);
                    }
                    // Opening implies Normal state
                    if current_state.is_none() {
                        current_state = Some(WindowState::Normal);
                        last_state_ts = Some(event.timestamp_us);
                    }
                }
                WindowEventKind::Closed => {
                    if stats.closed_at.is_none() {
                        stats.closed_at = Some(event.timestamp_us);
                    }
                    // Close cuts off any open focus interval
                    if let Some(focus_start) = focused_at.take() {
                        stats.focus_time_us += event.timestamp_us.saturating_sub(focus_start);
                    }
                    // Accumulate final state duration
                    if let (Some(state), Some(ts)) = (current_state, last_state_ts) {
                        let dur = event.timestamp_us.saturating_sub(ts);
                        *stats.time_in_state.entry(format!("{state}")).or_insert(0) += dur;
                    }
                    last_state_ts = None;
                    current_state = None;
                }
                WindowEventKind::Focused => {
                    stats.focus_count += 1;
                    focused_at = Some(event.timestamp_us);
                }
                WindowEventKind::Unfocused => {
                    if let Some(focus_start) = focused_at.take() {
                        stats.focus_time_us += event.timestamp_us.saturating_sub(focus_start);
                    }
                }
                WindowEventKind::Moved { from, to } => {
                    stats.move_count += 1;
                    let dx = (to.x - from.x) as f64;
                    let dy = (to.y - from.y) as f64;
                    stats.total_distance_moved += (dx * dx + dy * dy).sqrt();
                }
                WindowEventKind::Resized { .. } => {
                    stats.resize_count += 1;
                }
                WindowEventKind::StateChanged { from, to } => {
                    stats.state_change_count += 1;
                    // Accumulate duration in the previous state
                    if let Some(ts) = last_state_ts {
                        let dur = event.timestamp_us.saturating_sub(ts);
                        *stats.time_in_state.entry(format!("{from}")).or_insert(0) += dur;
                    }
                    current_state = Some(*to);
                    last_state_ts = Some(event.timestamp_us);
                    stats.last_state = Some(*to);
                }
                WindowEventKind::TitleChanged { .. } => {
                    stats.title_change_count += 1;
                }
                WindowEventKind::ZOrderChanged { .. } => {
                    stats.z_order_change_count += 1;
                }
                WindowEventKind::VisibilityChanged { .. } => {
                    stats.visibility_change_count += 1;
                }
                WindowEventKind::FlagsChanged { .. } => {
                    stats.flags_change_count += 1;
                }
            }
        }

        // Compute runtime
        if let (Some(opened), Some(closed)) = (stats.opened_at, stats.closed_at) {
            stats.runtime_us = Some(closed.saturating_sub(opened));
        }

        // Set last_state from Opened if no StateChanged was recorded
        if stats.last_state.is_none() && stats.opened_at.is_some() && stats.closed_at.is_none() {
            stats.last_state = Some(WindowState::Normal);
        }

        stats
    }

    /// Compute statistics for an application.
    #[must_use]
    pub fn app_stats(&self, app_id: &str) -> Option<AppStats> {
        let info = self.app_history.app_info(app_id)?;

        let mut total_runtime: u64 = 0;
        let mut closed_count: u32 = 0;
        let mut min_dur: Option<u64> = None;
        let mut max_dur: Option<u64> = None;

        for session in &info.sessions {
            if let Some(closed_at) = session.closed_at {
                let dur = closed_at.saturating_sub(session.opened_at);
                total_runtime += dur;
                closed_count += 1;
                min_dur = Some(min_dur.map_or(dur, |m: u64| m.min(dur)));
                max_dur = Some(max_dur.map_or(dur, |m: u64| m.max(dur)));
            }
        }

        let avg_dur = if closed_count > 0 {
            total_runtime / closed_count as u64
        } else {
            0
        };

        // Aggregate per-window stats for this app's windows
        let mut total_focus: u64 = 0;
        let mut total_moves: u32 = 0;
        let mut total_resizes: u32 = 0;
        let mut total_events: u32 = 0;

        for session in &info.sessions {
            let ws = self.window_stats(session.window_id);
            total_focus += ws.focus_time_us;
            total_moves += ws.move_count;
            total_resizes += ws.resize_count;
            total_events += ws.total_event_count;
        }

        let total_sessions = info.sessions.len() as u32;
        let active_sessions = info.active_window_count;

        Some(AppStats {
            app_id: app_id.to_string(),
            total_runtime_us: total_runtime,
            avg_session_duration_us: avg_dur,
            min_session_duration_us: min_dur,
            max_session_duration_us: max_dur,
            total_sessions,
            closed_sessions: closed_count,
            active_sessions,
            total_focus_time_us: total_focus,
            total_move_count: total_moves,
            total_resize_count: total_resizes,
            total_event_count: total_events,
            first_seen: info.first_seen,
            last_seen: info.last_seen,
        })
    }

    /// Compute system-wide statistics.
    #[must_use]
    pub fn system_stats(&self) -> SystemStats {
        let all_events = self.window_history.recent(self.window_history.len());
        let total_events = all_events.len() as u32;

        if total_events == 0 {
            return SystemStats {
                total_windows_opened: 0,
                total_windows_closed: 0,
                currently_open: 0,
                total_events: 0,
                total_runtime_us: 0,
                avg_window_runtime_us: 0,
                total_focus_switches: 0,
                total_moves: 0,
                total_resizes: 0,
                unique_apps: 0,
                most_focused_window: None,
                most_active_app: None,
                longest_session: None,
                shortest_session: None,
                timestamp_range: None,
                events_per_window_avg: 0.0,
            };
        }

        // Collect distinct window IDs and basic counts
        let mut window_ids: HashSet<WindowId> = HashSet::new();
        let mut opens: u32 = 0;
        let mut closes: u32 = 0;
        let mut focus_switches: u32 = 0;
        let mut moves: u32 = 0;
        let mut resizes: u32 = 0;
        let mut first_ts = u64::MAX;
        let mut last_ts: u64 = 0;

        for event in &all_events {
            window_ids.insert(event.window_id);
            if event.timestamp_us < first_ts {
                first_ts = event.timestamp_us;
            }
            if event.timestamp_us > last_ts {
                last_ts = event.timestamp_us;
            }
            match &event.kind {
                WindowEventKind::Opened => opens += 1,
                WindowEventKind::Closed => closes += 1,
                WindowEventKind::Focused => focus_switches += 1,
                WindowEventKind::Moved { .. } => moves += 1,
                WindowEventKind::Resized { .. } => resizes += 1,
                _ => {}
            }
        }

        // Compute per-window stats for all seen windows
        let mut total_runtime: u64 = 0;
        let mut closed_windows: u32 = 0;
        let mut most_focused: Option<(WindowId, u64)> = None;
        let mut longest: Option<(WindowId, u64)> = None;
        let mut shortest: Option<(WindowId, u64)> = None;

        for &wid in &window_ids {
            let ws = self.window_stats(wid);
            if let Some(rt) = ws.runtime_us {
                total_runtime += rt;
                closed_windows += 1;
                match longest {
                    None => longest = Some((wid, rt)),
                    Some((_, prev)) if rt > prev => longest = Some((wid, rt)),
                    _ => {}
                }
                match shortest {
                    None => shortest = Some((wid, rt)),
                    Some((_, prev)) if rt < prev => shortest = Some((wid, rt)),
                    _ => {}
                }
            }
            if ws.focus_time_us > 0 {
                match most_focused {
                    None => most_focused = Some((wid, ws.focus_time_us)),
                    Some((_, prev)) if ws.focus_time_us > prev => {
                        most_focused = Some((wid, ws.focus_time_us));
                    }
                    _ => {}
                }
            }
        }

        let avg_runtime = if closed_windows > 0 {
            total_runtime / closed_windows as u64
        } else {
            0
        };

        // Find unique apps and most active app
        let mut app_ids: HashSet<String> = HashSet::new();
        // We need to look up app_ids from AppHistory tracked apps
        let mut most_active_app: Option<(String, u64)> = None;

        // Collect app_ids from app_history's most_frequent (use a large N)
        let app_infos = self
            .app_history
            .most_frequent(self.app_history.tracked_count());
        for info in &app_infos {
            app_ids.insert(info.app_id.clone());
            if let Some(app_st) = self.app_stats(&info.app_id) {
                match &most_active_app {
                    None => most_active_app = Some((info.app_id.clone(), app_st.total_runtime_us)),
                    Some((_, prev_rt)) if app_st.total_runtime_us > *prev_rt => {
                        most_active_app = Some((info.app_id.clone(), app_st.total_runtime_us));
                    }
                    _ => {}
                }
            }
        }

        let distinct_windows = window_ids.len() as u32;
        let events_per_window = if distinct_windows > 0 {
            total_events as f64 / distinct_windows as f64
        } else {
            0.0
        };

        SystemStats {
            total_windows_opened: opens,
            total_windows_closed: closes,
            currently_open: opens.saturating_sub(closes),
            total_events,
            total_runtime_us: total_runtime,
            avg_window_runtime_us: avg_runtime,
            total_focus_switches: focus_switches,
            total_moves: moves,
            total_resizes: resizes,
            unique_apps: app_ids.len() as u32,
            most_focused_window: most_focused,
            most_active_app,
            longest_session: longest,
            shortest_session: shortest,
            timestamp_range: Some((first_ts, last_ts)),
            events_per_window_avg: events_per_window,
        }
    }

    /// Return stats for every distinct window seen in history.
    #[must_use]
    pub fn all_window_stats(&self) -> Vec<WindowStats> {
        let all_events = self.window_history.recent(self.window_history.len());
        let mut seen: HashSet<WindowId> = HashSet::new();
        let mut ids: Vec<WindowId> = Vec::new();
        for event in &all_events {
            if seen.insert(event.window_id) {
                ids.push(event.window_id);
            }
        }
        ids.iter().map(|&id| self.window_stats(id)).collect()
    }

    /// Top N windows by focus time (descending).
    #[must_use]
    pub fn top_by_focus_time(&self, n: usize) -> Vec<WindowStats> {
        let mut all = self.all_window_stats();
        all.sort_by(|a, b| b.focus_time_us.cmp(&a.focus_time_us));
        all.truncate(n);
        all
    }

    /// Top N closed windows by runtime (descending).
    #[must_use]
    pub fn top_by_runtime(&self, n: usize) -> Vec<WindowStats> {
        let mut all: Vec<WindowStats> = self
            .all_window_stats()
            .into_iter()
            .filter(|s| s.runtime_us.is_some())
            .collect();
        all.sort_by(|a, b| b.runtime_us.cmp(&a.runtime_us));
        all.truncate(n);
        all
    }

    /// Top N apps by total runtime (descending).
    #[must_use]
    pub fn top_apps_by_runtime(&self, n: usize) -> Vec<AppStats> {
        let app_infos = self
            .app_history
            .most_frequent(self.app_history.tracked_count());
        let mut all: Vec<AppStats> = app_infos
            .iter()
            .filter_map(|info| self.app_stats(&info.app_id))
            .collect();
        all.sort_by(|a, b| b.total_runtime_us.cmp(&a.total_runtime_us));
        all.truncate(n);
        all
    }

    /// Find windows with zero focus time and runtime exceeding the threshold.
    #[must_use]
    pub fn idle_windows(&self, threshold_us: u64) -> Vec<WindowId> {
        self.all_window_stats()
            .into_iter()
            .filter(|s| s.focus_time_us == 0 && s.runtime_us.is_some_and(|rt| rt > threshold_us))
            .map(|s| s.window_id)
            .collect()
    }
}

impl fmt::Display for StatsCollector<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StatsCollector(events={}, apps={})",
            self.window_history.len(),
            self.app_history.tracked_count(),
        )
    }
}
