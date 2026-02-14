//! CSS property inheritance table.
//!
//! Defines which CSS properties inherit from parent to child by default.

/// Returns true if the named CSS property inherits by default.
pub fn is_inherited(property: &str) -> bool {
    matches!(
        property,
        // Typography
        "color"
            | "font-family"
            | "font-size"
            | "font-style"
            | "font-weight"
            | "font-variant"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-align"
            | "text-indent"
            | "text-transform"
            | "white-space"
            | "word-break"
            | "word-wrap"
            | "overflow-wrap"
            | "hyphens"
            | "tab-size"
            | "direction"
            | "unicode-bidi"
            // List
            | "list-style"
            | "list-style-type"
            | "list-style-position"
            | "list-style-image"
            // Visibility
            | "visibility"
            // Cursor
            | "cursor"
            // Quotation
            | "quotes"
            // Table
            | "border-collapse"
            | "border-spacing"
            | "caption-side"
            | "empty-cells"
            // Font features
            | "font-feature-settings"
            | "font-kerning"
            | "font-size-adjust"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_props() {
        assert!(is_inherited("color"));
        assert!(is_inherited("font-size"));
        assert!(is_inherited("cursor"));
        assert!(is_inherited("visibility"));
    }

    #[test]
    fn non_inherited_props() {
        assert!(!is_inherited("display"));
        assert!(!is_inherited("width"));
        assert!(!is_inherited("margin"));
        assert!(!is_inherited("padding"));
        assert!(!is_inherited("background"));
        assert!(!is_inherited("border"));
        assert!(!is_inherited("position"));
        assert!(!is_inherited("opacity"));
    }
}
