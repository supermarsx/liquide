//! String interning via an atom table for efficient string deduplication.
//!
//! Each string is assigned a unique [`Atom`] (u32). Atoms are reference-counted:
//! calling [`AtomTable::add`] for a string that already exists increments its
//! refcount and returns the existing atom. [`AtomTable::delete`] decrements the
//! refcount, and the entry is removed when it reaches zero.

use crate::types::Atom;
use std::collections::HashMap;

/// A reference-counted entry in the atom table.
#[derive(Debug, Clone)]
struct AtomEntry {
    name: String,
    refcount: u32,
}

/// String interning table mapping strings to unique [`Atom`] IDs.
///
/// Pre-registered atoms for system class names can be added at construction
/// time via [`AtomTable::with_system_classes`].
#[derive(Debug, Clone)]
pub struct AtomTable {
    /// Forward map: atom -> entry (name + refcount).
    entries: HashMap<u32, AtomEntry>,
    /// Reverse map: name -> atom value.
    name_to_atom: HashMap<String, u32>,
    /// Next atom value to assign.
    next_atom: u32,
}

/// Atom value 0 is reserved (invalid/null atom).
const FIRST_USER_ATOM: u32 = 0xC000;
/// System atoms start at 1 and go up to FIRST_USER_ATOM - 1.
const FIRST_SYSTEM_ATOM: u32 = 1;

impl AtomTable {
    /// Creates an empty atom table.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            name_to_atom: HashMap::new(),
            next_atom: FIRST_USER_ATOM,
        }
    }

    /// Creates an atom table pre-populated with system class name atoms.
    ///
    /// System atoms are assigned IDs starting at 1, below the user atom range.
    /// They have a refcount of `u32::MAX` so they can never be deleted.
    pub fn with_system_classes(classes: &[&str]) -> Self {
        let mut table = Self::new();
        let mut sys_atom = FIRST_SYSTEM_ATOM;
        for &class in classes {
            let entry = AtomEntry {
                name: class.to_string(),
                refcount: u32::MAX, // immortal
            };
            table.entries.insert(sys_atom, entry);
            table.name_to_atom.insert(class.to_string(), sys_atom);
            sys_atom += 1;
        }
        table
    }

    /// Interns a string, returning its atom. If the string is already interned,
    /// increments its refcount and returns the existing atom.
    pub fn add(&mut self, name: &str) -> Atom {
        if let Some(&atom_val) = self.name_to_atom.get(name) {
            let entry = self.entries.get_mut(&atom_val).expect("atom table inconsistent");
            // Don't overflow immortal system atoms.
            if entry.refcount < u32::MAX {
                entry.refcount += 1;
            }
            return Atom(atom_val);
        }

        let atom_val = self.next_atom;
        self.next_atom += 1;

        let entry = AtomEntry {
            name: name.to_string(),
            refcount: 1,
        };
        self.entries.insert(atom_val, entry);
        self.name_to_atom.insert(name.to_string(), atom_val);
        Atom(atom_val)
    }

    /// Looks up an atom by name without creating it.
    pub fn find(&self, name: &str) -> Option<Atom> {
        self.name_to_atom.get(name).map(|&v| Atom(v))
    }

    /// Reverse lookup: get the string name for an atom.
    pub fn get_name(&self, atom: Atom) -> Option<&str> {
        self.entries.get(&atom.0).map(|e| e.name.as_str())
    }

    /// Returns the current reference count of an atom, or `None` if it doesn't exist.
    pub fn refcount(&self, atom: Atom) -> Option<u32> {
        self.entries.get(&atom.0).map(|e| e.refcount)
    }

    /// Decrements the reference count of an atom. If the refcount reaches zero,
    /// the atom is removed from the table. System atoms (refcount = `u32::MAX`)
    /// are never deleted.
    pub fn delete(&mut self, atom: Atom) {
        let Some(entry) = self.entries.get_mut(&atom.0) else {
            return;
        };

        // System atoms are immortal.
        if entry.refcount == u32::MAX {
            return;
        }

        entry.refcount = entry.refcount.saturating_sub(1);
        if entry.refcount == 0 {
            let name = entry.name.clone();
            self.entries.remove(&atom.0);
            self.name_to_atom.remove(&name);
        }
    }

    /// Returns the number of atoms currently in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the table contains no atoms.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AtomTable {
    fn default() -> Self {
        Self::new()
    }
}
