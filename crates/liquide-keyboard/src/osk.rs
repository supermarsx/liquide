//! On-screen keyboard (OSK) layout computation.
//!
//! Provides a virtual keyboard representation suitable for rendering
//! in the desktop shell. The layout is based on a standard ANSI 104-key
//! arrangement and adapts labels from the active `KeyboardLayout`.

use crate::layout::KeyboardLayout;

/// The type of an on-screen keyboard key.
#[derive(Debug, Clone, PartialEq)]
pub enum OskKeyType {
    /// A normal character key.
    Normal,
    /// A modifier key (Shift, Ctrl, Alt, etc.).
    Modifier(ModifierKind),
    /// The spacebar.
    Space,
    /// Enter / Return.
    Enter,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
}

/// Which modifier a key represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierKind {
    Shift,
    Ctrl,
    Alt,
    AltGr,
    CapsLock,
    Super,
}

/// A single key in the on-screen keyboard layout.
#[derive(Debug, Clone)]
pub struct OskKey {
    /// Hardware scancode for this key (0 for special keys with no scancode).
    pub scancode: u32,
    /// Display label (e.g., "A", "Shift", "Enter").
    pub label: String,
    /// Width in abstract grid units (normal key = 1.0).
    pub width_units: f32,
    /// Key type.
    pub key_type: OskKeyType,
    /// Computed position: X offset in pixels from the left edge.
    pub x: f32,
    /// Computed position: Y offset in pixels from the top edge.
    pub y: f32,
    /// Computed width in pixels.
    pub w: f32,
    /// Computed height in pixels.
    pub h: f32,
}

/// A row of keys in the on-screen keyboard.
#[derive(Debug, Clone)]
pub struct OskRow {
    pub keys: Vec<OskKey>,
}

/// Complete on-screen keyboard layout with computed pixel positions.
#[derive(Debug, Clone)]
pub struct OskLayout {
    /// Rows of keys, top to bottom.
    pub rows: Vec<OskRow>,
    /// Total width in pixels.
    pub total_width: f32,
    /// Total height in pixels.
    pub total_height: f32,
}

impl OskLayout {
    /// Find the key under a given pixel coordinate, if any.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&OskKey> {
        for row in &self.rows {
            for key in &row.keys {
                if x >= key.x && x < key.x + key.w && y >= key.y && y < key.y + key.h {
                    return Some(key);
                }
            }
        }
        None
    }

    /// Total number of keys.
    pub fn key_count(&self) -> usize {
        self.rows.iter().map(|r| r.keys.len()).sum()
    }
}

/// Definition of a key before position computation: scancode, default label,
/// width in grid units, and type.
struct KeyDef {
    scancode: u32,
    default_label: &'static str,
    width_units: f32,
    key_type: OskKeyType,
}

impl KeyDef {
    const fn normal(scancode: u32, label: &'static str) -> Self {
        Self {
            scancode,
            default_label: label,
            width_units: 1.0,
            key_type: OskKeyType::Normal,
        }
    }

    const fn wide(scancode: u32, label: &'static str, width: f32, key_type: OskKeyType) -> Self {
        Self {
            scancode,
            default_label: label,
            width_units: width,
            key_type,
        }
    }
}

/// Standard ANSI keyboard row definitions (5 rows).
fn standard_rows() -> Vec<Vec<KeyDef>> {
    vec![
        // Row 0: number row (14 keys)
        vec![
            KeyDef::normal(0x29, "`"),    // grave
            KeyDef::normal(0x02, "1"),
            KeyDef::normal(0x03, "2"),
            KeyDef::normal(0x04, "3"),
            KeyDef::normal(0x05, "4"),
            KeyDef::normal(0x06, "5"),
            KeyDef::normal(0x07, "6"),
            KeyDef::normal(0x08, "7"),
            KeyDef::normal(0x09, "8"),
            KeyDef::normal(0x0A, "9"),
            KeyDef::normal(0x0B, "0"),
            KeyDef::normal(0x0C, "-"),
            KeyDef::normal(0x0D, "="),
            KeyDef::wide(0x0E, "Bksp", 2.0, OskKeyType::Backspace),
        ],
        // Row 1: top alpha row (14 keys)
        vec![
            KeyDef::wide(0x0F, "Tab", 1.5, OskKeyType::Tab),
            KeyDef::normal(0x10, "Q"),
            KeyDef::normal(0x11, "W"),
            KeyDef::normal(0x12, "E"),
            KeyDef::normal(0x13, "R"),
            KeyDef::normal(0x14, "T"),
            KeyDef::normal(0x15, "Y"),
            KeyDef::normal(0x16, "U"),
            KeyDef::normal(0x17, "I"),
            KeyDef::normal(0x18, "O"),
            KeyDef::normal(0x19, "P"),
            KeyDef::normal(0x1A, "["),
            KeyDef::normal(0x1B, "]"),
            KeyDef::wide(0x2B, "\\", 1.5, OskKeyType::Normal),
        ],
        // Row 2: home row (13 keys)
        vec![
            KeyDef::wide(0x3A, "Caps", 1.75, OskKeyType::Modifier(ModifierKind::CapsLock)),
            KeyDef::normal(0x1E, "A"),
            KeyDef::normal(0x1F, "S"),
            KeyDef::normal(0x20, "D"),
            KeyDef::normal(0x21, "F"),
            KeyDef::normal(0x22, "G"),
            KeyDef::normal(0x23, "H"),
            KeyDef::normal(0x24, "J"),
            KeyDef::normal(0x25, "K"),
            KeyDef::normal(0x26, "L"),
            KeyDef::normal(0x27, ";"),
            KeyDef::normal(0x28, "'"),
            KeyDef::wide(0x1C, "Enter", 2.25, OskKeyType::Enter),
        ],
        // Row 3: bottom alpha row (12 keys)
        vec![
            KeyDef::wide(0x2A, "Shift", 2.25, OskKeyType::Modifier(ModifierKind::Shift)),
            KeyDef::normal(0x2C, "Z"),
            KeyDef::normal(0x2D, "X"),
            KeyDef::normal(0x2E, "C"),
            KeyDef::normal(0x2F, "V"),
            KeyDef::normal(0x30, "B"),
            KeyDef::normal(0x31, "N"),
            KeyDef::normal(0x32, "M"),
            KeyDef::normal(0x33, ","),
            KeyDef::normal(0x34, "."),
            KeyDef::normal(0x35, "/"),
            KeyDef::wide(0x36, "Shift", 2.75, OskKeyType::Modifier(ModifierKind::Shift)),
        ],
        // Row 4: spacebar row (7 keys)
        vec![
            KeyDef::wide(0x1D, "Ctrl", 1.5, OskKeyType::Modifier(ModifierKind::Ctrl)),
            KeyDef::wide(0x5B, "Super", 1.0, OskKeyType::Modifier(ModifierKind::Super)),
            KeyDef::wide(0x38, "Alt", 1.5, OskKeyType::Modifier(ModifierKind::Alt)),
            KeyDef::wide(0x39, " ", 6.0, OskKeyType::Space),
            KeyDef::wide(0xE038, "AltGr", 1.5, OskKeyType::Modifier(ModifierKind::AltGr)),
            KeyDef::wide(0x5C, "Super", 1.0, OskKeyType::Modifier(ModifierKind::Super)),
            KeyDef::wide(0xE01D, "Ctrl", 1.5, OskKeyType::Modifier(ModifierKind::Ctrl)),
        ],
    ]
}

/// Resolve the display label for a key from the keyboard layout.
///
/// For normal keys mapped in the layout, uses the uppercase normal character.
/// For unmapped or special keys, falls back to the default label.
fn resolve_label(layout: &KeyboardLayout, scancode: u32, default: &str, key_type: &OskKeyType) -> String {
    match key_type {
        OskKeyType::Normal => {
            if let Some(mapping) = layout.get(scancode) {
                let ch = mapping.shift.unwrap_or(mapping.normal);
                if ch.is_control() || ch == ' ' {
                    default.to_string()
                } else {
                    ch.to_string()
                }
            } else {
                default.to_string()
            }
        }
        _ => default.to_string(),
    }
}

/// Compute a complete on-screen keyboard layout with pixel positions.
///
/// The layout fills the given `width` x `height` rectangle. Keys are
/// positioned proportionally based on their grid unit widths. A small
/// gap (2px) is left between keys.
pub fn compute_osk_layout(layout: &KeyboardLayout, width: f32, height: f32) -> OskLayout {
    let row_defs = standard_rows();
    let num_rows = row_defs.len() as f32;
    let gap = 2.0_f32;
    let row_height = (height - gap * (num_rows - 1.0)) / num_rows;

    let mut rows = Vec::with_capacity(row_defs.len());

    for (row_idx, row_def) in row_defs.into_iter().enumerate() {
        let y = row_idx as f32 * (row_height + gap);

        // Total grid units in this row.
        let total_units: f32 = row_def.iter().map(|k| k.width_units).sum();
        let num_keys = row_def.len() as f32;
        let available_width = width - gap * (num_keys - 1.0);
        let unit_width = available_width / total_units;

        let mut x = 0.0_f32;
        let mut keys = Vec::with_capacity(row_def.len());

        for kd in &row_def {
            let key_w = kd.width_units * unit_width;
            let label = resolve_label(layout, kd.scancode, kd.default_label, &kd.key_type);

            keys.push(OskKey {
                scancode: kd.scancode,
                label,
                width_units: kd.width_units,
                key_type: kd.key_type.clone(),
                x,
                y,
                w: key_w,
                h: row_height,
            });

            x += key_w + gap;
        }

        rows.push(OskRow { keys });
    }

    OskLayout {
        rows,
        total_width: width,
        total_height: height,
    }
}
