use std::fmt;

use crate::task::{Schedule, ScheduledTask, Weekday};

/// Errors from platform bridge operations.
#[derive(Debug)]
pub enum PlatformError {
    /// The operation is not supported on this platform.
    Unsupported,
    /// An I/O error occurred running a system command.
    Io(std::io::Error),
    /// The system command returned a non-zero exit code.
    CommandFailed { stderr: String },
    /// Failed to parse system output.
    ParseFailed(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Unsupported => write!(f, "operation not supported on this platform"),
            PlatformError::Io(e) => write!(f, "I/O error: {e}"),
            PlatformError::CommandFailed { stderr } => {
                write!(f, "command failed: {stderr}")
            }
            PlatformError::ParseFailed(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<std::io::Error> for PlatformError {
    fn from(e: std::io::Error) -> Self {
        PlatformError::Io(e)
    }
}

/// Platform bridge for registering/querying tasks with the OS scheduler.
pub struct PlatformBridge;

impl PlatformBridge {
    // -----------------------------------------------------------------------
    // Linux: crontab
    // -----------------------------------------------------------------------

    /// Read the current user's crontab and return its lines.
    #[cfg(target_os = "linux")]
    pub fn crontab_list() -> Result<Vec<String>, PlatformError> {
        let output = std::process::Command::new("crontab")
            .arg("-l")
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // "no crontab for ..." is not a real error
            if stderr.contains("no crontab for") {
                return Ok(Vec::new());
            }
            return Err(PlatformError::CommandFailed { stderr });
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text.lines().map(|l| l.to_string()).collect())
    }

    /// Parse crontab lines into tasks. Lines starting with `#` or blank are
    /// skipped. Each valid line is `min hour dom mon dow command`.
    #[cfg(target_os = "linux")]
    pub fn parse_crontab_lines(lines: &[String]) -> Vec<ScheduledTask> {
        let mut tasks = Vec::new();
        let mut id_counter = 1u32;
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.splitn(6, char::is_whitespace).collect();
            if parts.len() < 6 {
                continue;
            }
            let cron_str = format!("{} {} {} {} {}", parts[0], parts[1], parts[2], parts[3], parts[4]);
            let command = parts[5].to_string();
            if let Ok(expr) = CronExpr::parse(&cron_str) {
                let mut task = ScheduledTask::new(
                    id_counter,
                    format!("crontab-{id_counter}"),
                    command,
                    Schedule::Cron(expr),
                    0,
                );
                task.enabled = true;
                tasks.push(task);
                id_counter += 1;
            }
        }
        tasks
    }

    /// Write tasks as crontab entries (replaces the user's crontab).
    #[cfg(target_os = "linux")]
    pub fn crontab_write(tasks: &[ScheduledTask]) -> Result<(), PlatformError> {
        let mut content = String::from("# Managed by liquide-scheduler\n");
        for task in tasks {
            if !task.enabled {
                continue;
            }
            if let Some(cron_line) = Self::task_to_crontab_line(task) {
                content.push_str(&cron_line);
                content.push('\n');
            }
        }
        let mut child = std::process::Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(content.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(PlatformError::CommandFailed {
                stderr: "crontab write failed".to_string(),
            });
        }
        Ok(())
    }

    /// Convert a task's schedule to a crontab line. Returns `None` for
    /// schedules that don't map cleanly to cron (e.g., `Once`, short intervals).
    #[cfg(target_os = "linux")]
    fn task_to_crontab_line(task: &ScheduledTask) -> Option<String> {
        let sched = match &task.schedule {
            Schedule::Cron(expr) => {
                format!(
                    "{} {} {} {} {}",
                    format_cron_field(&expr.minute),
                    format_cron_field(&expr.hour),
                    format_cron_field(&expr.day_of_month),
                    format_cron_field(&expr.month),
                    format_cron_field(&expr.day_of_week),
                )
            }
            Schedule::Daily { hour, minute } => {
                format!("{minute} {hour} * * *")
            }
            Schedule::Weekly { day, hour, minute } => {
                format!("{minute} {hour} * * {}", *day as u8)
            }
            Schedule::Monthly { day, hour, minute } => {
                format!("{minute} {hour} {day} * *")
            }
            Schedule::Interval { seconds } => {
                if *seconds >= 60 && *seconds % 60 == 0 {
                    let mins = seconds / 60;
                    if mins <= 59 {
                        format!("*/{mins} * * * *")
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            Schedule::Once(_) => return None,
        };
        Some(format!("{} {}", sched, task.command))
    }

    /// Schedule a task via `systemd-run --on-calendar`.
    #[cfg(target_os = "linux")]
    pub fn systemd_schedule(task: &ScheduledTask) -> Result<(), PlatformError> {
        let calendar = match &task.schedule {
            Schedule::Daily { hour, minute } => format!("*-*-* {hour:02}:{minute:02}:00"),
            Schedule::Weekly { day, hour, minute } => {
                let day_name = match day {
                    Weekday::Sunday => "Sun",
                    Weekday::Monday => "Mon",
                    Weekday::Tuesday => "Tue",
                    Weekday::Wednesday => "Wed",
                    Weekday::Thursday => "Thu",
                    Weekday::Friday => "Fri",
                    Weekday::Saturday => "Sat",
                };
                format!("{day_name} *-*-* {hour:02}:{minute:02}:00")
            }
            Schedule::Monthly { day, hour, minute } => {
                format!("*-*-{day:02} {hour:02}:{minute:02}:00")
            }
            _ => {
                return Err(PlatformError::Unsupported);
            }
        };

        let unit_name = format!("liquide-task-{}", task.id);
        let output = std::process::Command::new("systemd-run")
            .args([
                "--user",
                "--on-calendar",
                &calendar,
                "--unit",
                &unit_name,
                "--",
            ])
            .arg(&task.command)
            .output()?;

        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Windows: PowerShell ScheduledTask
    // -----------------------------------------------------------------------

    /// List scheduled tasks via PowerShell `Get-ScheduledTask`.
    #[cfg(target_os = "windows")]
    pub fn windows_list_tasks() -> Result<Vec<String>, PlatformError> {
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-ScheduledTask | Select-Object -Property TaskName,State | Format-Table -HideTableHeaders",
            ])
            .output()?;
        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Register a scheduled task via PowerShell `Register-ScheduledTask`.
    #[cfg(target_os = "windows")]
    pub fn windows_register_task(task: &ScheduledTask) -> Result<(), PlatformError> {
        let trigger = match &task.schedule {
            Schedule::Daily { hour, minute } => {
                format!(
                    "New-ScheduledTaskTrigger -Daily -At '{hour:02}:{minute:02}'",
                )
            }
            Schedule::Weekly { day, hour, minute } => {
                let day_name = match day {
                    Weekday::Sunday => "Sunday",
                    Weekday::Monday => "Monday",
                    Weekday::Tuesday => "Tuesday",
                    Weekday::Wednesday => "Wednesday",
                    Weekday::Thursday => "Thursday",
                    Weekday::Friday => "Friday",
                    Weekday::Saturday => "Saturday",
                };
                format!(
                    "New-ScheduledTaskTrigger -Weekly -DaysOfWeek {day_name} -At '{hour:02}:{minute:02}'",
                )
            }
            Schedule::Once(ts) => {
                // Convert timestamp to a PowerShell datetime string
                format!(
                    "New-ScheduledTaskTrigger -Once -At (Get-Date -Date '1970-01-01').AddSeconds({ts})",
                )
            }
            _ => return Err(PlatformError::Unsupported),
        };

        let action = format!(
            "New-ScheduledTaskAction -Execute 'cmd.exe' -Argument '/C {}'",
            task.command.replace('\'', "''"),
        );

        let run_level = if task.run_as_admin {
            "-RunLevel Highest"
        } else {
            ""
        };

        let task_name = format!("LiquiDE-{}", task.id);
        let ps_command = format!(
            "$trigger = {trigger}; $action = {action}; Register-ScheduledTask -TaskName '{task_name}' -Trigger $trigger -Action $action {run_level} -Force",
        );

        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_command])
            .output()?;

        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }

    /// Unregister a scheduled task via PowerShell.
    #[cfg(target_os = "windows")]
    pub fn windows_unregister_task(task_id: u32) -> Result<(), PlatformError> {
        let task_name = format!("LiquiDE-{task_id}");
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("Unregister-ScheduledTask -TaskName '{task_name}' -Confirm:$false"),
            ])
            .output()?;
        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // macOS: launchctl + plist
    // -----------------------------------------------------------------------

    /// Generate a launchd plist XML string for a task.
    #[cfg(target_os = "macos")]
    pub fn generate_plist(task: &ScheduledTask) -> String {
        let label = format!("com.liquide.task-{}", task.id);
        let mut plist = String::new();
        plist.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        plist.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
        plist.push_str("<plist version=\"1.0\">\n<dict>\n");
        plist.push_str(&format!("  <key>Label</key>\n  <string>{label}</string>\n"));

        // ProgramArguments
        plist.push_str("  <key>ProgramArguments</key>\n  <array>\n");
        plist.push_str("    <string>/bin/sh</string>\n");
        plist.push_str("    <string>-c</string>\n");
        plist.push_str(&format!(
            "    <string>{}</string>\n",
            xml_escape(&task.command)
        ));
        plist.push_str("  </array>\n");

        if let Some(ref dir) = task.working_dir {
            plist.push_str(&format!(
                "  <key>WorkingDirectory</key>\n  <string>{}</string>\n",
                xml_escape(dir)
            ));
        }

        // StartCalendarInterval
        match &task.schedule {
            Schedule::Daily { hour, minute } => {
                plist.push_str("  <key>StartCalendarInterval</key>\n  <dict>\n");
                plist.push_str(&format!("    <key>Hour</key>\n    <integer>{hour}</integer>\n"));
                plist.push_str(&format!(
                    "    <key>Minute</key>\n    <integer>{minute}</integer>\n"
                ));
                plist.push_str("  </dict>\n");
            }
            Schedule::Weekly { day, hour, minute } => {
                plist.push_str("  <key>StartCalendarInterval</key>\n  <dict>\n");
                plist.push_str(&format!(
                    "    <key>Weekday</key>\n    <integer>{}</integer>\n",
                    *day as u8
                ));
                plist.push_str(&format!("    <key>Hour</key>\n    <integer>{hour}</integer>\n"));
                plist.push_str(&format!(
                    "    <key>Minute</key>\n    <integer>{minute}</integer>\n"
                ));
                plist.push_str("  </dict>\n");
            }
            Schedule::Monthly { day, hour, minute } => {
                plist.push_str("  <key>StartCalendarInterval</key>\n  <dict>\n");
                plist.push_str(&format!("    <key>Day</key>\n    <integer>{day}</integer>\n"));
                plist.push_str(&format!("    <key>Hour</key>\n    <integer>{hour}</integer>\n"));
                plist.push_str(&format!(
                    "    <key>Minute</key>\n    <integer>{minute}</integer>\n"
                ));
                plist.push_str("  </dict>\n");
            }
            Schedule::Interval { seconds } => {
                plist.push_str(&format!(
                    "  <key>StartInterval</key>\n  <integer>{seconds}</integer>\n"
                ));
            }
            _ => {}
        }

        plist.push_str("</dict>\n</plist>\n");
        plist
    }

    /// Install a plist into ~/Library/LaunchAgents and load it via launchctl.
    #[cfg(target_os = "macos")]
    pub fn launchctl_install(task: &ScheduledTask) -> Result<(), PlatformError> {
        let label = format!("com.liquide.task-{}", task.id);
        let plist_content = Self::generate_plist(task);

        let home = std::env::var("HOME").map_err(|_| {
            PlatformError::ParseFailed("HOME not set".to_string())
        })?;
        let plist_path = format!("{home}/Library/LaunchAgents/{label}.plist");

        std::fs::write(&plist_path, &plist_content)?;

        let output = std::process::Command::new("launchctl")
            .args(["load", &plist_path])
            .output()?;
        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }

    /// Unload and remove a launchd plist.
    #[cfg(target_os = "macos")]
    pub fn launchctl_uninstall(task_id: u32) -> Result<(), PlatformError> {
        let label = format!("com.liquide.task-{task_id}");
        let home = std::env::var("HOME").map_err(|_| {
            PlatformError::ParseFailed("HOME not set".to_string())
        })?;
        let plist_path = format!("{home}/Library/LaunchAgents/{label}.plist");

        // Unload (ignore errors — might not be loaded)
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &plist_path])
            .output();

        // Remove file
        let _ = std::fs::remove_file(&plist_path);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Cross-platform: format helpers
    // -----------------------------------------------------------------------

    /// Format a `Schedule` as a human-readable summary.
    pub fn describe_schedule(schedule: &Schedule) -> String {
        match schedule {
            Schedule::Once(ts) => format!("once at timestamp {ts}"),
            Schedule::Interval { seconds } => {
                if *seconds < 60 {
                    format!("every {seconds} seconds")
                } else if *seconds < 3600 {
                    format!("every {} minutes", seconds / 60)
                } else {
                    format!("every {} hours", seconds / 3600)
                }
            }
            Schedule::Daily { hour, minute } => {
                format!("daily at {hour:02}:{minute:02}")
            }
            Schedule::Weekly { day, hour, minute } => {
                format!("every {day:?} at {hour:02}:{minute:02}")
            }
            Schedule::Monthly { day, hour, minute } => {
                format!("monthly on day {day} at {hour:02}:{minute:02}")
            }
            Schedule::Cron(expr) => {
                format!(
                    "cron: {} {} {} {} {}",
                    format_cron_field(&expr.minute),
                    format_cron_field(&expr.hour),
                    format_cron_field(&expr.day_of_month),
                    format_cron_field(&expr.month),
                    format_cron_field(&expr.day_of_week),
                )
            }
        }
    }
}

/// Format a CronField back to its cron string representation.
fn format_cron_field(field: &crate::cron::CronField) -> String {
    use crate::cron::CronField;
    match field {
        CronField::Any => "*".to_string(),
        CronField::Value(v) => v.to_string(),
        CronField::Range(lo, hi) => format!("{lo}-{hi}"),
        CronField::Step(s) => format!("*/{s}"),
        CronField::List(vals) => {
            vals.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        }
    }
}

/// Escape special XML characters.
#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
