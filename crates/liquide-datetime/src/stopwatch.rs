/// A stopwatch with lap timing support.
///
/// Uses microsecond timestamps provided by the caller (or an external clock).
/// The stopwatch itself does not query the system clock — instead, callers
/// pass the current time in microseconds to `start()`, `stop()`, etc.
#[derive(Debug, Clone)]
pub struct Stopwatch {
    /// Timestamp (microseconds) when the stopwatch was most recently started/resumed.
    pub start_us: u64,
    /// Accumulated elapsed microseconds from previous start/stop segments.
    accumulated_us: u64,
    /// Lap split times in microseconds (cumulative elapsed at each lap).
    pub laps: Vec<u64>,
    /// Whether the stopwatch is currently running.
    pub running: bool,
}

impl Stopwatch {
    /// Create a new, stopped stopwatch.
    pub fn new() -> Self {
        Self {
            start_us: 0,
            accumulated_us: 0,
            laps: Vec::new(),
            running: false,
        }
    }

    /// Start or resume the stopwatch at the given timestamp (microseconds).
    /// Does nothing if already running.
    pub fn start(&mut self, now_us: u64) {
        if !self.running {
            self.start_us = now_us;
            self.running = true;
        }
    }

    /// Stop the stopwatch at the given timestamp. Returns elapsed microseconds
    /// since the last `start()` call.
    /// Does nothing if already stopped.
    pub fn stop(&mut self, now_us: u64) -> u64 {
        if self.running {
            let segment = now_us.saturating_sub(self.start_us);
            self.accumulated_us += segment;
            self.running = false;
            segment
        } else {
            0
        }
    }

    /// Record a lap at the given timestamp. Returns the cumulative elapsed
    /// time in microseconds. Does nothing if the stopwatch is stopped.
    pub fn lap(&mut self, now_us: u64) -> u64 {
        if self.running {
            let elapsed = self.elapsed(now_us);
            self.laps.push(elapsed);
            elapsed
        } else {
            self.elapsed(now_us)
        }
    }

    /// Reset the stopwatch to its initial state.
    pub fn reset(&mut self) {
        self.start_us = 0;
        self.accumulated_us = 0;
        self.laps.clear();
        self.running = false;
    }

    /// Current elapsed time in microseconds (including accumulated segments).
    pub fn elapsed(&self, now_us: u64) -> u64 {
        if self.running {
            self.accumulated_us + now_us.saturating_sub(self.start_us)
        } else {
            self.accumulated_us
        }
    }

    /// Elapsed time formatted as "HH:MM:SS.mmm".
    pub fn elapsed_display(&self, now_us: u64) -> String {
        let total_ms = self.elapsed(now_us) / 1000;
        let ms = total_ms % 1000;
        let total_secs = total_ms / 1000;
        let secs = total_secs % 60;
        let total_mins = total_secs / 60;
        let mins = total_mins % 60;
        let hours = total_mins / 60;
        format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, ms)
    }

    /// Number of recorded laps.
    pub fn lap_count(&self) -> usize {
        self.laps.len()
    }

    /// Get individual lap durations (delta between consecutive laps).
    /// The first lap's duration is measured from the start.
    pub fn lap_splits(&self) -> Vec<u64> {
        let mut splits = Vec::with_capacity(self.laps.len());
        let mut prev = 0u64;
        for &lap in &self.laps {
            splits.push(lap.saturating_sub(prev));
            prev = lap;
        }
        splits
    }
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}
