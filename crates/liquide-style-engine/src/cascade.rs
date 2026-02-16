//! Full CSS cascade implementation.
//!
//! Implements CSS Cascading and Inheritance Level 5:
//! - Origin (UA → User → Author)
//! - `!important` (reverses origin order)
//! - Specificity
//! - Source order
//! - Inline styles (highest author specificity)
//!
//! Cascade priority encoding — we encode priority as a
//! single comparable struct so sorting the cascade is a simple `Ord` comparison.

use crate::specificity::Specificity;

/// CSS cascade origin, from lowest to highest priority for normal declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CascadeOrigin {
    /// Browser default styles.
    UserAgent = 0,
    /// User-configured styles (accessibility, reader mode).
    User = 1,
    /// Author styles (theme CSS).
    Author = 2,
    /// Author styles marked inline (style attribute).
    AuthorInline = 3,
    /// Animation values (override normal author but not !important).
    Animation = 4,
    /// Transition values.
    Transition = 5,
}

/// Priority of a single CSS declaration in the cascade.
///
/// For `!important` declarations, origin ordering is inverted:
///   Author !important < User !important < UA !important
///
/// We encode this so that `CascadePriority` implements `Ord` and a simple sort
/// gives the correct cascade order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CascadePriority {
    /// Whether the declaration is `!important`.
    pub important: bool,
    /// The cascade origin.
    pub origin: CascadeOrigin,
    /// Layer order (0 = no layer, higher = later @layer).
    pub layer_order: u32,
    /// Specificity of the selector.
    pub specificity: Specificity,
    /// Source order among all declarations.
    pub source_order: u32,
    /// Whether this is an inline style.
    pub is_inline: bool,
}

impl CascadePriority {
    /// Create a normal (non-important) author declaration priority.
    pub fn author(specificity: Specificity, source_order: u32) -> Self {
        Self {
            important: false,
            origin: CascadeOrigin::Author,
            layer_order: 0,
            specificity,
            source_order,
            is_inline: false,
        }
    }

    /// Create a normal inline style priority (highest author specificity).
    pub fn inline(source_order: u32) -> Self {
        Self {
            important: false,
            origin: CascadeOrigin::AuthorInline,
            layer_order: 0,
            specificity: Specificity { id: 1, class: 0, type_sel: 0 },
            source_order,
            is_inline: true,
        }
    }

    /// Create an `!important` author declaration priority.
    pub fn author_important(specificity: Specificity, source_order: u32) -> Self {
        Self {
            important: true,
            origin: CascadeOrigin::Author,
            layer_order: 0,
            specificity,
            source_order,
            is_inline: false,
        }
    }

    /// Create a user-agent default priority.
    pub fn ua(source_order: u32) -> Self {
        Self {
            important: false,
            origin: CascadeOrigin::UserAgent,
            layer_order: 0,
            specificity: Specificity::ZERO,
            source_order,
            is_inline: false,
        }
    }

    /// Effective priority level for sorting.
    /// Normal: UA < User < Author < Inline < Animation < Transition
    /// Important: Author !imp < User !imp < UA !imp (reversed)
    fn effective_level(&self) -> u32 {
        if !self.important {
            // Normal declarations: 0..5 based on origin
            self.origin as u32
        } else {
            // Important declarations come after ALL normal, with reversed origin
            // Author !imp = 6, User !imp = 7, UA !imp = 8
            match self.origin {
                CascadeOrigin::Author | CascadeOrigin::AuthorInline => 6,
                CascadeOrigin::User => 7,
                CascadeOrigin::UserAgent => 8,
                CascadeOrigin::Animation => 9,
                CascadeOrigin::Transition => 10,
            }
        }
    }
}

impl Ord for CascadePriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.effective_level()
            .cmp(&other.effective_level())
            .then(self.layer_order.cmp(&other.layer_order))
            .then(self.is_inline.cmp(&other.is_inline))
            .then(self.specificity.cmp(&other.specificity))
            .then(self.source_order.cmp(&other.source_order))
    }
}

impl PartialOrd for CascadePriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A single declaration in the cascade.
#[derive(Debug, Clone)]
pub struct CascadeDeclaration {
    /// Property name (longhand only — shorthands must be expanded before entering).
    pub property: String,
    /// Property value.
    pub value: liquide_theme_css::value::PropertyValue,
    /// Priority in the cascade.
    pub priority: CascadePriority,
}

/// The cascade map — collects all declarations for an element, then resolves
/// the winner for each property.
pub struct CascadeMap {
    declarations: Vec<CascadeDeclaration>,
}

impl CascadeMap {
    pub fn new() -> Self {
        Self {
            declarations: Vec::new(),
        }
    }

    /// Add a declaration to the cascade.
    pub fn add(&mut self, decl: CascadeDeclaration) {
        self.declarations.push(decl);
    }

    /// Add multiple declarations from a property set with uniform priority.
    pub fn add_properties(
        &mut self,
        properties: &liquide_theme_css::property::PropertySet,
        priority: CascadePriority,
    ) {
        for (key, val) in properties.iter() {
            // Check for !important flag in the value
            let (actual_val, is_important) = strip_important(val);
            let actual_priority = if is_important {
                CascadePriority {
                    important: true,
                    ..priority
                }
            } else {
                priority
            };

            // Try shorthand expansion first
            if let Some(expanded) = crate::shorthand::expand_shorthand(key, &actual_val) {
                for (longhand, lh_val) in expanded {
                    self.declarations.push(CascadeDeclaration {
                        property: longhand.to_string(),
                        value: lh_val,
                        priority: actual_priority,
                    });
                }
            } else {
                self.declarations.push(CascadeDeclaration {
                    property: key.clone(),
                    value: actual_val,
                    priority: actual_priority,
                });
            }
        }
    }

    /// Resolve the cascade: for each property, pick the highest-priority
    /// declaration. Returns a map of property → value.
    pub fn resolve(
        &mut self,
    ) -> Vec<(String, liquide_theme_css::value::PropertyValue)> {
        // Sort all declarations by priority
        self.declarations
            .sort_by(|a, b| a.priority.cmp(&b.priority));

        // For each property, last one wins (highest priority due to sort order)
        let mut winners: std::collections::HashMap<String, &CascadeDeclaration> =
            std::collections::HashMap::new();

        for decl in &self.declarations {
            winners.insert(decl.property.clone(), decl);
        }

        winners
            .into_iter()
            .map(|(k, v)| (k, v.value.clone()))
            .collect()
    }

    /// Number of raw declarations.
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// Clear for reuse.
    pub fn clear(&mut self) {
        self.declarations.clear();
    }
}

impl Default for CascadeMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip `!important` annotation from a value if present.
/// Returns (cleaned value, is_important).
fn strip_important(
    value: &liquide_theme_css::value::PropertyValue,
) -> (liquide_theme_css::value::PropertyValue, bool) {
    // Check if the string representation ends with !important
    if let liquide_theme_css::value::PropertyValue::Keyword(kw) = value {
        if let Some(stripped) = kw.strip_suffix("!important") {
            let cleaned = stripped.trim();
            if cleaned.is_empty() {
                return (value.clone(), false);
            }
            return (
                liquide_theme_css::value::PropertyValue::Keyword(cleaned.to_string()),
                true,
            );
        }
    }
    (value.clone(), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_theme_css::value::PropertyValue;

    #[test]
    fn normal_cascade_order() {
        let ua = CascadePriority::ua(0);
        let author = CascadePriority::author(Specificity::ZERO, 1);
        let inline = CascadePriority::inline(2);
        assert!(ua < author);
        assert!(author < inline);
    }

    #[test]
    fn important_overrides_normal() {
        let normal_author = CascadePriority::author(
            Specificity { id: 1, class: 0, type_sel: 0 },
            100,
        );
        let important_author = CascadePriority::author_important(Specificity::ZERO, 0);
        assert!(normal_author < important_author);
    }

    #[test]
    fn specificity_within_origin() {
        let low = CascadePriority::author(Specificity { id: 0, class: 1, type_sel: 0 }, 0);
        let high = CascadePriority::author(Specificity { id: 1, class: 0, type_sel: 0 }, 0);
        assert!(low < high);
    }

    #[test]
    fn source_order_breaks_ties() {
        let first = CascadePriority::author(Specificity::ZERO, 0);
        let second = CascadePriority::author(Specificity::ZERO, 1);
        assert!(first < second);
    }

    #[test]
    fn cascade_map_resolve() {
        let mut map = CascadeMap::new();
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("red".into()),
            priority: CascadePriority::author(Specificity::ZERO, 0),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("blue".into()),
            priority: CascadePriority::author(
                Specificity { id: 0, class: 1, type_sel: 0 },
                1,
            ),
        });
        let resolved = map.resolve();
        let color = resolved.iter().find(|(k, _)| k == "color").unwrap();
        assert!(matches!(&color.1, PropertyValue::Keyword(kw) if kw == "blue"));
    }
}
