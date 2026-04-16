//! X11 atom cache for efficient property lookups.

use std::collections::HashMap;

// Predefined atom values (matching X11 protocol standard atoms).
pub const WM_PROTOCOLS: u32 = 1;
pub const WM_DELETE_WINDOW: u32 = 2;
pub const WM_TAKE_FOCUS: u32 = 3;
pub const _NET_WM_NAME: u32 = 4;
pub const _NET_WM_WINDOW_TYPE: u32 = 5;
pub const _NET_WM_STATE: u32 = 6;
pub const _NET_WM_PID: u32 = 7;
pub const _MOTIF_WM_HINTS: u32 = 8;
pub const CLIPBOARD: u32 = 9;
pub const PRIMARY: u32 = 10;
pub const UTF8_STRING: u32 = 11;

/// Cache of X11 atoms for efficient bidirectional lookup.
#[derive(Debug)]
pub struct AtomCache {
    atoms: HashMap<String, u32>,
    reverse: HashMap<u32, String>,
}

impl AtomCache {
    /// Create a new atom cache pre-populated with standard atoms.
    pub fn new() -> Self {
        let mut cache = Self {
            atoms: HashMap::new(),
            reverse: HashMap::new(),
        };
        cache.intern("WM_PROTOCOLS".to_string(), WM_PROTOCOLS);
        cache.intern("WM_DELETE_WINDOW".to_string(), WM_DELETE_WINDOW);
        cache.intern("WM_TAKE_FOCUS".to_string(), WM_TAKE_FOCUS);
        cache.intern("_NET_WM_NAME".to_string(), _NET_WM_NAME);
        cache.intern("_NET_WM_WINDOW_TYPE".to_string(), _NET_WM_WINDOW_TYPE);
        cache.intern("_NET_WM_STATE".to_string(), _NET_WM_STATE);
        cache.intern("_NET_WM_PID".to_string(), _NET_WM_PID);
        cache.intern("_MOTIF_WM_HINTS".to_string(), _MOTIF_WM_HINTS);
        cache.intern("CLIPBOARD".to_string(), CLIPBOARD);
        cache.intern("PRIMARY".to_string(), PRIMARY);
        cache.intern("UTF8_STRING".to_string(), UTF8_STRING);
        cache
    }

    /// Look up an atom value by name.
    pub fn get(&self, name: &str) -> Option<u32> {
        self.atoms.get(name).copied()
    }

    /// Look up an atom name by value.
    pub fn name(&self, atom: u32) -> Option<&str> {
        self.reverse.get(&atom).map(|s| s.as_str())
    }

    /// Intern a new atom (store name ↔ value mapping).
    pub fn intern(&mut self, name: String, atom: u32) {
        self.reverse.insert(atom, name.clone());
        self.atoms.insert(name, atom);
    }
}

impl Default for AtomCache {
    fn default() -> Self {
        Self::new()
    }
}
