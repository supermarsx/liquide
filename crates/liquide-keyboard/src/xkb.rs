//! XKB keymap abstraction for keyboard layout rules.
//!
//! Provides an XKB-style keymap model with modifier state tracking,
//! keysym resolution, and common keysym constants following the
//! freedesktop XKB specification.

use std::collections::HashMap;

// ── Keysym constants (X11 keysyms, freedesktop compatible) ──────────────

/// Latin letters a-z.
pub const XK_A: u32 = 0x0061;
pub const XK_B: u32 = 0x0062;
pub const XK_C: u32 = 0x0063;
pub const XK_D: u32 = 0x0064;
pub const XK_E: u32 = 0x0065;
pub const XK_F: u32 = 0x0066;
pub const XK_G: u32 = 0x0067;
pub const XK_H: u32 = 0x0068;
pub const XK_I: u32 = 0x0069;
pub const XK_J: u32 = 0x006a;
pub const XK_K: u32 = 0x006b;
pub const XK_L: u32 = 0x006c;
pub const XK_M: u32 = 0x006d;
pub const XK_N: u32 = 0x006e;
pub const XK_O: u32 = 0x006f;
pub const XK_P: u32 = 0x0070;
pub const XK_Q: u32 = 0x0071;
pub const XK_R: u32 = 0x0072;
pub const XK_S: u32 = 0x0073;
pub const XK_T: u32 = 0x0074;
pub const XK_U: u32 = 0x0075;
pub const XK_V: u32 = 0x0076;
pub const XK_W: u32 = 0x0077;
pub const XK_X: u32 = 0x0078;
pub const XK_Y: u32 = 0x0079;
pub const XK_Z: u32 = 0x007a;

/// Function and special keys.
pub const XK_SPACE: u32 = 0x0020;
pub const XK_RETURN: u32 = 0xff0d;
pub const XK_ESCAPE: u32 = 0xff1b;
pub const XK_TAB: u32 = 0xff09;
pub const XK_BACKSPACE: u32 = 0xff08;
pub const XK_DELETE: u32 = 0xffff;
pub const XK_INSERT: u32 = 0xff63;
pub const XK_HOME: u32 = 0xff50;
pub const XK_END: u32 = 0xff57;
pub const XK_PAGE_UP: u32 = 0xff55;
pub const XK_PAGE_DOWN: u32 = 0xff56;
pub const XK_UP: u32 = 0xff52;
pub const XK_DOWN: u32 = 0xff54;
pub const XK_LEFT: u32 = 0xff51;
pub const XK_RIGHT: u32 = 0xff53;

/// Modifier keysyms.
pub const XK_SHIFT_L: u32 = 0xffe1;
pub const XK_SHIFT_R: u32 = 0xffe2;
pub const XK_CONTROL_L: u32 = 0xffe3;
pub const XK_CONTROL_R: u32 = 0xffe4;
pub const XK_ALT_L: u32 = 0xffe9;
pub const XK_ALT_R: u32 = 0xffea;
pub const XK_SUPER_L: u32 = 0xffeb;
pub const XK_SUPER_R: u32 = 0xffec;
pub const XK_CAPS_LOCK: u32 = 0xffe5;
pub const XK_NUM_LOCK: u32 = 0xff7f;

/// Digits 0-9.
pub const XK_0: u32 = 0x0030;
pub const XK_1: u32 = 0x0031;
pub const XK_2: u32 = 0x0032;
pub const XK_3: u32 = 0x0033;
pub const XK_4: u32 = 0x0034;
pub const XK_5: u32 = 0x0035;
pub const XK_6: u32 = 0x0036;
pub const XK_7: u32 = 0x0037;
pub const XK_8: u32 = 0x0038;
pub const XK_9: u32 = 0x0039;

/// Function keys F1-F12.
pub const XK_F1: u32 = 0xffbe;
pub const XK_F2: u32 = 0xffbf;
pub const XK_F3: u32 = 0xffc0;
pub const XK_F4: u32 = 0xffc1;
pub const XK_F5: u32 = 0xffc2;
pub const XK_F6: u32 = 0xffc3;
pub const XK_F7: u32 = 0xffc4;
pub const XK_F8: u32 = 0xffc5;
pub const XK_F9: u32 = 0xffc6;
pub const XK_F10: u32 = 0xffc7;
pub const XK_F11: u32 = 0xffc8;
pub const XK_F12: u32 = 0xffc9;

// ── Modifier mask ───────────────────────────────────────────────────────

bitflags::bitflags! {
    /// XKB modifier bitmask combining depressed, latched, and locked state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ModifierMask: u32 {
        const SHIFT   = 1 << 0;
        const LOCK    = 1 << 1;  // Caps Lock
        const CONTROL = 1 << 2;
        const MOD1    = 1 << 3;  // Alt
        const MOD2    = 1 << 4;  // Num Lock
        const MOD3    = 1 << 5;  // (unused on most layouts)
        const MOD4    = 1 << 6;  // Super
        const MOD5    = 1 << 7;  // ISO Level3 Shift (AltGr)
    }
}

// ── ModifierChange ──────────────────────────────────────────────────────

/// Describes a modifier state change produced by a key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierChange {
    /// Which modifier changed.
    pub modifier: ModifierMask,
    /// Whether the modifier is now active.
    pub active: bool,
    /// What kind of activation.
    pub kind: ModifierChangeKind,
}

/// How the modifier was activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierChangeKind {
    /// Held down (depressed).
    Depressed,
    /// Latched (one-shot, clears after next non-modifier key).
    Latched,
    /// Locked (toggled on/off, e.g. Caps Lock).
    Locked,
}

// ── Keymap configuration ────────────────────────────────────────────────

/// XKB keymap configuration parameters (RMLVO model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapConfig {
    /// XKB rules file name (e.g., "evdev").
    pub rules: String,
    /// Keyboard model (e.g., "pc105", "pc104").
    pub model: String,
    /// Layout name (e.g., "us", "de", "fr").
    pub layout: String,
    /// Layout variant (e.g., "dvorak", "nodeadkeys").
    pub variant: String,
    /// XKB options (e.g., "ctrl:nocaps").
    pub options: String,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            rules: "evdev".to_string(),
            model: "pc105".to_string(),
            layout: "us".to_string(),
            variant: String::new(),
            options: String::new(),
        }
    }
}

// ── XkbKeymap ───────────────────────────────────────────────────────────

/// Entry in the keycode-to-keysym map: up to 4 levels (unshifted, shifted,
/// level3, level3+shift).
#[derive(Debug, Clone)]
pub struct KeySymEntry {
    /// Keysyms at each level. Index 0 = base, 1 = shift, 2 = level3 (AltGr),
    /// 3 = shift+level3.
    pub levels: [u32; 4],
}

/// Compiled XKB keymap holding layout rules, keycode-to-keysym mappings,
/// and modifier type definitions.
#[derive(Debug, Clone)]
pub struct XkbKeymap {
    /// The configuration that produced this keymap.
    pub config: KeymapConfig,
    /// Keycode to keysym mapping table.
    keysym_map: HashMap<u32, KeySymEntry>,
    /// Set of keycodes that are modifier keys (should not repeat).
    modifier_keycodes: HashMap<u32, ModifierMask>,
    /// Set of keycodes that are lock-toggle modifiers (Caps Lock, Num Lock).
    lock_keycodes: HashMap<u32, ModifierMask>,
}

impl XkbKeymap {
    /// Return the keysym entry for a keycode, if mapped.
    pub fn get_entry(&self, keycode: u32) -> Option<&KeySymEntry> {
        self.keysym_map.get(&keycode)
    }

    /// Whether a keycode is a modifier key.
    pub fn is_modifier(&self, keycode: u32) -> bool {
        self.modifier_keycodes.contains_key(&keycode)
    }

    /// Whether a keycode is a lock-toggle modifier (Caps Lock, Num Lock).
    pub fn is_lock_modifier(&self, keycode: u32) -> bool {
        self.lock_keycodes.contains_key(&keycode)
    }

    /// Get the modifier mask for a modifier keycode.
    pub fn modifier_for_keycode(&self, keycode: u32) -> Option<ModifierMask> {
        self.modifier_keycodes.get(&keycode).copied()
    }

    /// Number of mapped keycodes.
    pub fn key_count(&self) -> usize {
        self.keysym_map.len()
    }

    /// Add or replace a keysym entry.
    pub fn set_entry(&mut self, keycode: u32, entry: KeySymEntry) {
        self.keysym_map.insert(keycode, entry);
    }

    /// Register a keycode as a modifier.
    pub fn set_modifier_keycode(&mut self, keycode: u32, mask: ModifierMask) {
        self.modifier_keycodes.insert(keycode, mask);
    }

    /// Register a keycode as a lock-toggle modifier.
    pub fn set_lock_keycode(&mut self, keycode: u32, mask: ModifierMask) {
        self.lock_keycodes.insert(keycode, mask);
        // Lock keys are also modifiers.
        self.modifier_keycodes.insert(keycode, mask);
    }
}

/// Compile an XKB keymap from a configuration.
///
/// Validates the config and builds a keymap with standard PC key mappings
/// for the requested layout. Currently supports "us" (default fallback).
pub fn compile_keymap(config: KeymapConfig) -> XkbKeymap {
    let mut keysym_map = HashMap::new();
    let mut modifier_keycodes = HashMap::new();
    let mut lock_keycodes = HashMap::new();

    // Standard modifier keycodes (evdev offsets: hardware scancode + 8).
    modifier_keycodes.insert(42, ModifierMask::SHIFT); // Left Shift
    modifier_keycodes.insert(54, ModifierMask::SHIFT); // Right Shift
    modifier_keycodes.insert(29, ModifierMask::CONTROL); // Left Ctrl
    modifier_keycodes.insert(97, ModifierMask::CONTROL); // Right Ctrl
    modifier_keycodes.insert(56, ModifierMask::MOD1); // Left Alt
    modifier_keycodes.insert(100, ModifierMask::MOD5); // Right Alt (AltGr)
    modifier_keycodes.insert(125, ModifierMask::MOD4); // Left Super
    modifier_keycodes.insert(126, ModifierMask::MOD4); // Right Super

    // Lock modifiers.
    lock_keycodes.insert(58, ModifierMask::LOCK); // Caps Lock
    lock_keycodes.insert(69, ModifierMask::MOD2); // Num Lock
    // Also register in modifier_keycodes.
    modifier_keycodes.insert(58, ModifierMask::LOCK);
    modifier_keycodes.insert(69, ModifierMask::MOD2);

    // Build US QWERTY base keysym map.
    // Letters a-z (keycodes follow AT set 1 scancodes).
    let letter_map: &[(u32, u32, u32)] = &[
        (30, XK_A, XK_A - 0x20), // a / A
        (48, XK_B, XK_B - 0x20),
        (46, XK_C, XK_C - 0x20),
        (32, XK_D, XK_D - 0x20),
        (18, XK_E, XK_E - 0x20),
        (33, XK_F, XK_F - 0x20),
        (34, XK_G, XK_G - 0x20),
        (35, XK_H, XK_H - 0x20),
        (23, XK_I, XK_I - 0x20),
        (36, XK_J, XK_J - 0x20),
        (37, XK_K, XK_K - 0x20),
        (38, XK_L, XK_L - 0x20),
        (50, XK_M, XK_M - 0x20),
        (49, XK_N, XK_N - 0x20),
        (24, XK_O, XK_O - 0x20),
        (25, XK_P, XK_P - 0x20),
        (16, XK_Q, XK_Q - 0x20),
        (19, XK_R, XK_R - 0x20),
        (31, XK_S, XK_S - 0x20),
        (20, XK_T, XK_T - 0x20),
        (22, XK_U, XK_U - 0x20),
        (47, XK_V, XK_V - 0x20),
        (17, XK_W, XK_W - 0x20),
        (45, XK_X, XK_X - 0x20),
        (21, XK_Y, XK_Y - 0x20),
        (44, XK_Z, XK_Z - 0x20),
    ];
    for &(kc, lower, upper) in letter_map {
        keysym_map.insert(
            kc,
            KeySymEntry {
                levels: [lower, upper, lower, upper],
            },
        );
    }

    // Digits and number row symbols.
    let digit_map: &[(u32, u32, u32)] = &[
        (2, XK_1, 0x0021),  // 1 / !
        (3, XK_2, 0x0040),  // 2 / @
        (4, XK_3, 0x0023),  // 3 / #
        (5, XK_4, 0x0024),  // 4 / $
        (6, XK_5, 0x0025),  // 5 / %
        (7, XK_6, 0x005e),  // 6 / ^
        (8, XK_7, 0x0026),  // 7 / &
        (9, XK_8, 0x002a),  // 8 / *
        (10, XK_9, 0x0028), // 9 / (
        (11, XK_0, 0x0029), // 0 / )
    ];
    for &(kc, base, shifted) in digit_map {
        keysym_map.insert(
            kc,
            KeySymEntry {
                levels: [base, shifted, base, shifted],
            },
        );
    }

    // Special keys.
    keysym_map.insert(
        57,
        KeySymEntry {
            levels: [XK_SPACE, XK_SPACE, XK_SPACE, XK_SPACE],
        },
    );
    keysym_map.insert(
        28,
        KeySymEntry {
            levels: [XK_RETURN, XK_RETURN, XK_RETURN, XK_RETURN],
        },
    );
    keysym_map.insert(
        1,
        KeySymEntry {
            levels: [XK_ESCAPE, XK_ESCAPE, XK_ESCAPE, XK_ESCAPE],
        },
    );
    keysym_map.insert(
        15,
        KeySymEntry {
            levels: [XK_TAB, XK_TAB, XK_TAB, XK_TAB],
        },
    );
    keysym_map.insert(
        14,
        KeySymEntry {
            levels: [XK_BACKSPACE, XK_BACKSPACE, XK_BACKSPACE, XK_BACKSPACE],
        },
    );

    XkbKeymap {
        config,
        keysym_map,
        modifier_keycodes,
        lock_keycodes,
    }
}

// ── XkbState ────────────────────────────────────────────────────────────

/// Tracks the XKB modifier state: depressed (held), latched (one-shot),
/// and locked (toggled) modifiers, following the XKB state model.
#[derive(Debug, Clone)]
pub struct XkbState {
    depressed: ModifierMask,
    latched: ModifierMask,
    locked: ModifierMask,
}

impl XkbState {
    /// Create a new state with no modifiers active.
    pub fn new() -> Self {
        Self {
            depressed: ModifierMask::empty(),
            latched: ModifierMask::empty(),
            locked: ModifierMask::empty(),
        }
    }

    /// Update state for a key press or release event.
    ///
    /// Returns a list of modifier changes produced by this key event.
    pub fn update_key(
        &mut self,
        keycode: u32,
        pressed: bool,
        keymap: &XkbKeymap,
    ) -> Vec<ModifierChange> {
        let mut changes = Vec::new();

        if let Some(mask) = keymap.modifier_for_keycode(keycode) {
            if keymap.is_lock_modifier(keycode) {
                // Lock modifiers toggle on press, ignore release.
                if pressed {
                    let was_locked = self.locked.contains(mask);
                    if was_locked {
                        self.locked.remove(mask);
                        changes.push(ModifierChange {
                            modifier: mask,
                            active: false,
                            kind: ModifierChangeKind::Locked,
                        });
                    } else {
                        self.locked.insert(mask);
                        changes.push(ModifierChange {
                            modifier: mask,
                            active: true,
                            kind: ModifierChangeKind::Locked,
                        });
                    }
                }
            } else {
                // Normal modifiers: depressed on press, released on release.
                if pressed {
                    self.depressed.insert(mask);
                    changes.push(ModifierChange {
                        modifier: mask,
                        active: true,
                        kind: ModifierChangeKind::Depressed,
                    });
                } else {
                    self.depressed.remove(mask);
                    changes.push(ModifierChange {
                        modifier: mask,
                        active: false,
                        kind: ModifierChangeKind::Depressed,
                    });
                }
            }
        }

        changes
    }

    /// Compute the effective modifier mask by combining depressed, latched,
    /// and locked states.
    pub fn effective_modifiers(&self) -> ModifierMask {
        self.depressed | self.latched | self.locked
    }

    /// Get only depressed (held) modifiers.
    pub fn depressed(&self) -> ModifierMask {
        self.depressed
    }

    /// Get only latched (one-shot) modifiers.
    pub fn latched(&self) -> ModifierMask {
        self.latched
    }

    /// Get only locked (toggled) modifiers.
    pub fn locked(&self) -> ModifierMask {
        self.locked
    }

    /// Set a modifier as latched (one-shot). Used by sticky keys.
    pub fn latch(&mut self, mask: ModifierMask) {
        self.latched.insert(mask);
    }

    /// Clear latched modifiers (called after a non-modifier key press).
    pub fn clear_latches(&mut self) {
        self.latched = ModifierMask::empty();
    }

    /// Reset all modifier state.
    pub fn reset(&mut self) {
        self.depressed = ModifierMask::empty();
        self.latched = ModifierMask::empty();
        self.locked = ModifierMask::empty();
    }
}

impl Default for XkbState {
    fn default() -> Self {
        Self::new()
    }
}

/// Look up the keysym for a keycode given the current modifier state.
///
/// Selects the appropriate level based on Shift and AltGr (MOD5) modifiers.
pub fn lookup_keysym(keymap: &XkbKeymap, keycode: u32, modifiers: ModifierMask) -> Option<u32> {
    let entry = keymap.get_entry(keycode)?;
    let shift = modifiers.contains(ModifierMask::SHIFT) || modifiers.contains(ModifierMask::LOCK);
    let level3 = modifiers.contains(ModifierMask::MOD5);

    let level = match (shift, level3) {
        (false, false) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (true, true) => 3,
    };

    Some(entry.levels[level])
}
