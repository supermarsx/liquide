/// A countdown timer with start, pause, resume, and reset.
///
/// Like `Stopwatch`, this does not query the system clock. Callers provide
/// elapsed time via the `tick()` method.
#[derive(Debug, Clone)]
pub struct CountdownTimer {
    /// Total countdown duration in milliseconds.
    pub duration_ms: u64,
    /// Remaining time in milliseconds.
    pub remaining_ms: u64,
    /// Whether the timer is currently running.
    pub running: bool,
}

impl CountdownTimer {
    /// Create a new countdown timer with the given duration in milliseconds.
    pub fn new(duration_ms: u64) -> Self {
        Self {
            duration_ms,
            remaining_ms: duration_ms,
            running: false,
        }
    }

    /// Create a countdown timer from hours, minutes, seconds.
    pub fn from_hms(hours: u64, minutes: u64, seconds: u64) -> Self {
        let ms = (hours * 3600 + minutes * 60 + seconds) * 1000;
        Self::new(ms)
    }

    /// Start or resume the timer.
    pub fn start(&mut self) {
        if self.remaining_ms > 0 {
            self.running = true;
        }
    }

    /// Pause the timer.
    pub fn pause(&mut self) {
        self.running = false;
    }

    /// Reset the timer to its original duration.
    pub fn reset(&mut self) {
        self.remaining_ms = self.duration_ms;
        self.running = false;
    }

    /// Advance the timer by `delta_ms` milliseconds.
    /// Returns `true` if the timer just reached zero on this tick.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.running || self.remaining_ms == 0 {
            return false;
        }

        if delta_ms >= self.remaining_ms {
            self.remaining_ms = 0;
            self.running = false;
            true
        } else {
            self.remaining_ms -= delta_ms;
            false
        }
    }

    /// Returns true if the timer has finished (remaining = 0).
    pub fn is_finished(&self) -> bool {
        self.remaining_ms == 0
    }

    /// Elapsed time in milliseconds (duration - remaining).
    pub fn elapsed_ms(&self) -> u64 {
        self.duration_ms.saturating_sub(self.remaining_ms)
    }

    /// Progress as a fraction in [0.0, 1.0].
    pub fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        self.elapsed_ms() as f64 / self.duration_ms as f64
    }

    /// Format remaining time as "HH:MM:SS".
    pub fn remaining_display(&self) -> String {
        let total_secs = self.remaining_ms / 1000;
        let secs = total_secs % 60;
        let total_mins = total_secs / 60;
        let mins = total_mins % 60;
        let hours = total_mins / 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }

    /// Format remaining time as "MM:SS" (for short timers).
    pub fn remaining_display_short(&self) -> String {
        let total_secs = self.remaining_ms / 1000;
        let secs = total_secs % 60;
        let mins = total_secs / 60;
        format!("{:02}:{:02}", mins, secs)
    }
}
