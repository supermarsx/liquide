//! Print job tracking.

use crate::printer::PrinterId;
use crate::settings::PrintSettings;

/// Status of a print job.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    /// Job is queued, waiting to start.
    Queued,
    /// Job is currently printing.
    Printing,
    /// Job completed successfully.
    Completed,
    /// Job was cancelled by the user.
    Cancelled,
    /// Job failed with an error.
    Failed(String),
}

impl JobStatus {
    /// Returns `true` if the job is in a terminal state (completed, cancelled, or failed).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Cancelled | JobStatus::Failed(_)
        )
    }

    /// Returns a human-readable label for this status.
    pub fn label(&self) -> &str {
        match self {
            JobStatus::Queued => "Queued",
            JobStatus::Printing => "Printing",
            JobStatus::Completed => "Completed",
            JobStatus::Cancelled => "Cancelled",
            JobStatus::Failed(_) => "Failed",
        }
    }
}

/// A print job managed by the print system.
#[derive(Debug, Clone)]
pub struct PrintJob {
    /// Unique job identifier.
    pub id: u64,
    /// Printer this job targets.
    pub printer_id: PrinterId,
    /// Name of the document being printed.
    pub document_name: String,
    /// Print settings for this job.
    pub settings: PrintSettings,
    /// Current job status.
    pub status: JobStatus,
    /// Number of pages printed so far.
    pub pages_printed: u32,
    /// Total number of pages in the document.
    pub total_pages: u32,
    /// Timestamp (epoch microseconds) when the job was created.
    pub created_at: u64,
    /// Timestamp when printing actually started.
    pub started_at: Option<u64>,
    /// Timestamp when the job finished (completed, cancelled, or failed).
    pub completed_at: Option<u64>,
}

impl PrintJob {
    /// Returns the printing progress as a fraction in [0.0, 1.0].
    pub fn progress(&self) -> f32 {
        if self.total_pages == 0 {
            return 0.0;
        }
        (self.pages_printed as f32 / self.total_pages as f32).min(1.0)
    }

    /// Returns `true` if this job is still active (queued or printing).
    pub fn is_active(&self) -> bool {
        !self.status.is_terminal()
    }

    /// Returns the elapsed time in microseconds from creation to completion,
    /// or `None` if the job hasn't completed.
    pub fn duration_us(&self) -> Option<u64> {
        self.completed_at.map(|end| end.saturating_sub(self.created_at))
    }
}
