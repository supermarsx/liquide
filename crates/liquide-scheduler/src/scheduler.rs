use std::collections::HashMap;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::task::{ScheduledTask, TaskResult};

/// Truncate a string to at most `max_bytes` bytes on a valid UTF-8 boundary.
fn truncate_preview(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let mut end = max_bytes;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s[..end].to_string()
    }
}

const PREVIEW_MAX: usize = 4096;

/// Central scheduler that owns tasks, tracks history, and drives execution.
pub struct Scheduler {
    tasks: HashMap<u32, ScheduledTask>,
    history: HashMap<u32, Vec<TaskResult>>,
    next_id: u32,
}

impl Scheduler {
    /// Create a new, empty scheduler.
    pub fn new() -> Self {
        Scheduler {
            tasks: HashMap::new(),
            history: HashMap::new(),
            next_id: 1,
        }
    }

    /// Add a task to the scheduler. Returns the assigned task ID.
    ///
    /// The task's `id` field is overwritten with the auto-assigned ID.
    pub fn add_task(&mut self, mut task: ScheduledTask) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        task.id = id;
        self.tasks.insert(id, task);
        id
    }

    /// Remove a task by ID. Returns `true` if it existed.
    pub fn remove_task(&mut self, id: u32) -> bool {
        self.tasks.remove(&id).is_some()
    }

    /// Enable a task. Returns `true` if the task exists.
    pub fn enable_task(&mut self, id: u32) -> bool {
        if let Some(t) = self.tasks.get_mut(&id) {
            t.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable a task. Returns `true` if the task exists.
    pub fn disable_task(&mut self, id: u32) -> bool {
        if let Some(t) = self.tasks.get_mut(&id) {
            t.enabled = false;
            true
        } else {
            false
        }
    }

    /// Get a reference to a task by ID.
    pub fn get_task(&self, id: u32) -> Option<&ScheduledTask> {
        self.tasks.get(&id)
    }

    /// Get a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, id: u32) -> Option<&mut ScheduledTask> {
        self.tasks.get_mut(&id)
    }

    /// Return all task IDs.
    pub fn task_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.tasks.keys().copied().collect();
        ids.sort();
        ids
    }

    /// Return the number of tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Tick the scheduler: check all tasks and return the IDs of tasks that
    /// are due at or before `now`. Does NOT execute them — call `run_task`
    /// separately. Updates `next_run` for due tasks to their next occurrence
    /// after `now`.
    pub fn tick(&mut self, now: u64) -> Vec<u32> {
        let mut due = Vec::new();
        for (id, task) in &mut self.tasks {
            if task.is_due(now) {
                due.push(*id);
                // Advance next_run past `now` so the same tick doesn't re-fire.
                task.recompute_next_run(now + 1);
            }
        }
        due.sort();
        due
    }

    /// Return references to all enabled tasks whose `next_run <= now`.
    pub fn pending_tasks(&self, now: u64) -> Vec<&ScheduledTask> {
        let mut out: Vec<&ScheduledTask> = self
            .tasks
            .values()
            .filter(|t| t.is_due(now))
            .collect();
        out.sort_by_key(|t| t.id);
        out
    }

    /// Execute a task synchronously via `std::process::Command`.
    ///
    /// On Unix the command is run through `sh -c`; on Windows through `cmd /C`.
    /// The task's `last_run`, `run_count`, and `last_result` are updated.
    /// The result is also appended to the history.
    pub fn run_task(&mut self, id: u32) -> Option<TaskResult> {
        let task = self.tasks.get(&id)?;
        let command = task.command.clone();
        let working_dir = task.working_dir.clone();

        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let result = Self::execute_command(&command, working_dir.as_deref());

        let finished_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let task_result = match result {
            Ok(output) => TaskResult {
                exit_code: output.status.code().unwrap_or(-1),
                stdout_preview: truncate_preview(
                    &String::from_utf8_lossy(&output.stdout),
                    PREVIEW_MAX,
                ),
                stderr_preview: truncate_preview(
                    &String::from_utf8_lossy(&output.stderr),
                    PREVIEW_MAX,
                ),
                duration_ms: (finished_at - started_at) * 1000,
                started_at,
                finished_at,
            },
            Err(e) => TaskResult {
                exit_code: -1,
                stdout_preview: String::new(),
                stderr_preview: truncate_preview(&e.to_string(), PREVIEW_MAX),
                duration_ms: (finished_at - started_at) * 1000,
                started_at,
                finished_at,
            },
        };

        // Update task state
        let task = self.tasks.get_mut(&id).unwrap();
        task.last_run = Some(finished_at);
        task.run_count += 1;
        task.last_result = Some(task_result.clone());

        // Append to history
        self.history.entry(id).or_default().push(task_result.clone());

        Some(task_result)
    }

    /// Retrieve execution history for a task.
    pub fn history(&self, task_id: u32) -> Vec<TaskResult> {
        self.history.get(&task_id).cloned().unwrap_or_default()
    }

    /// Execute a shell command, returning the raw output.
    fn execute_command(
        command: &str,
        working_dir: Option<&str>,
    ) -> std::io::Result<std::process::Output> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        cmd.output()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
