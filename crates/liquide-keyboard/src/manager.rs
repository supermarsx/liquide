//! Keyboard layout manager — manages active layouts, translates scancodes,
//! handles dead key composition, and supports layout cycling.

use bitflags::bitflags;

use crate::builtin::all_builtin_layouts;
use crate::layout::{DeadKeyId, KeyboardLayout};

bitflags! {
    /// Modifier key state bitmask.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u32 {
        /// Left or right Shift.
        const SHIFT     = 0b0000_0001;
        /// Left or right Ctrl.
        const CTRL      = 0b0000_0010;
        /// Left Alt.
        const ALT       = 0b0000_0100;
        /// Right Alt / AltGr.
        const ALT_GR    = 0b0000_1000;
        /// Caps Lock is active.
        const CAPS_LOCK = 0b0001_0000;
        /// Num Lock is active.
        const NUM_LOCK  = 0b0010_0000;
    }
}

/// Result of translating a scancode through the active layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutput {
    /// A normal character was produced.
    Char(char),
    /// A dead key was activated — waiting for the next key press to compose.
    DeadKey(DeadKeyId),
    /// A dead key resolved to a composed character.
    Composed(char),
    /// No character output (modifier-only key, unknown scancode, etc.).
    None,
}

/// Manages keyboard layouts, active layout selection, and scancode translation.
pub struct KeyboardLayoutManager {
    /// All available layouts.
    layouts: Vec<KeyboardLayout>,
    /// Index of the currently active layout.
    active_index: usize,
    /// Pending dead key (if the last key press started a dead key sequence).
    pending_dead_key: Option<DeadKeyId>,
}

impl KeyboardLayoutManager {
    /// Create a new manager with all built-in layouts. US QWERTY is active by default.
    pub fn new() -> Self {
        Self {
            layouts: all_builtin_layouts(),
            active_index: 0,
            pending_dead_key: None,
        }
    }

    /// Create a manager with no layouts (for manual population).
    pub fn empty() -> Self {
        Self {
            layouts: Vec::new(),
            active_index: 0,
            pending_dead_key: None,
        }
    }

    /// Get all available layouts.
    pub fn available_layouts(&self) -> Vec<&KeyboardLayout> {
        self.layouts.iter().collect()
    }

    /// Get the currently active layout.
    ///
    /// Panics if no layouts are loaded.
    pub fn active_layout(&self) -> &KeyboardLayout {
        &self.layouts[self.active_index]
    }

    /// Set the active layout by ID. Returns `true` if found and switched.
    pub fn set_layout(&mut self, id: &str) -> bool {
        if let Some(idx) = self.layouts.iter().position(|l| l.id == id) {
            self.active_index = idx;
            self.pending_dead_key = None;
            true
        } else {
            false
        }
    }

    /// Add a new layout. If a layout with the same ID exists, it is replaced.
    pub fn add_layout(&mut self, layout: KeyboardLayout) {
        if let Some(idx) = self.layouts.iter().position(|l| l.id == layout.id) {
            self.layouts[idx] = layout;
        } else {
            self.layouts.push(layout);
        }
    }

    /// Cycle to the next layout (wraps around). Useful for Alt+Shift switching.
    pub fn next_layout(&mut self) {
        if !self.layouts.is_empty() {
            self.active_index = (self.active_index + 1) % self.layouts.len();
            self.pending_dead_key = None;
        }
    }

    /// Number of loaded layouts.
    pub fn layout_count(&self) -> usize {
        self.layouts.len()
    }

    /// Whether a dead key composition is pending.
    pub fn is_composing(&self) -> bool {
        self.pending_dead_key.is_some()
    }

    /// Cancel any pending dead key composition.
    pub fn cancel_composition(&mut self) {
        self.pending_dead_key = None;
    }

    /// Translate a hardware scancode + modifier state into a `KeyOutput`.
    ///
    /// Handles dead key composition: if the previous key started a dead key,
    /// this call attempts to compose the new key with the pending accent.
    pub fn translate_scancode(&mut self, scancode: u32, modifiers: Modifiers) -> KeyOutput {
        let layout = &self.layouts[self.active_index];

        let mapping = match layout.get(scancode) {
            Some(m) => m,
            None => return KeyOutput::None,
        };

        // Determine the effective shift state (Shift XOR CapsLock for letters).
        let base_char = mapping.normal;
        let is_letter = base_char.is_ascii_alphabetic();
        let shifted = if is_letter {
            modifiers.contains(Modifiers::SHIFT) ^ modifiers.contains(Modifiers::CAPS_LOCK)
        } else {
            modifiers.contains(Modifiers::SHIFT)
        };
        let alt_gr = modifiers.contains(Modifiers::ALT_GR);

        // Pick the character based on modifier state.
        let ch = if shifted && alt_gr {
            mapping.shift_alt_gr.or(mapping.alt_gr).or(mapping.shift).unwrap_or(base_char)
        } else if alt_gr {
            mapping.alt_gr.unwrap_or(base_char)
        } else if shifted {
            mapping.shift.unwrap_or(base_char)
        } else {
            base_char
        };

        // Handle dead key composition.
        if let Some(pending_dk_id) = self.pending_dead_key.take() {
            // Look up the dead key in the active layout.
            if let Some(dk) = layout.get_dead_key(pending_dk_id) {
                if dk.has_combination(ch) {
                    return KeyOutput::Composed(dk.compose(ch));
                }
                // No combination: output the dead key's fallback, then
                // fall through to handle the current key normally.
                // For simplicity, if the current key is itself a dead key we emit
                // the fallback char and start a new dead key sequence.
                if let Some(dk_id) = mapping.dead_key {
                    if !alt_gr {
                        self.pending_dead_key = Some(dk_id);
                        return KeyOutput::Char(dk.fallback);
                    }
                }
                // Current key is normal — output fallback + current as composed.
                // We return the composed pair's first char (fallback) and the
                // caller can check again for the second. For simplicity in this
                // API, we return just the fallback and let the current key
                // be reprocessed. But that's awkward — instead, return the
                // fallback and swallow the current. A real implementation would
                // buffer both. We choose to return just the current character
                // if there's no combination, which is the common XKB behavior.
                return KeyOutput::Char(ch);
            }
            // Dead key ID not found — just output the char.
            return KeyOutput::Char(ch);
        }

        // Check if this key starts a dead key sequence (only without AltGr).
        if let Some(dk_id) = mapping.dead_key {
            if !alt_gr {
                self.pending_dead_key = Some(dk_id);
                return KeyOutput::DeadKey(dk_id);
            }
        }

        KeyOutput::Char(ch)
    }
}

impl Default for KeyboardLayoutManager {
    fn default() -> Self {
        Self::new()
    }
}
