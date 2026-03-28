/// Bitflag-style class style constants for window class registration.
///
/// Rather than pulling in the `bitflags` crate we define a simple newtype
/// wrapper around `u32` with named constants and bitwise operator impls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ClassStyle(u32);

impl ClassStyle {
    /// No style flags set.
    pub const NONE: Self = Self(0);
    /// Redraw entire window when the vertical size changes.
    pub const VREDRAW: Self = Self(0x0001);
    /// Redraw entire window when the horizontal size changes.
    pub const HREDRAW: Self = Self(0x0002);
    /// Window receives double-click messages.
    pub const DBLCLKS: Self = Self(0x0008);
    /// Each window instance gets its own device context.
    pub const OWNDC: Self = Self(0x0020);
    /// All windows of this class share a single device context.
    pub const CLASSDC: Self = Self(0x0040);
    /// Use parent window's device context.
    pub const PARENTDC: Self = Self(0x0080);
    /// Disable the Close command on the system menu.
    pub const NOCLOSE: Self = Self(0x0200);
    /// Save the screen bitmap obscured by the window so it can be restored
    /// without sending `WM_PAINT` to underlying windows.
    pub const SAVEBITS: Self = Self(0x0800);
    /// Class is visible to all modules, not just the registering one.
    pub const GLOBALCLASS: Self = Self(0x4000);
    /// The window has a drop shadow effect.
    pub const DROPSHADOW: Self = Self(0x0002_0000);

    /// Raw bits.
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Construct from raw bits (unchecked).
    #[inline]
    pub const fn from_bits_unchecked(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns `true` if `other` is a subset of `self`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` if no bits are set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl core::ops::BitOr for ClassStyle {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for ClassStyle {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for ClassStyle {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::BitAndAssign for ClassStyle {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl core::ops::Not for ClassStyle {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_styles() {
        let s = ClassStyle::HREDRAW | ClassStyle::VREDRAW;
        assert!(s.contains(ClassStyle::HREDRAW));
        assert!(s.contains(ClassStyle::VREDRAW));
        assert!(!s.contains(ClassStyle::DBLCLKS));
    }

    #[test]
    fn default_is_none() {
        assert_eq!(ClassStyle::default(), ClassStyle::NONE);
        assert!(ClassStyle::default().is_empty());
    }

    #[test]
    fn bits_roundtrip() {
        let s = ClassStyle::OWNDC | ClassStyle::GLOBALCLASS;
        let bits = s.bits();
        assert_eq!(ClassStyle::from_bits_unchecked(bits), s);
    }

    #[test]
    fn not_operator() {
        let s = ClassStyle::DBLCLKS;
        let inv = !s;
        assert!(!inv.contains(ClassStyle::DBLCLKS));
        assert!(inv.contains(ClassStyle::VREDRAW));
    }
}
