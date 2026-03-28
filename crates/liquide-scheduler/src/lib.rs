//! Scheduled task management for the LiquiDE desktop environment.
//!
//! Provides cron-like task scheduling with platform-native bridges
//! (crontab on Linux, Task Scheduler on Windows, launchctl on macOS).

mod cron;
mod platform;
mod scheduler;
mod task;

pub use cron::{CronExpr, CronField, ParseError};
pub use platform::{PlatformBridge, PlatformError};
pub use scheduler::Scheduler;
pub use task::{Schedule, ScheduledTask, TaskResult, Weekday};

#[cfg(test)]
mod tests;
