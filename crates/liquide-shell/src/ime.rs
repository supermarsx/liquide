//! Input-method (IME) drive on the shell keyboard path (t73-input §1).
//!
//! The shell owns a `liquide_input_method::InputMethodEngine`
//! ([`Shell::input_method`]) and feeds every pressed key into it from
//! `events.rs`. The engine is keysym + produced-text based, so this module
//! provides the one adapter the spec calls for — a `KeyCode → X11/XKB keysym`
//! map — plus the translation of the engine's `InputAction` into a small
//! [`ImeOutcome`] the keyboard path acts on.
//!
//! Defaults: the engine starts in Direct mode and inactive, so for an
//! ASCII-input session every key returns [`ImeOutcome::Forward`] and the shell's
//! existing text-input / shortcut handling runs unchanged. Composition only
//! begins after Ctrl+Space (activate) or a mode switch.

use liquide_input::keyboard::KeyCode;
use liquide_input_method::{InputAction, KeyEvent as ImeKeyEvent};

use crate::shell::Shell;

/// What the keyboard path should do after feeding a key to the IME engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeOutcome {
    /// The engine committed this text — route it into the focused window as if
    /// typed, then redraw.
    Commit(String),
    /// The engine consumed the key for composition (preedit / candidates /
    /// mode switch) — redraw and stop; do NOT fall through to text/shortcuts.
    Consumed,
    /// The engine did not consume the key — fall through to the shell's existing
    /// text-input seam and shortcut table.
    Forward,
}

impl Shell {
    /// Feed a key event into the IME engine and translate the resulting action
    /// (t73-input §1). Mirrors any preedit into [`Shell::ime_preedit`] so the
    /// scene/host can render the composition where feasible.
    pub(crate) fn drive_input_method(
        &mut self,
        ke: &liquide_input::keyboard::KeyEvent,
    ) -> ImeOutcome {
        let keysym = keycode_to_keysym(ke.key, ke.modifiers.shift());
        let text = keycode_to_text(ke.key, ke.modifiers.shift());
        let ev = ImeKeyEvent::from_parts(
            keysym,
            text,
            ke.modifiers.shift(),
            ke.modifiers.ctrl(),
            ke.modifiers.alt(),
            ke.modifiers.super_key(),
        );
        match self.input_method.process_key(ev) {
            InputAction::Commit(s) => {
                self.ime_preedit.clear();
                ImeOutcome::Commit(s)
            }
            InputAction::UpdatePreedit(p) => {
                self.ime_preedit = p.text.clone();
                self.mark_window_scene_dirty();
                ImeOutcome::Consumed
            }
            InputAction::ShowCandidates(_) => {
                self.mark_window_scene_dirty();
                ImeOutcome::Consumed
            }
            InputAction::HideCandidates => {
                self.ime_preedit.clear();
                self.mark_window_scene_dirty();
                ImeOutcome::Consumed
            }
            InputAction::SwitchMode(_) => {
                self.ime_preedit.clear();
                ImeOutcome::Consumed
            }
            InputAction::Forward => ImeOutcome::Forward,
        }
    }

    /// The current IME preedit (composition) string, for hosts/tests/scene
    /// rendering. Empty when not composing (t73-input §1).
    #[must_use]
    pub fn ime_preedit(&self) -> &str {
        &self.ime_preedit
    }

    /// Whether the IME engine is currently active (composing-capable). Read-only
    /// observability for tests/hosts.
    #[must_use]
    pub fn ime_active(&self) -> bool {
        self.input_method.state().active
    }

    /// Mutable access to the IME engine so a host can switch modes / configure it
    /// (e.g. enable Pinyin/Hiragana from a settings panel). The shell drives
    /// `process_key` itself on the keyboard path (t73-input §1).
    pub fn input_method_mut(&mut self) -> &mut liquide_input_method::InputMethodEngine {
        &mut self.input_method
    }
}

/// The ASCII text a key produces (shift-aware for the cases the IME cares
/// about), or `None` for non-text keys. The IME's romaji/pinyin/emoji modes
/// consume `key.text`, so this must carry the produced character.
fn keycode_to_text(key: KeyCode, shift: bool) -> Option<String> {
    let lower = Shell::keycode_to_char(key)?;
    let ch = if shift {
        lower.to_ascii_uppercase()
    } else {
        lower
    };
    Some(ch.to_string())
}

/// Map a live `liquide_input::KeyCode` to the X11/XKB keysym the IME engine
/// keys on (t73-input §1 adapter). Printable ASCII keys map to their Latin-1
/// keysym (which equals the character code); the navigation/edit keys map to the
/// well-known `XK_*` function keysyms the engine special-cases. Unknown keys map
/// to `0` (the engine treats them as inert / Forward).
pub(crate) fn keycode_to_keysym(key: KeyCode, shift: bool) -> u32 {
    // Printable keys: the keysym for Latin-1 printable characters equals the
    // Unicode/ASCII code point, which is exactly what the engine compares
    // against (XK_SPACE == 0x20, letters/digits == their ASCII codes).
    if let Some(text) = keycode_to_text(key, shift) {
        if let Some(ch) = text.chars().next() {
            return ch as u32;
        }
    }

    // Navigation / editing keys → well-known XK_* keysyms.
    match key {
        KeyCode::Enter => 0xff0d,     // XK_Return
        KeyCode::Escape => 0xff1b,    // XK_Escape
        KeyCode::Backspace => 0xff08, // XK_BackSpace
        KeyCode::Tab => 0xff09,       // XK_Tab
        KeyCode::ArrowUp => 0xff52,   // XK_Up
        KeyCode::ArrowDown => 0xff54, // XK_Down
        KeyCode::ArrowLeft => 0xff51, // XK_Left
        KeyCode::ArrowRight => 0xff53, // XK_Right
        KeyCode::Home => 0xff50,      // XK_Home
        KeyCode::End => 0xff57,       // XK_End
        KeyCode::PageUp => 0xff55,    // XK_Page_Up
        KeyCode::PageDown => 0xff56,  // XK_Page_Down
        KeyCode::Delete => 0xffff,    // XK_Delete
        _ => 0,
    }
}
