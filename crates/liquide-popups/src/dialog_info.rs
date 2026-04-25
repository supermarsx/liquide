//! Minimal dialog description trait.
//!
//! Downstream crates (e.g. `liquide-dialogs`) implement this trait on their
//! concrete dialog types so [`PopupManager::show_dialog`](crate::PopupManager::show_dialog)
//! can render any dialog kind without `liquide-popups` depending on the
//! dialog crate.

use crate::popup::{PopupConfig, WindowId};

/// A value that describes a dialog well enough to render it as a modal popup.
pub trait DialogInfo {
    /// Preferred `(width, height)` of the dialog popup in logical pixels.
    fn preferred_size(&self) -> (f32, f32);

    /// Dialog title — intended for the popup's window chrome.
    fn title(&self) -> &str {
        ""
    }

    /// Whether this dialog is modal. Defaults to `true` because all the
    /// standard `MessageBox`/`FilePicker`/`ColorPicker` dialogs are modal.
    fn is_modal(&self) -> bool {
        true
    }

    /// Build the popup configuration for this dialog without binding it to a
    /// specific owner window.
    fn popup_config(&self) -> PopupConfig {
        self.popup_config_with_owner(None)
    }

    /// Build the popup configuration for this dialog, optionally binding it to
    /// the owning window. `None` produces a global modal dialog instead of
    /// routing through a desktop sentinel window id.
    fn popup_config_with_owner(&self, owner: Option<WindowId>) -> PopupConfig {
        let (width, height) = self.preferred_size();
        let mut config = PopupConfig::dialog_for(width, height, owner);
        config.modal = self.is_modal();
        config
    }
}
