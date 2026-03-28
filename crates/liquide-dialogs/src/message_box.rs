use crate::{Dialog, DialogId, DialogResult};

#[derive(Debug, Clone)]
pub struct MessageBox {
    pub id: DialogId,
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub icon: MessageIcon,
    pub buttons: Vec<MessageButton>,
    pub default_button: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageIcon {
    Info,
    Warning,
    Error,
    Question,
    None,
}

#[derive(Debug, Clone)]
pub struct MessageButton {
    pub label: String,
    pub id: ButtonId,
    pub is_destructive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonId {
    Ok,
    Cancel,
    Yes,
    No,
    Save,
    DontSave,
    Discard,
    Apply,
    Retry,
    Close,
    Custom(u32),
}

static NEXT_MSG_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);

fn next_id() -> DialogId {
    DialogId(NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

impl MessageBox {
    /// Information dialog with OK button
    pub fn info(title: &str, message: &str) -> Self {
        Self {
            id: next_id(),
            title: title.into(),
            message: message.into(),
            detail: None,
            icon: MessageIcon::Info,
            buttons: vec![MessageButton {
                label: "OK".into(),
                id: ButtonId::Ok,
                is_destructive: false,
            }],
            default_button: 0,
        }
    }

    /// Warning dialog with OK button
    pub fn warning(title: &str, message: &str) -> Self {
        Self {
            id: next_id(),
            title: title.into(),
            message: message.into(),
            detail: None,
            icon: MessageIcon::Warning,
            buttons: vec![MessageButton {
                label: "OK".into(),
                id: ButtonId::Ok,
                is_destructive: false,
            }],
            default_button: 0,
        }
    }

    /// Error dialog with OK button
    pub fn error(title: &str, message: &str) -> Self {
        Self {
            id: next_id(),
            title: title.into(),
            message: message.into(),
            detail: None,
            icon: MessageIcon::Error,
            buttons: vec![MessageButton {
                label: "OK".into(),
                id: ButtonId::Ok,
                is_destructive: false,
            }],
            default_button: 0,
        }
    }

    /// Confirmation dialog with Yes / No buttons
    pub fn confirm(title: &str, message: &str) -> Self {
        Self {
            id: next_id(),
            title: title.into(),
            message: message.into(),
            detail: None,
            icon: MessageIcon::Question,
            buttons: vec![
                MessageButton {
                    label: "Yes".into(),
                    id: ButtonId::Yes,
                    is_destructive: false,
                },
                MessageButton {
                    label: "No".into(),
                    id: ButtonId::No,
                    is_destructive: false,
                },
            ],
            default_button: 0,
        }
    }

    /// Save / Discard / Cancel dialog (unsaved changes pattern)
    pub fn save_discard_cancel(title: &str, message: &str) -> Self {
        Self {
            id: next_id(),
            title: title.into(),
            message: message.into(),
            detail: None,
            icon: MessageIcon::Warning,
            buttons: vec![
                MessageButton {
                    label: "Save".into(),
                    id: ButtonId::Save,
                    is_destructive: false,
                },
                MessageButton {
                    label: "Discard".into(),
                    id: ButtonId::Discard,
                    is_destructive: true,
                },
                MessageButton {
                    label: "Cancel".into(),
                    id: ButtonId::Cancel,
                    is_destructive: false,
                },
            ],
            default_button: 0,
        }
    }

    /// Set detail text (shown below the message in smaller font)
    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Handle button click by index
    pub fn click(&self, button_index: usize) -> DialogResult<ButtonId> {
        if let Some(button) = self.buttons.get(button_index) {
            DialogResult::Ok(button.id)
        } else {
            DialogResult::Cancelled
        }
    }
}

impl Dialog for MessageBox {
    type Output = ButtonId;
    fn id(&self) -> DialogId {
        self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info() {
        let mb = MessageBox::info("Title", "Message");
        assert_eq!(mb.title, "Title");
        assert_eq!(mb.message, "Message");
        assert_eq!(mb.icon, MessageIcon::Info);
        assert_eq!(mb.buttons.len(), 1);
        assert_eq!(mb.buttons[0].id, ButtonId::Ok);
    }

    #[test]
    fn test_warning() {
        let mb = MessageBox::warning("Warn", "Be careful");
        assert_eq!(mb.icon, MessageIcon::Warning);
        assert_eq!(mb.buttons[0].id, ButtonId::Ok);
    }

    #[test]
    fn test_error() {
        let mb = MessageBox::error("Error", "Something failed");
        assert_eq!(mb.icon, MessageIcon::Error);
        assert_eq!(mb.buttons[0].id, ButtonId::Ok);
    }

    #[test]
    fn test_confirm() {
        let mb = MessageBox::confirm("Confirm", "Are you sure?");
        assert_eq!(mb.icon, MessageIcon::Question);
        assert_eq!(mb.buttons.len(), 2);
        assert_eq!(mb.buttons[0].id, ButtonId::Yes);
        assert_eq!(mb.buttons[1].id, ButtonId::No);
    }

    #[test]
    fn test_save_discard_cancel() {
        let mb = MessageBox::save_discard_cancel("Unsaved", "Save changes?");
        assert_eq!(mb.buttons.len(), 3);
        assert_eq!(mb.buttons[0].id, ButtonId::Save);
        assert_eq!(mb.buttons[1].id, ButtonId::Discard);
        assert_eq!(mb.buttons[2].id, ButtonId::Cancel);
        assert!(mb.buttons[1].is_destructive);
        assert!(!mb.buttons[0].is_destructive);
    }

    #[test]
    fn test_with_detail() {
        let mb = MessageBox::info("Title", "Msg").with_detail("Extra info");
        assert_eq!(mb.detail.as_deref(), Some("Extra info"));
    }

    #[test]
    fn test_click_valid() {
        let mb = MessageBox::confirm("Q", "?");
        match mb.click(0) {
            DialogResult::Ok(id) => assert_eq!(id, ButtonId::Yes),
            _ => panic!("expected Ok"),
        }
        match mb.click(1) {
            DialogResult::Ok(id) => assert_eq!(id, ButtonId::No),
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn test_click_invalid() {
        let mb = MessageBox::info("T", "M");
        assert_eq!(mb.click(5), DialogResult::Cancelled);
    }

    #[test]
    fn test_unique_ids() {
        let a = MessageBox::info("A", "A");
        let b = MessageBox::info("B", "B");
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_dialog_trait() {
        let mb = MessageBox::info("Title", "Msg");
        assert_eq!(mb.title(), "Title");
        assert!(mb.is_modal());
    }
}
