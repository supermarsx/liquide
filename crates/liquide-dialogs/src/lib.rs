pub mod color_picker;
pub mod file_picker;
pub mod font_picker;
pub mod input_dialog;
pub mod message_box;
pub mod progress;

/// Dialog result — user confirmed or cancelled
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult<T> {
    Ok(T),
    Cancelled,
    Invalid(String),
}

/// Dialog ID for tracking multiple open dialogs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DialogId(pub u64);

/// Common dialog trait
pub trait Dialog {
    type Output;
    fn id(&self) -> DialogId;
    fn title(&self) -> &str;
    fn is_modal(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod popup_bridge_tests {
    use super::color_picker::ColorPickerState;
    use super::file_picker::{FilePickerConfig, FilePickerState};
    use super::font_picker::FontPickerState;
    use super::input_dialog::InputDialog;
    use super::message_box::MessageBox;
    use super::progress::ProgressDialog;
    use liquide_popups::{DialogInfo, PopupManager, PopupType, WindowId};

    fn assert_dialog_popup<D: DialogInfo>(dialog: &D, owner: WindowId) {
        let mut manager = PopupManager::default();
        let expected_size = dialog.preferred_size();

        let id = manager.show_dialog(dialog, owner);
        let popup = manager.get(id).unwrap();

        assert_eq!(popup.popup_type, PopupType::Dialog);
        assert_eq!(popup.owner, Some(owner));
        assert_eq!((popup.bounds.width, popup.bounds.height), expected_size);
    }

    #[test]
    fn message_box_popup_bridge_uses_owner_window() {
        assert_dialog_popup(&MessageBox::confirm("Confirm", "Proceed?"), WindowId(7));
    }

    #[test]
    fn input_dialog_popup_bridge_uses_owner_window() {
        assert_dialog_popup(&InputDialog::new(super::DialogId(1), "Rename", "Name"), WindowId(8));
    }

    #[test]
    fn file_picker_popup_bridge_uses_owner_window() {
        let dialog = FilePickerState::new(FilePickerConfig::default());
        assert_dialog_popup(&dialog, WindowId(9));
    }

    #[test]
    fn color_picker_popup_bridge_uses_owner_window() {
        assert_dialog_popup(&ColorPickerState::new(super::DialogId(2), "Color"), WindowId(10));
    }

    #[test]
    fn font_picker_popup_bridge_uses_owner_window() {
        assert_dialog_popup(&FontPickerState::new(super::DialogId(3), "Font"), WindowId(11));
    }

    #[test]
    fn progress_dialog_popup_bridge_uses_owner_window() {
        assert_dialog_popup(&ProgressDialog::new(super::DialogId(4), "Progress", "Working"), WindowId(12));
    }
}
