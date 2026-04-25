//! Central print coordinator: printer discovery, job submission, and job tracking.

use crate::discovery;
use crate::job::{JobStatus, PrintJob};
use crate::printer::{Printer, PrinterId};
use crate::settings::PrintSettings;

/// Central coordinator for the print system.
///
/// Manages printer discovery, job submission, tracking, and history.
pub struct PrintManager {
    /// Cached list of discovered printers.
    printers: Vec<Printer>,
    /// All jobs (active and historical).
    jobs: Vec<PrintJob>,
    /// Counter for generating unique job IDs.
    next_job_id: u64,
}

impl PrintManager {
    /// Create a new print manager with no printers or jobs.
    pub fn new() -> Self {
        Self {
            printers: Vec::new(),
            jobs: Vec::new(),
            next_job_id: 1,
        }
    }

    /// Discover available printers on the system.
    ///
    /// This replaces the cached printer list with fresh results from platform
    /// discovery. Returns a reference to the updated list.
    pub fn discover_printers(&mut self) -> &[Printer] {
        self.printers = discovery::discover_printers();
        tracing::info!("Discovered {} printers", self.printers.len());
        &self.printers
    }

    /// Returns the cached list of printers. Call [`discover_printers`] first
    /// to populate it.
    pub fn printers(&self) -> &[Printer] {
        &self.printers
    }

    /// Returns the default printer, if one was found.
    pub fn default_printer(&self) -> Option<&Printer> {
        self.printers.iter().find(|p| p.is_default)
    }

    /// Find a printer by its ID.
    pub fn printer_by_id(&self, id: PrinterId) -> Option<&Printer> {
        self.printers.iter().find(|p| p.id == id)
    }

    /// Submit a new print job.
    ///
    /// Returns the job ID. The job starts in [`JobStatus::Queued`].
    /// Returns `None` if the target printer is not found.
    pub fn submit_job(
        &mut self,
        printer_id: PrinterId,
        document_name: impl Into<String>,
        settings: PrintSettings,
        total_pages: u32,
    ) -> Option<u64> {
        // Verify the printer exists.
        if self.printer_by_id(printer_id).is_none() {
            tracing::warn!("Cannot submit job: printer {:?} not found", printer_id);
            return None;
        }

        let job_id = self.next_job_id;
        self.next_job_id += 1;

        let now = current_timestamp_us();

        let job = PrintJob {
            id: job_id,
            printer_id,
            document_name: document_name.into(),
            settings,
            status: JobStatus::Queued,
            pages_printed: 0,
            total_pages,
            created_at: now,
            started_at: None,
            completed_at: None,
        };

        tracing::info!(
            "Submitted print job {} ({} pages) to printer {:?}",
            job_id,
            total_pages,
            printer_id
        );
        self.jobs.push(job);
        Some(job_id)
    }

    /// Cancel a print job.
    ///
    /// Sets the job status to [`JobStatus::Cancelled`] and records the
    /// completion timestamp. Has no effect if the job is already in a
    /// terminal state.
    pub fn cancel_job(&mut self, job_id: u64) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            if !job.status.is_terminal() {
                tracing::info!("Cancelling print job {}", job_id);
                job.status = JobStatus::Cancelled;
                job.completed_at = Some(current_timestamp_us());
            }
        }
    }

    /// Get the status of a job by ID.
    pub fn job_status(&self, job_id: u64) -> Option<&PrintJob> {
        self.jobs.iter().find(|j| j.id == job_id)
    }

    /// Get a mutable reference to a job by ID (for updating progress).
    pub fn job_mut(&mut self, job_id: u64) -> Option<&mut PrintJob> {
        self.jobs.iter_mut().find(|j| j.id == job_id)
    }

    /// Returns all active (non-terminal) jobs.
    pub fn active_jobs(&self) -> Vec<&PrintJob> {
        self.jobs.iter().filter(|j| j.is_active()).collect()
    }

    /// Returns all completed/cancelled/failed jobs.
    pub fn history(&self) -> Vec<&PrintJob> {
        self.jobs.iter().filter(|j| !j.is_active()).collect()
    }

    /// Total number of jobs (active + historical).
    pub fn total_jobs(&self) -> usize {
        self.jobs.len()
    }

    /// Mark a job as started (transition from Queued to Printing).
    pub fn start_job(&mut self, job_id: u64) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            if matches!(job.status, JobStatus::Queued) {
                job.status = JobStatus::Printing;
                job.started_at = Some(current_timestamp_us());
                tracing::debug!("Print job {} started", job_id);
            }
        }
    }

    /// Record that a page has been printed for a job.
    pub fn advance_page(&mut self, job_id: u64) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            if matches!(job.status, JobStatus::Printing) {
                job.pages_printed += 1;
                if job.pages_printed >= job.total_pages {
                    job.status = JobStatus::Completed;
                    job.completed_at = Some(current_timestamp_us());
                    tracing::info!("Print job {} completed", job_id);
                }
            }
        }
    }

    /// Mark a job as failed with an error message.
    pub fn fail_job(&mut self, job_id: u64, reason: impl Into<String>) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            if !job.status.is_terminal() {
                let msg = reason.into();
                tracing::error!("Print job {} failed: {}", job_id, msg);
                job.status = JobStatus::Failed(msg);
                job.completed_at = Some(current_timestamp_us());
            }
        }
    }

    /// Add a printer manually (e.g., from saved configuration).
    pub fn add_printer(&mut self, printer: Printer) {
        self.printers.push(printer);
    }

    /// Remove all completed/cancelled/failed jobs from history.
    pub fn clear_history(&mut self) {
        self.jobs.retain(|j| j.is_active());
    }
}

impl Default for PrintManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the current timestamp in microseconds (monotonic-ish).
fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}
