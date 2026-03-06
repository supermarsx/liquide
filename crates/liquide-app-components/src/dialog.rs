//! Dialog models for confirmations, alerts, and progress indicators.

use serde::{Deserialize, Serialize};

/// The kind of dialog (determines layout and buttons).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DialogKind {
    /// Two-button confirmation dialog.
    Confirm {
        message: String,
        confirm_label: String,
        cancel_label: String,
    },
    /// Single-button alert/info dialog.
    Alert {
        message: String,
        ok_label: String,
    },
    /// Progress indicator dialog.
    Progress {
        message: String,
        /// Progress value 0.0–1.0.
        progress: f32,
        /// Whether the operation can be cancelled.
        cancelable: bool,
    },
}

/// A dialog with a title, kind-specific content, and optional icon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dialog {
    /// Dialog title.
    pub title: String,
    /// Dialog content variant.
    pub kind: DialogKind,
    /// Optional icon identifier.
    pub icon: Option<String>,
}

/// The user's response to a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogResponse {
    /// Confirmed / OK pressed.
    Confirm,
    /// Cancel pressed or dialog dismissed.
    Cancel,
}

impl Dialog {
    /// Create a confirmation dialog.
    pub fn confirm(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            kind: DialogKind::Confirm {
                message: message.into(),
                confirm_label: "OK".into(),
                cancel_label: "Cancel".into(),
            },
            icon: None,
        }
    }

    /// Create an alert dialog.
    pub fn alert(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            kind: DialogKind::Alert {
                message: message.into(),
                ok_label: "OK".into(),
            },
            icon: None,
        }
    }

    /// Create a progress dialog.
    pub fn progress(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            kind: DialogKind::Progress {
                message: message.into(),
                progress: 0.0,
                cancelable: false,
            },
            icon: None,
        }
    }

    /// Set the icon.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_confirm() {
        let d = Dialog::confirm("Delete?", "Are you sure?");
        assert_eq!(d.title, "Delete?");
        assert!(matches!(d.kind, DialogKind::Confirm { .. }));
    }

    #[test]
    fn dialog_alert() {
        let d = Dialog::alert("Error", "Something went wrong").with_icon("warning");
        assert_eq!(d.icon.as_deref(), Some("warning"));
        assert!(matches!(d.kind, DialogKind::Alert { .. }));
    }

    #[test]
    fn dialog_progress() {
        let d = Dialog::progress("Installing", "Please wait...");
        if let DialogKind::Progress { progress, cancelable, .. } = &d.kind {
            assert_eq!(*progress, 0.0);
            assert!(!cancelable);
        } else {
            panic!("expected Progress kind");
        }
    }
}
