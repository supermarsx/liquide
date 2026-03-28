use std::sync::atomic::{AtomicU32, Ordering};

/// Monotonically-increasing counter for ClassAtom allocation.
/// Atom 0 is reserved as "invalid/null", so we start at 1.
static NEXT_ATOM: AtomicU32 = AtomicU32::new(1);

/// A unique, opaque identifier for a registered window class.
///
/// Atoms are never reused — the counter is monotonically increasing.
/// Atom(0) is reserved and never issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassAtom(u32);

impl ClassAtom {
    /// The null/invalid atom.
    pub const NULL: Self = Self(0);

    /// Allocate the next unique atom. This is thread-safe.
    pub(crate) fn next() -> Self {
        let v = NEXT_ATOM.fetch_add(1, Ordering::Relaxed);
        // In practice we will never exhaust u32 atoms, but guard anyway.
        assert!(v != 0, "ClassAtom counter wrapped around");
        Self(v)
    }

    /// Returns the raw `u32` value.
    #[inline]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Returns `true` if this is the null atom.
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Construct from a raw value — only for tests / deserialization.
    #[inline]
    pub const fn from_raw(v: u32) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for ClassAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClassAtom({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoms_are_unique() {
        let a = ClassAtom::next();
        let b = ClassAtom::next();
        assert_ne!(a, b);
        assert!(b.value() > a.value());
    }

    #[test]
    fn null_atom() {
        assert!(ClassAtom::NULL.is_null());
        assert!(!ClassAtom::next().is_null());
    }

    #[test]
    fn display_format() {
        let a = ClassAtom::from_raw(42);
        assert_eq!(format!("{a}"), "ClassAtom(42)");
    }
}
