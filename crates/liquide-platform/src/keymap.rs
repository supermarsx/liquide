//! Scancode-to-keycode translation.
//!
//! Provides the [`KeymapTranslator`] trait for mapping platform-specific
//! scancodes to [`liquide_input::KeyCode`] values, and a [`DefaultKeymap`]
//! that returns `None` for all scancodes.

use liquide_input::KeyCode;

/// Translates platform-specific scancodes to logical key codes.
pub trait KeymapTranslator: Send {
    /// Map a raw scancode to a logical key code, if known.
    fn translate_scancode(&self, scancode: u32) -> Option<KeyCode>;

    /// Return the name of the platform this keymap targets.
    #[must_use]
    fn platform_name(&self) -> &str;
}

/// A [`KeymapTranslator`] that reports `"null"` as its platform
/// and returns `None` for every scancode.
#[derive(Debug, Default)]
pub struct DefaultKeymap;

impl KeymapTranslator for DefaultKeymap {
    fn translate_scancode(&self, _scancode: u32) -> Option<KeyCode> {
        None
    }

    fn platform_name(&self) -> &str {
        "null"
    }
}
