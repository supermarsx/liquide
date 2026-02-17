//! CSS selector matching
//!
//! The `Selector` type stores the **raw CSS selector string** exactly as
//! serialized by lightningcss so that the downstream style engine can re-parse
//! it with its full `ComplexSelector` parser — preserving combinators,
//! attribute selectors, functional pseudo-classes (`:nth-child()`, `:not()`,
//! `:is()`, `:has()`, etc.) that were previously lost.
//!
//! For backward compatibility and simple theme-only matching the struct also
//! caches a decomposed single-compound representation.

use crate::error::{Result, ThemeError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// CSS selector.
///
/// `raw` stores the full serialized selector text — including combinators,
/// attribute selectors, and functional pseudo-classes — so the style engine
/// can re-parse it losslessly with its richer `ComplexSelector`.
///
/// The `element`, `classes`, `id`, `pseudo_classes`, and `pseudo_element`
/// fields are a best-effort decomposition of the **last compound** in the
/// selector chain for simple flat matching (used by `StyleSheet::compute_styles`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selector {
    /// The full, raw CSS selector string (e.g. `"div > p.intro:hover::before"`).
    /// This is the canonical representation passed to the style engine.
    pub raw: String,

    /// Element name from the last compound (e.g., "window", "button").
    pub element: String,

    /// Class names from the last compound.
    pub classes: Vec<String>,

    /// ID from the last compound.
    pub id: Option<String>,

    /// Pseudo-classes from the last compound (e.g., "hover", "focus", "active").
    pub pseudo_classes: Vec<String>,

    /// Pseudo-element (e.g., "before", "after", "placeholder").
    pub pseudo_element: Option<String>,
}

impl Selector {
    /// Create a simple element selector
    pub fn element(name: &str) -> Self {
        Self {
            raw: name.to_string(),
            element: name.to_string(),
            classes: Vec::new(),
            id: None,
            pseudo_classes: Vec::new(),
            pseudo_element: None,
        }
    }

    /// Add a class
    pub fn with_class(mut self, class: &str) -> Self {
        self.classes.push(class.to_string());
        self.rebuild_raw();
        self
    }

    /// Add an ID
    pub fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self.rebuild_raw();
        self
    }

    /// Add a pseudo-class
    pub fn with_pseudo_class(mut self, pseudo: &str) -> Self {
        self.pseudo_classes.push(pseudo.to_string());
        self.rebuild_raw();
        self
    }

    /// Rebuild the `raw` string from decomposed fields (for the builder API).
    fn rebuild_raw(&mut self) {
        let mut s = String::new();
        s.push_str(&self.element);
        if let Some(ref id) = self.id {
            s.push('#');
            s.push_str(id);
        }
        for class in &self.classes {
            s.push('.');
            s.push_str(class);
        }
        for pseudo in &self.pseudo_classes {
            s.push(':');
            s.push_str(pseudo);
        }
        if let Some(ref pe) = self.pseudo_element {
            s.push_str("::");
            s.push_str(pe);
        }
        self.raw = s;
    }

    /// Parse from CSS selector string.
    ///
    /// The full raw string is stored verbatim in `self.raw`.  For the
    /// decomposed fields we parse only the **last compound** in the chain
    /// (i.e. the subject of the selector) which is sufficient for the simple
    /// flat-matching API (`matches()`).  The style engine's `ComplexSelector`
    /// re-parses the full `raw` for DOM-aware matching.
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ThemeError::InvalidSelector(s.to_string()));
        }

        let raw = s.to_string();

        // Find the last compound selector: split on combinators (space, >, +, ~).
        // We want the rightmost simple/compound selector.
        let last_compound = Self::last_compound(s);

        // Decompose the last compound
        let (element, classes, id, pseudo_classes, pseudo_element) =
            Self::decompose_compound(last_compound);

        Ok(Self {
            raw,
            element,
            classes,
            id,
            pseudo_classes,
            pseudo_element,
        })
    }

    /// Construct a `Selector` from a raw CSS string that was already serialized
    /// by lightningcss.  This avoids re-parsing when we already have the string.
    pub fn from_raw(raw: String) -> Result<Self> {
        Self::parse(&raw)
    }

    /// Extract the last compound selector from a complex selector string.
    ///
    /// E.g. `"div > p.intro:hover"` → `"p.intro:hover"`.
    fn last_compound(s: &str) -> &str {
        let bytes = s.as_bytes();
        let mut depth: i32 = 0; // track parentheses for functional pseudo-classes
        let mut last_split = 0;
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'(' | b'[' => depth += 1,
                b')' | b']' => depth -= 1,
                b' ' | b'>' | b'+' | b'~' if depth == 0 => {
                    // Skip whitespace and combinators
                    let mut j = i + 1;
                    while j < bytes.len()
                        && (bytes[j] == b' '
                            || bytes[j] == b'>'
                            || bytes[j] == b'+'
                            || bytes[j] == b'~')
                    {
                        j += 1;
                    }
                    if j < bytes.len() {
                        last_split = j;
                    }
                    i = j;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        &s[last_split..]
    }

    /// Decompose a single compound selector string into (element, classes, id, pseudo_classes, pseudo_element).
    fn decompose_compound(
        s: &str,
    ) -> (
        String,
        Vec<String>,
        Option<String>,
        Vec<String>,
        Option<String>,
    ) {
        let mut element = String::new();
        let mut classes = Vec::new();
        let mut id = None;
        let mut pseudo_classes = Vec::new();
        let mut pseudo_element = None;

        let mut chars = s.chars().peekable();

        // Read element name
        while let Some(&ch) = chars.peek() {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == '*' {
                element.push(ch);
                chars.next();
            } else {
                break;
            }
        }

        if element.is_empty() {
            element = "*".to_string();
        }

        // Parse modifiers
        while let Some(ch) = chars.next() {
            match ch {
                '.' => {
                    let mut class = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                            class.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if !class.is_empty() {
                        classes.push(class);
                    }
                }
                '#' => {
                    let mut id_str = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                            id_str.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if !id_str.is_empty() {
                        id = Some(id_str);
                    }
                }
                ':' => {
                    if chars.peek() == Some(&':') {
                        // Pseudo-element
                        chars.next();
                        let mut pseudo = String::new();
                        while let Some(&ch) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '-' {
                                pseudo.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        if !pseudo.is_empty() {
                            pseudo_element = Some(pseudo);
                        }
                    } else {
                        // Pseudo-class (may have functional args)
                        let mut pseudo = String::new();
                        while let Some(&ch) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '-' {
                                pseudo.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        // Skip over functional arguments like :nth-child(2n+1)
                        if chars.peek() == Some(&'(') {
                            let mut depth = 0;
                            for ch in chars.by_ref() {
                                if ch == '(' {
                                    depth += 1;
                                }
                                if ch == ')' {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                            }
                        }
                        if !pseudo.is_empty() {
                            pseudo_classes.push(pseudo);
                        }
                    }
                }
                '[' => {
                    // Attribute selector — skip over it
                    for ch in chars.by_ref() {
                        if ch == ']' {
                            break;
                        }
                    }
                }
                _ if ch.is_whitespace() => continue,
                _ => { /* ignore unexpected chars in decomposition */ }
            }
        }

        (element, classes, id, pseudo_classes, pseudo_element)
    }

    /// Check if this selector matches an element with given properties
    pub fn matches(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> bool {
        // Check element
        if self.element != "*" && self.element != element {
            return false;
        }

        // Check ID
        if let Some(ref sel_id) = self.id {
            if id != Some(sel_id.as_str()) {
                return false;
            }
        }

        // Check classes (all must match)
        for class in &self.classes {
            if !classes.contains(class) {
                return false;
            }
        }

        // Check pseudo-classes (all must match)
        for pseudo in &self.pseudo_classes {
            if !pseudo_classes.contains(pseudo) {
                return false;
            }
        }

        true
    }

    /// Get specificity (for CSS cascade)
    /// Returns (id_count, class_count, element_count)
    pub fn specificity(&self) -> (u32, u32, u32) {
        let id_count = if self.id.is_some() { 1 } else { 0 };
        let class_count = (self.classes.len() + self.pseudo_classes.len()) as u32;
        let element_count = if self.element != "*" { 1 } else { 0 };

        (id_count, class_count, element_count)
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let sel = Selector::parse("button").unwrap();
        assert_eq!(sel.element, "button");
        assert!(sel.classes.is_empty());
        assert_eq!(sel.raw, "button");
    }

    #[test]
    fn test_parse_with_class() {
        let sel = Selector::parse("button.primary").unwrap();
        assert_eq!(sel.element, "button");
        assert_eq!(sel.classes, vec!["primary"]);
        assert_eq!(sel.raw, "button.primary");
    }

    #[test]
    fn test_parse_with_id() {
        let sel = Selector::parse("window#main").unwrap();
        assert_eq!(sel.element, "window");
        assert_eq!(sel.id, Some("main".to_string()));
    }

    #[test]
    fn test_parse_complex() {
        let sel = Selector::parse("button.primary.large:hover").unwrap();
        assert_eq!(sel.element, "button");
        assert_eq!(sel.classes, vec!["primary", "large"]);
        assert_eq!(sel.pseudo_classes, vec!["hover"]);
    }

    #[test]
    fn test_parse_descendant_combinator() {
        let sel = Selector::parse("div p.intro").unwrap();
        assert_eq!(sel.raw, "div p.intro");
        // Last compound is "p.intro"
        assert_eq!(sel.element, "p");
        assert_eq!(sel.classes, vec!["intro"]);
    }

    #[test]
    fn test_parse_child_combinator() {
        let sel = Selector::parse("ul > li.active").unwrap();
        assert_eq!(sel.raw, "ul > li.active");
        assert_eq!(sel.element, "li");
        assert_eq!(sel.classes, vec!["active"]);
    }

    #[test]
    fn test_parse_sibling_combinators() {
        let sel = Selector::parse("h1 + p").unwrap();
        assert_eq!(sel.raw, "h1 + p");
        assert_eq!(sel.element, "p");

        let sel2 = Selector::parse("h1 ~ p").unwrap();
        assert_eq!(sel2.raw, "h1 ~ p");
        assert_eq!(sel2.element, "p");
    }

    #[test]
    fn test_parse_attribute_selector() {
        let sel = Selector::parse("input[type=\"text\"]").unwrap();
        assert_eq!(sel.raw, "input[type=\"text\"]");
        assert_eq!(sel.element, "input");
    }

    #[test]
    fn test_parse_functional_pseudo_class() {
        let sel = Selector::parse("li:nth-child(2n+1)").unwrap();
        assert_eq!(sel.raw, "li:nth-child(2n+1)");
        assert_eq!(sel.element, "li");
        assert_eq!(sel.pseudo_classes, vec!["nth-child"]);
    }

    #[test]
    fn test_matches() {
        let sel = Selector::parse("button.primary:hover").unwrap();

        assert!(sel.matches(
            "button",
            &vec!["primary".to_string()],
            None,
            &vec!["hover".to_string()],
        ));

        assert!(!sel.matches(
            "button",
            &vec!["secondary".to_string()],
            None,
            &vec!["hover".to_string()],
        ));
    }

    #[test]
    fn test_specificity() {
        let sel1 = Selector::parse("button").unwrap();
        assert_eq!(sel1.specificity(), (0, 0, 1));

        let sel2 = Selector::parse("button.primary").unwrap();
        assert_eq!(sel2.specificity(), (0, 1, 1));

        let sel3 = Selector::parse("#main").unwrap();
        assert_eq!(sel3.specificity(), (1, 0, 0));
    }

    #[test]
    fn test_parse_pseudo_element() {
        let sel = Selector::parse("p::before").unwrap();
        assert_eq!(sel.raw, "p::before");
        assert_eq!(sel.element, "p");
        assert_eq!(sel.pseudo_element, Some("before".to_string()));
    }

    #[test]
    fn test_display_uses_raw() {
        let sel = Selector::parse("div > p.intro:hover").unwrap();
        assert_eq!(format!("{}", sel), "div > p.intro:hover");
    }
}
