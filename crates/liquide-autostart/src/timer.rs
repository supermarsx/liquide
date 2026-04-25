use std::collections::HashMap;
use std::time::Instant;

/// Tracks timing information for a single autostart application.
#[derive(Debug, Clone)]
pub struct AppTiming {
    /// When the app process was spawned.
    pub started_at: Option<Instant>,
    /// When the app signaled it was fully ready.
    pub ready_at: Option<Instant>,
}

impl AppTiming {
    /// Time from session start to app process spawn, in milliseconds.
    pub fn start_latency_ms(&self, session_start: Instant) -> Option<u64> {
        self.started_at
            .map(|t| t.duration_since(session_start).as_millis() as u64)
    }

    /// Time from app spawn to app ready, in milliseconds.
    pub fn startup_duration_ms(&self) -> Option<u64> {
        match (self.started_at, self.ready_at) {
            (Some(s), Some(r)) => Some(r.duration_since(s).as_millis() as u64),
            _ => None,
        }
    }

    /// Time from session start to app ready, in milliseconds.
    pub fn total_ms(&self, session_start: Instant) -> Option<u64> {
        self.ready_at
            .map(|t| t.duration_since(session_start).as_millis() as u64)
    }
}

/// Per-app timing entry in the startup report.
#[derive(Debug, Clone)]
pub struct AppReportEntry {
    /// Application id.
    pub id: String,
    /// Milliseconds from session start to process spawn.
    pub start_latency_ms: Option<u64>,
    /// Milliseconds from process spawn to ready.
    pub startup_duration_ms: Option<u64>,
    /// Milliseconds from session start to ready.
    pub total_ms: Option<u64>,
    /// Whether the app has signaled ready.
    pub is_ready: bool,
}

/// Aggregated timing report for the entire startup sequence.
#[derive(Debug, Clone)]
pub struct StartupReport {
    /// Per-app timing breakdown.
    pub apps: Vec<AppReportEntry>,
    /// Total elapsed time from session start to the last app becoming ready.
    pub total_time_ms: u64,
    /// Number of apps that have started.
    pub started_count: usize,
    /// Number of apps that have signaled ready.
    pub ready_count: usize,
}

/// Tracks the startup timing of the desktop session and its autostart applications.
pub struct StartupTimer {
    session_start: Option<Instant>,
    timings: HashMap<String, AppTiming>,
}

impl StartupTimer {
    /// Create a new timer (session not yet started).
    pub fn new() -> Self {
        Self {
            session_start: None,
            timings: HashMap::new(),
        }
    }

    /// Mark the beginning of the session.
    pub fn begin(&mut self) {
        self.session_start = Some(Instant::now());
        self.timings.clear();
    }

    /// Mark that the session has begun at a specific instant (for testing).
    pub fn begin_at(&mut self, instant: Instant) {
        self.session_start = Some(instant);
        self.timings.clear();
    }

    /// Mark an application as having been spawned/started.
    pub fn app_started(&mut self, id: &str) {
        let entry = self.timings.entry(id.to_string()).or_insert(AppTiming {
            started_at: None,
            ready_at: None,
        });
        entry.started_at = Some(Instant::now());
    }

    /// Mark an application as having been spawned at a specific instant (for testing).
    pub fn app_started_at(&mut self, id: &str, instant: Instant) {
        let entry = self.timings.entry(id.to_string()).or_insert(AppTiming {
            started_at: None,
            ready_at: None,
        });
        entry.started_at = Some(instant);
    }

    /// Mark an application as fully ready (e.g., it opened its window).
    pub fn app_ready(&mut self, id: &str) {
        let entry = self.timings.entry(id.to_string()).or_insert(AppTiming {
            started_at: None,
            ready_at: None,
        });
        entry.ready_at = Some(Instant::now());
    }

    /// Mark an application as ready at a specific instant (for testing).
    pub fn app_ready_at(&mut self, id: &str, instant: Instant) {
        let entry = self.timings.entry(id.to_string()).or_insert(AppTiming {
            started_at: None,
            ready_at: None,
        });
        entry.ready_at = Some(instant);
    }

    /// Whether the session has been started.
    pub fn is_started(&self) -> bool {
        self.session_start.is_some()
    }

    /// Number of tracked applications.
    pub fn tracked_count(&self) -> usize {
        self.timings.len()
    }

    /// Get the timing data for a specific app.
    pub fn get_app_timing(&self, id: &str) -> Option<&AppTiming> {
        self.timings.get(id)
    }

    /// Total elapsed time from session start to the last app that signaled ready.
    /// Returns 0 if no apps are ready or the session hasn't started.
    pub fn total_time_ms(&self) -> u64 {
        let session_start = match self.session_start {
            Some(s) => s,
            None => return 0,
        };

        self.timings
            .values()
            .filter_map(|t| t.ready_at)
            .map(|r| r.duration_since(session_start).as_millis() as u64)
            .max()
            .unwrap_or(0)
    }

    /// Generate a full startup report.
    pub fn report(&self) -> StartupReport {
        let session_start = match self.session_start {
            Some(s) => s,
            None => {
                return StartupReport {
                    apps: Vec::new(),
                    total_time_ms: 0,
                    started_count: 0,
                    ready_count: 0,
                };
            }
        };

        let mut apps: Vec<AppReportEntry> = self
            .timings
            .iter()
            .map(|(id, timing)| AppReportEntry {
                id: id.clone(),
                start_latency_ms: timing.start_latency_ms(session_start),
                startup_duration_ms: timing.startup_duration_ms(),
                total_ms: timing.total_ms(session_start),
                is_ready: timing.ready_at.is_some(),
            })
            .collect();

        // Sort by total_ms (ready apps first), then by id.
        apps.sort_by(|a, b| {
            let a_time = a.total_ms.unwrap_or(u64::MAX);
            let b_time = b.total_ms.unwrap_or(u64::MAX);
            a_time.cmp(&b_time).then_with(|| a.id.cmp(&b.id))
        });

        let started_count = self
            .timings
            .values()
            .filter(|t| t.started_at.is_some())
            .count();
        let ready_count = self
            .timings
            .values()
            .filter(|t| t.ready_at.is_some())
            .count();

        StartupReport {
            total_time_ms: self.total_time_ms(),
            apps,
            started_count,
            ready_count,
        }
    }
}

impl Default for StartupTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn new_timer_not_started() {
        let timer = StartupTimer::new();
        assert!(!timer.is_started());
        assert_eq!(timer.total_time_ms(), 0);
        assert_eq!(timer.tracked_count(), 0);
    }

    #[test]
    fn begin_marks_started() {
        let mut timer = StartupTimer::new();
        timer.begin();
        assert!(timer.is_started());
    }

    #[test]
    fn app_started_and_ready() {
        let mut timer = StartupTimer::new();
        let t0 = Instant::now();
        timer.begin_at(t0);
        timer.app_started_at("firefox", t0 + Duration::from_millis(100));
        timer.app_ready_at("firefox", t0 + Duration::from_millis(600));

        let timing = timer.get_app_timing("firefox").unwrap();
        assert_eq!(timing.start_latency_ms(t0), Some(100));
        assert_eq!(timing.startup_duration_ms(), Some(500));
        assert_eq!(timing.total_ms(t0), Some(600));
    }

    #[test]
    fn total_time_is_max_ready() {
        let mut timer = StartupTimer::new();
        let t0 = Instant::now();
        timer.begin_at(t0);

        timer.app_started_at("a", t0 + Duration::from_millis(0));
        timer.app_ready_at("a", t0 + Duration::from_millis(200));

        timer.app_started_at("b", t0 + Duration::from_millis(50));
        timer.app_ready_at("b", t0 + Duration::from_millis(800));

        assert_eq!(timer.total_time_ms(), 800);
    }

    #[test]
    fn total_time_zero_when_no_ready() {
        let mut timer = StartupTimer::new();
        let t0 = Instant::now();
        timer.begin_at(t0);
        timer.app_started_at("a", t0 + Duration::from_millis(100));
        // Never marked ready.
        assert_eq!(timer.total_time_ms(), 0);
    }

    #[test]
    fn report_empty_session() {
        let timer = StartupTimer::new();
        let report = timer.report();
        assert!(report.apps.is_empty());
        assert_eq!(report.total_time_ms, 0);
        assert_eq!(report.started_count, 0);
        assert_eq!(report.ready_count, 0);
    }

    #[test]
    fn report_with_apps() {
        let mut timer = StartupTimer::new();
        let t0 = Instant::now();
        timer.begin_at(t0);

        timer.app_started_at("fast", t0 + Duration::from_millis(10));
        timer.app_ready_at("fast", t0 + Duration::from_millis(50));

        timer.app_started_at("slow", t0 + Duration::from_millis(100));
        timer.app_ready_at("slow", t0 + Duration::from_millis(1000));

        timer.app_started_at("pending", t0 + Duration::from_millis(200));
        // pending never signals ready

        let report = timer.report();
        assert_eq!(report.started_count, 3);
        assert_eq!(report.ready_count, 2);
        assert_eq!(report.total_time_ms, 1000);

        // Sorted by total_ms: fast (50), slow (1000), pending (MAX).
        assert_eq!(report.apps[0].id, "fast");
        assert!(report.apps[0].is_ready);
        assert_eq!(report.apps[0].total_ms, Some(50));

        assert_eq!(report.apps[1].id, "slow");
        assert!(report.apps[1].is_ready);
        assert_eq!(report.apps[1].total_ms, Some(1000));

        assert_eq!(report.apps[2].id, "pending");
        assert!(!report.apps[2].is_ready);
        assert_eq!(report.apps[2].total_ms, None);
    }

    #[test]
    fn begin_clears_previous_data() {
        let mut timer = StartupTimer::new();
        timer.begin();
        timer.app_started("old-app");
        assert_eq!(timer.tracked_count(), 1);

        timer.begin();
        assert_eq!(timer.tracked_count(), 0);
    }

    #[test]
    fn app_timing_without_start() {
        // An app can be marked ready without ever being marked started
        // (e.g., it was already running before the session).
        let mut timer = StartupTimer::new();
        let t0 = Instant::now();
        timer.begin_at(t0);
        timer.app_ready_at("pre-existing", t0 + Duration::from_millis(300));

        let timing = timer.get_app_timing("pre-existing").unwrap();
        assert!(timing.started_at.is_none());
        assert!(timing.ready_at.is_some());
        assert_eq!(timing.startup_duration_ms(), None);
        assert_eq!(timing.total_ms(t0), Some(300));
    }
}
