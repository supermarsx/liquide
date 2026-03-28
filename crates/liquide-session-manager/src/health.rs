#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

pub struct HealthCheck {
    checks: Vec<Box<dyn Fn() -> HealthStatus + Send>>,
}

impl HealthCheck {
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    pub fn add_check(&mut self, check: Box<dyn Fn() -> HealthStatus + Send>) {
        self.checks.push(check);
    }

    pub fn run_all(&self) -> HealthStatus {
        let mut worst = HealthStatus::Healthy;
        for check in &self.checks {
            let result = check();
            worst = match (worst, result) {
                (_, HealthStatus::Unhealthy) | (HealthStatus::Unhealthy, _) => HealthStatus::Unhealthy,
                (_, HealthStatus::Degraded) | (HealthStatus::Degraded, _) => HealthStatus::Degraded,
                (_, HealthStatus::Unknown) | (HealthStatus::Unknown, _) => HealthStatus::Unknown,
                _ => HealthStatus::Healthy,
            };
        }
        worst
    }
}

impl Default for HealthCheck {
    fn default() -> Self { Self::new() }
}

/// Check if a process is still alive by PID
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
    #[cfg(unix)]
    {
        // kill -0 checks existence without sending signal
        use std::process::Command;
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "windows", unix)))]
    { false }
}
