//! Pseudo-class state flags for DOM nodes.

use bitflags::bitflags;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

bitflags! {
    /// CSS pseudo-class states tracked per DOM node.
    ///
    /// These are set by the event dispatcher (`:hover`, `:focus`, `:active`)
    /// or by application logic (`:disabled`, `:checked`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PseudoStateFlags: u32 {
        /// `:hover` — pointer is over this element.
        const HOVER            = 0x0001;
        /// `:focus` — element has keyboard focus.
        const FOCUS            = 0x0002;
        /// `:active` — element is being clicked/pressed.
        const ACTIVE           = 0x0004;
        /// `:visited` — link has been visited (rarely used in desktop).
        const VISITED          = 0x0008;
        /// `:disabled` — element is disabled.
        const DISABLED         = 0x0010;
        /// `:checked` — checkbox/radio is checked.
        const CHECKED          = 0x0020;
        /// `:first-child` — element is first child of its parent.
        const FIRST_CHILD      = 0x0040;
        /// `:last-child` — element is last child of its parent.
        const LAST_CHILD       = 0x0080;
        /// `:focus-within` — element or a descendant has focus.
        const FOCUS_WITHIN     = 0x0100;
        /// `:focus-visible` — focus visible (keyboard navigation).
        const FOCUS_VISIBLE    = 0x0200;
        /// `:placeholder-shown` — input placeholder is visible.
        const PLACEHOLDER_SHOWN = 0x0400;
        /// `:read-only` — element is not editable.
        const READ_ONLY        = 0x0800;
        /// `:read-write` — element is editable.
        const READ_WRITE       = 0x1000;
        /// `:empty` — element has no children.
        const EMPTY            = 0x2000;
        /// `:root` — element is the document root.
        const ROOT             = 0x4000;
        /// Internal: drag is in progress over this element.
        const DRAG_OVER        = 0x8000;
        /// `:target` — element is the current URL fragment target.
        const TARGET           = 0x10000;
        /// `:scope` — element is the scoping root (context element).
        const SCOPE            = 0x20000;
    }
}

impl Serialize for PseudoStateFlags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PseudoStateFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bits = u32::deserialize(deserializer)?;
        PseudoStateFlags::from_bits(bits)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid PseudoStateFlags bits: {bits:#x}")))
    }
}

impl PseudoStateFlags {
    /// Convert active flags to CSS pseudo-class name strings.
    pub fn to_css_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.contains(Self::HOVER) {
            names.push("hover");
        }
        if self.contains(Self::FOCUS) {
            names.push("focus");
        }
        if self.contains(Self::ACTIVE) {
            names.push("active");
        }
        if self.contains(Self::VISITED) {
            names.push("visited");
        }
        if self.contains(Self::DISABLED) {
            names.push("disabled");
        }
        if self.contains(Self::CHECKED) {
            names.push("checked");
        }
        if self.contains(Self::FIRST_CHILD) {
            names.push("first-child");
        }
        if self.contains(Self::LAST_CHILD) {
            names.push("last-child");
        }
        if self.contains(Self::FOCUS_WITHIN) {
            names.push("focus-within");
        }
        if self.contains(Self::FOCUS_VISIBLE) {
            names.push("focus-visible");
        }
        if self.contains(Self::PLACEHOLDER_SHOWN) {
            names.push("placeholder-shown");
        }
        if self.contains(Self::READ_ONLY) {
            names.push("read-only");
        }
        if self.contains(Self::READ_WRITE) {
            names.push("read-write");
        }
        if self.contains(Self::EMPTY) {
            names.push("empty");
        }
        if self.contains(Self::ROOT) {
            names.push("root");
        }
        if self.contains(Self::TARGET) {
            names.push("target");
        }
        if self.contains(Self::SCOPE) {
            names.push("scope");
        }
        names
    }

    /// Parse a CSS pseudo-class name to the corresponding flag.
    pub fn from_css_name(name: &str) -> Option<Self> {
        match name {
            "hover" => Some(Self::HOVER),
            "focus" => Some(Self::FOCUS),
            "active" => Some(Self::ACTIVE),
            "visited" => Some(Self::VISITED),
            "disabled" => Some(Self::DISABLED),
            "checked" => Some(Self::CHECKED),
            "first-child" => Some(Self::FIRST_CHILD),
            "last-child" => Some(Self::LAST_CHILD),
            "focus-within" => Some(Self::FOCUS_WITHIN),
            "focus-visible" => Some(Self::FOCUS_VISIBLE),
            "placeholder-shown" => Some(Self::PLACEHOLDER_SHOWN),
            "read-only" => Some(Self::READ_ONLY),
            "read-write" => Some(Self::READ_WRITE),
            "empty" => Some(Self::EMPTY),
            "root" => Some(Self::ROOT),
            "target" => Some(Self::TARGET),
            "scope" => Some(Self::SCOPE),
            _ => None,
        }
    }
}

impl Default for PseudoStateFlags {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_css_names_round_trip() {
        let flags = PseudoStateFlags::HOVER | PseudoStateFlags::FOCUS;
        let names = flags.to_css_names();
        assert!(names.contains(&"hover"));
        assert!(names.contains(&"focus"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn from_css_name_works() {
        assert_eq!(
            PseudoStateFlags::from_css_name("disabled"),
            Some(PseudoStateFlags::DISABLED)
        );
        assert_eq!(PseudoStateFlags::from_css_name("unknown"), None);
    }
}
