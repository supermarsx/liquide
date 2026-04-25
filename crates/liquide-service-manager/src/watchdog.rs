// Process watchdog: monitors service processes for unexpected termination
// and applies restart backoff logic.

use std::collections::HashMap;

use crate::service::ServiceId;

/// Events detected by the watchdog during a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchdogEvent {
    /// A service process exited with the given exit code.
    ProcessExited(ServiceId, i32),
    /// A service process crashed (terminated by signal).
    ProcessCrashed(ServiceId, i32),
    /// A heartbeat was received from a service (still alive).
    Heartbeat(ServiceId),
}

/// Internal state for a watched process.
#[derive(Debug)]
struct WatchedProcess {
    pid: u64,
    /// Whether this process is still considered alive.
    alive: bool,
    /// Number of restarts that have occurred.
    restart_count: u32,
    /// Current backoff delay in milliseconds.
    backoff_ms: u64,
    /// Time (ms) when the last restart occurred, or None if never restarted.
    last_restart_time_ms: Option<u64>,
    /// Whether a restart is pending (waiting for backoff).
    restart_pending: bool,
    /// Time (ms) when the process was marked for pending restart.
    pending_since_ms: Option<u64>,
}

/// Configuration for the watchdog.
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// Initial backoff delay in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum backoff delay in milliseconds.
    pub max_backoff_ms: u64,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: u32,
    /// After this many successful seconds, reset the backoff.
    pub reset_after_stable_ms: u64,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            initial_backoff_ms: 1_000,
            max_backoff_ms: 60_000,
            backoff_multiplier: 2,
            reset_after_stable_ms: 300_000, // 5 minutes
        }
    }
}

/// Process watchdog that monitors registered service processes.
pub struct Watchdog {
    processes: HashMap<ServiceId, WatchedProcess>,
    config: WatchdogConfig,
    events: Vec<WatchdogEvent>,
}

impl Watchdog {
    /// Create a new watchdog with default configuration.
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            config: WatchdogConfig::default(),
            events: Vec::new(),
        }
    }

    /// Create a new watchdog with custom configuration.
    pub fn with_config(config: WatchdogConfig) -> Self {
        Self {
            processes: HashMap::new(),
            config,
            events: Vec::new(),
        }
    }

    /// Register a process ID for a service to be watched.
    pub fn register_pid(&mut self, id: ServiceId, pid: u64) {
        self.processes.insert(
            id,
            WatchedProcess {
                pid,
                alive: true,
                restart_count: 0,
                backoff_ms: self.config.initial_backoff_ms,
                last_restart_time_ms: None,
                restart_pending: false,
                pending_since_ms: None,
            },
        );
    }

    /// Unregister a service from the watchdog.
    pub fn unregister(&mut self, id: &ServiceId) {
        self.processes.remove(id);
    }

    /// Report that a process has exited. Returns the generated event.
    pub fn report_exit(&mut self, id: &ServiceId, exit_code: i32) -> Option<WatchdogEvent> {
        let proc = self.processes.get_mut(id)?;
        proc.alive = false;

        let event = if exit_code == 0 {
            WatchdogEvent::ProcessExited(id.clone(), exit_code)
        } else {
            WatchdogEvent::ProcessCrashed(id.clone(), exit_code)
        };

        self.events.push(event.clone());
        Some(event)
    }

    /// Report that a process was terminated by a signal.
    pub fn report_signal(&mut self, id: &ServiceId, signal: i32) -> Option<WatchdogEvent> {
        let proc = self.processes.get_mut(id)?;
        proc.alive = false;
        let event = WatchdogEvent::ProcessCrashed(id.clone(), signal);
        self.events.push(event.clone());
        Some(event)
    }

    /// Record a heartbeat from a service, proving it is still alive.
    pub fn heartbeat(&mut self, id: &ServiceId) -> Option<WatchdogEvent> {
        let proc = self.processes.get_mut(id)?;
        if proc.alive {
            let event = WatchdogEvent::Heartbeat(id.clone());
            self.events.push(event.clone());
            Some(event)
        } else {
            None
        }
    }

    /// Run a tick of the watchdog at the given timestamp (ms). Scans all
    /// registered processes and returns events for any that have terminated.
    /// Also manages backoff timers for pending restarts.
    pub fn tick(&mut self, current_time_ms: u64) -> Vec<WatchdogEvent> {
        let mut tick_events = Vec::new();

        for (id, proc) in &mut self.processes {
            if !proc.alive && !proc.restart_pending {
                // Mark for restart with backoff
                proc.restart_pending = true;
                proc.pending_since_ms = Some(current_time_ms);
            }

            // Check if stable long enough to reset backoff
            if proc.alive {
                if let Some(restart_time) = proc.last_restart_time_ms {
                    if current_time_ms - restart_time >= self.config.reset_after_stable_ms {
                        proc.restart_count = 0;
                        proc.backoff_ms = self.config.initial_backoff_ms;
                        proc.last_restart_time_ms = None;
                    }
                }
            }

            // Emit heartbeat for alive processes
            if proc.alive {
                let event = WatchdogEvent::Heartbeat(id.clone());
                tick_events.push(event);
            }
        }

        tick_events
    }

    /// Check if a service is ready to be restarted (backoff period has elapsed).
    pub fn ready_for_restart(&self, id: &ServiceId, current_time_ms: u64) -> bool {
        if let Some(proc) = self.processes.get(id) {
            if proc.restart_pending {
                if let Some(pending_since) = proc.pending_since_ms {
                    return current_time_ms - pending_since >= proc.backoff_ms;
                }
            }
        }
        false
    }

    /// Mark a service as restarted with a new PID at the given time.
    /// Advances the backoff for the next potential restart.
    pub fn mark_restarted(&mut self, id: &ServiceId, new_pid: u64) {
        if let Some(proc) = self.processes.get_mut(id) {
            // Record restart time for stability tracking (use pending_since if available)
            proc.last_restart_time_ms = proc.pending_since_ms.or(Some(0));
            proc.pid = new_pid;
            proc.alive = true;
            proc.restart_pending = false;
            proc.pending_since_ms = None;
            proc.restart_count += 1;

            // Exponential backoff
            let new_backoff = proc.backoff_ms * self.config.backoff_multiplier as u64;
            proc.backoff_ms = new_backoff.min(self.config.max_backoff_ms);
        }
    }

    /// Get the current backoff delay for a service (in ms).
    pub fn current_backoff(&self, id: &ServiceId) -> Option<u64> {
        self.processes.get(id).map(|p| p.backoff_ms)
    }

    /// Get the restart count for a service.
    pub fn restart_count(&self, id: &ServiceId) -> Option<u32> {
        self.processes.get(id).map(|p| p.restart_count)
    }

    /// Check if a service process is currently alive.
    pub fn is_alive(&self, id: &ServiceId) -> bool {
        self.processes.get(id).map(|p| p.alive).unwrap_or(false)
    }

    /// Get the PID of a watched service.
    pub fn pid(&self, id: &ServiceId) -> Option<u64> {
        self.processes.get(id).map(|p| p.pid)
    }

    /// Number of watched processes.
    pub fn watched_count(&self) -> usize {
        self.processes.len()
    }

    /// Get all recorded events.
    pub fn events(&self) -> &[WatchdogEvent] {
        &self.events
    }

    /// Get services that have pending restarts.
    pub fn pending_restarts(&self) -> Vec<ServiceId> {
        self.processes
            .iter()
            .filter(|(_, p)| p.restart_pending)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Reset backoff for a specific service (e.g., after manual intervention).
    pub fn reset_backoff(&mut self, id: &ServiceId) {
        if let Some(proc) = self.processes.get_mut(id) {
            proc.backoff_ms = self.config.initial_backoff_ms;
            proc.restart_count = 0;
        }
    }
}

impl Default for Watchdog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> ServiceId {
        ServiceId::new(s)
    }

    #[test]
    fn register_and_check_alive() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);
        assert!(wd.is_alive(&sid("svc")));
        assert_eq!(wd.pid(&sid("svc")), Some(100));
        assert_eq!(wd.watched_count(), 1);
    }

    #[test]
    fn unregister_removes_watch() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);
        wd.unregister(&sid("svc"));
        assert!(!wd.is_alive(&sid("svc")));
        assert_eq!(wd.watched_count(), 0);
    }

    #[test]
    fn report_clean_exit() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);

        let event = wd.report_exit(&sid("svc"), 0).unwrap();
        assert_eq!(event, WatchdogEvent::ProcessExited(sid("svc"), 0));
        assert!(!wd.is_alive(&sid("svc")));
    }

    #[test]
    fn report_crash_exit() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);

        let event = wd.report_exit(&sid("svc"), 1).unwrap();
        assert_eq!(event, WatchdogEvent::ProcessCrashed(sid("svc"), 1));
        assert!(!wd.is_alive(&sid("svc")));
    }

    #[test]
    fn report_signal() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);

        let event = wd.report_signal(&sid("svc"), 11).unwrap();
        assert_eq!(event, WatchdogEvent::ProcessCrashed(sid("svc"), 11));
    }

    #[test]
    fn heartbeat_alive_process() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);

        let event = wd.heartbeat(&sid("svc")).unwrap();
        assert_eq!(event, WatchdogEvent::Heartbeat(sid("svc")));
    }

    #[test]
    fn heartbeat_dead_process_returns_none() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 1);
        assert!(wd.heartbeat(&sid("svc")).is_none());
    }

    #[test]
    fn heartbeat_unknown_returns_none() {
        let mut wd = Watchdog::new();
        assert!(wd.heartbeat(&sid("ghost")).is_none());
    }

    #[test]
    fn tick_generates_heartbeats_for_alive() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("a"), 1);
        wd.register_pid(sid("b"), 2);

        let events = wd.tick(1000);
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|e| matches!(e, WatchdogEvent::Heartbeat(_)))
        );
    }

    #[test]
    fn tick_marks_dead_for_pending_restart() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 1);

        wd.tick(5000);
        let pending = wd.pending_restarts();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0], sid("svc"));
    }

    #[test]
    fn backoff_exponential() {
        let mut wd = Watchdog::with_config(WatchdogConfig {
            initial_backoff_ms: 1000,
            max_backoff_ms: 60_000,
            backoff_multiplier: 2,
            ..Default::default()
        });

        wd.register_pid(sid("svc"), 100);
        assert_eq!(wd.current_backoff(&sid("svc")), Some(1000));

        // Simulate crash + restart cycle
        wd.report_exit(&sid("svc"), 1);
        wd.tick(0);
        wd.mark_restarted(&sid("svc"), 101);
        assert_eq!(wd.current_backoff(&sid("svc")), Some(2000)); // 1000 * 2

        wd.report_exit(&sid("svc"), 1);
        wd.tick(3000);
        wd.mark_restarted(&sid("svc"), 102);
        assert_eq!(wd.current_backoff(&sid("svc")), Some(4000)); // 2000 * 2

        wd.report_exit(&sid("svc"), 1);
        wd.tick(8000);
        wd.mark_restarted(&sid("svc"), 103);
        assert_eq!(wd.current_backoff(&sid("svc")), Some(8000)); // 4000 * 2
    }

    #[test]
    fn backoff_capped_at_max() {
        let mut wd = Watchdog::with_config(WatchdogConfig {
            initial_backoff_ms: 30_000,
            max_backoff_ms: 60_000,
            backoff_multiplier: 2,
            ..Default::default()
        });

        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 1);
        wd.tick(0);
        wd.mark_restarted(&sid("svc"), 101);
        assert_eq!(wd.current_backoff(&sid("svc")), Some(60_000)); // capped

        wd.report_exit(&sid("svc"), 1);
        wd.tick(70_000);
        wd.mark_restarted(&sid("svc"), 102);
        assert_eq!(wd.current_backoff(&sid("svc")), Some(60_000)); // still capped
    }

    #[test]
    fn ready_for_restart_respects_backoff() {
        let mut wd = Watchdog::with_config(WatchdogConfig {
            initial_backoff_ms: 5000,
            ..Default::default()
        });

        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 1);
        wd.tick(1000); // restart_pending set at time=1000

        assert!(!wd.ready_for_restart(&sid("svc"), 2000)); // 1s < 5s backoff
        assert!(!wd.ready_for_restart(&sid("svc"), 5000)); // 4s < 5s
        assert!(wd.ready_for_restart(&sid("svc"), 6000)); // 5s >= 5s
    }

    #[test]
    fn mark_restarted_resets_state() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 1);
        wd.tick(0);

        wd.mark_restarted(&sid("svc"), 200);
        assert!(wd.is_alive(&sid("svc")));
        assert_eq!(wd.pid(&sid("svc")), Some(200));
        assert_eq!(wd.restart_count(&sid("svc")), Some(1));
        assert!(wd.pending_restarts().is_empty());
    }

    #[test]
    fn reset_backoff() {
        let mut wd = Watchdog::with_config(WatchdogConfig {
            initial_backoff_ms: 1000,
            ..Default::default()
        });

        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 1);
        wd.tick(0);
        wd.mark_restarted(&sid("svc"), 101); // backoff now 2000

        wd.reset_backoff(&sid("svc"));
        assert_eq!(wd.current_backoff(&sid("svc")), Some(1000));
        assert_eq!(wd.restart_count(&sid("svc")), Some(0));
    }

    #[test]
    fn backoff_resets_after_stable_period() {
        let mut wd = Watchdog::with_config(WatchdogConfig {
            initial_backoff_ms: 1000,
            max_backoff_ms: 60_000,
            backoff_multiplier: 2,
            reset_after_stable_ms: 10_000,
        });

        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 1);
        wd.tick(0);
        wd.mark_restarted(&sid("svc"), 101); // backoff=2000, last_restart=0

        // After 10 seconds of stability, backoff should reset
        wd.tick(10_000);
        assert_eq!(wd.current_backoff(&sid("svc")), Some(1000)); // reset
        assert_eq!(wd.restart_count(&sid("svc")), Some(0));
    }

    #[test]
    fn events_accumulate() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);
        wd.report_exit(&sid("svc"), 0);
        wd.register_pid(sid("svc2"), 200);
        wd.heartbeat(&sid("svc2"));

        assert_eq!(wd.events().len(), 2);
    }

    #[test]
    fn report_exit_unknown_returns_none() {
        let mut wd = Watchdog::new();
        assert!(wd.report_exit(&sid("ghost"), 0).is_none());
    }

    #[test]
    fn report_signal_unknown_returns_none() {
        let mut wd = Watchdog::new();
        assert!(wd.report_signal(&sid("ghost"), 9).is_none());
    }

    #[test]
    fn multiple_restart_cycles_increment_count() {
        let mut wd = Watchdog::new();
        wd.register_pid(sid("svc"), 100);

        for i in 1..=5 {
            wd.report_exit(&sid("svc"), 1);
            wd.tick(i * 100_000);
            wd.mark_restarted(&sid("svc"), 100 + i);
        }

        assert_eq!(wd.restart_count(&sid("svc")), Some(5));
    }

    #[test]
    fn watchdog_config_defaults() {
        let cfg = WatchdogConfig::default();
        assert_eq!(cfg.initial_backoff_ms, 1_000);
        assert_eq!(cfg.max_backoff_ms, 60_000);
        assert_eq!(cfg.backoff_multiplier, 2);
        assert_eq!(cfg.reset_after_stable_ms, 300_000);
    }
}
