use crate::{Dialog, DialogId};
use liquide_popups::DialogInfo;

/// Progress mode — determinate (0.0-1.0) or indeterminate (spinner)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressMode {
    Determinate(f32),
    Indeterminate,
}

/// Progress dialog state
#[derive(Debug)]
pub struct ProgressDialog {
    pub id: DialogId,
    pub title: String,
    pub message: String,
    pub progress: ProgressMode,
    pub cancellable: bool,
    pub cancelled: bool,
}

impl ProgressDialog {
    pub fn new(id: DialogId, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            message: message.into(),
            progress: ProgressMode::Indeterminate,
            cancellable: true,
            cancelled: false,
        }
    }

    /// Create a non-cancellable progress dialog
    pub fn non_cancellable(
        id: DialogId,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            cancellable: false,
            ..Self::new(id, title, message)
        }
    }

    /// Set the progress value (0.0 to 1.0), switches to determinate mode
    pub fn set_progress(&mut self, value: f32) {
        self.progress = ProgressMode::Determinate(value.clamp(0.0, 1.0));
    }

    /// Switch to indeterminate (spinner) mode
    pub fn set_indeterminate(&mut self) {
        self.progress = ProgressMode::Indeterminate;
    }

    /// Update the message text
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Request cancellation (only if cancellable)
    pub fn request_cancel(&mut self) {
        if self.cancellable {
            self.cancelled = true;
        }
    }

    /// Check if cancellation was requested
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Get the progress fraction (None for indeterminate)
    pub fn fraction(&self) -> Option<f32> {
        match self.progress {
            ProgressMode::Determinate(v) => Some(v),
            ProgressMode::Indeterminate => None,
        }
    }

    /// Check if progress is complete (>= 1.0)
    pub fn is_complete(&self) -> bool {
        matches!(self.progress, ProgressMode::Determinate(v) if v >= 1.0)
    }
}

impl Dialog for ProgressDialog {
    type Output = bool; // true = completed, false = cancelled
    fn id(&self) -> DialogId {
        self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
}

impl DialogInfo for ProgressDialog {
    fn preferred_size(&self) -> (f32, f32) {
        let height = if self.cancellable { 170.0 } else { 150.0 };
        (420.0, height)
    }

    fn title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_progress() {
        let dlg = ProgressDialog::new(DialogId(1), "Loading", "Please wait...");
        assert_eq!(dlg.title, "Loading");
        assert_eq!(dlg.message, "Please wait...");
        assert_eq!(dlg.progress, ProgressMode::Indeterminate);
        assert!(dlg.cancellable);
        assert!(!dlg.cancelled);
    }

    #[test]
    fn test_non_cancellable() {
        let dlg = ProgressDialog::non_cancellable(DialogId(1), "Working", "...");
        assert!(!dlg.cancellable);
    }

    #[test]
    fn test_set_progress() {
        let mut dlg = ProgressDialog::new(DialogId(1), "T", "M");
        dlg.set_progress(0.5);
        assert_eq!(dlg.fraction(), Some(0.5));
        assert!(!dlg.is_complete());

        dlg.set_progress(1.0);
        assert_eq!(dlg.fraction(), Some(1.0));
        assert!(dlg.is_complete());
    }

    #[test]
    fn test_progress_clamped() {
        let mut dlg = ProgressDialog::new(DialogId(1), "T", "M");
        dlg.set_progress(-0.5);
        assert_eq!(dlg.fraction(), Some(0.0));
        dlg.set_progress(2.0);
        assert_eq!(dlg.fraction(), Some(1.0));
    }

    #[test]
    fn test_indeterminate() {
        let mut dlg = ProgressDialog::new(DialogId(1), "T", "M");
        dlg.set_progress(0.5);
        assert!(dlg.fraction().is_some());
        dlg.set_indeterminate();
        assert!(dlg.fraction().is_none());
    }

    #[test]
    fn test_cancel() {
        let mut dlg = ProgressDialog::new(DialogId(1), "T", "M");
        assert!(!dlg.is_cancelled());
        dlg.request_cancel();
        assert!(dlg.is_cancelled());
    }

    #[test]
    fn test_cancel_non_cancellable() {
        let mut dlg = ProgressDialog::non_cancellable(DialogId(1), "T", "M");
        dlg.request_cancel();
        assert!(!dlg.is_cancelled()); // should remain false
    }

    #[test]
    fn test_set_message() {
        let mut dlg = ProgressDialog::new(DialogId(1), "T", "Initial");
        dlg.set_message("Updated");
        assert_eq!(dlg.message, "Updated");
    }

    #[test]
    fn test_dialog_trait() {
        let dlg = ProgressDialog::new(DialogId(42), "Title", "Msg");
        assert_eq!(dlg.id(), DialogId(42));
        assert_eq!(Dialog::title(&dlg), "Title");
        assert!(Dialog::is_modal(&dlg));
    }
}
