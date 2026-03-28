pub mod file_picker;
pub mod color_picker;
pub mod font_picker;
pub mod message_box;
pub mod progress;
pub mod input_dialog;

/// Dialog result — user confirmed or cancelled
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult<T> {
    Ok(T),
    Cancelled,
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
