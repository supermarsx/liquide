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
            specificity: Specificity {
                id: 1,
                class: 0,
                type_sel: 0,
            },
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

    /// Create a normal user-origin declaration priority.
    pub fn user(specificity: Specificity, source_order: u32) -> Self {
        Self {
            important: false,
            origin: CascadeOrigin::User,
            layer_order: 0,
            specificity,
            source_order,
            is_inline: false,
        }
    }

    /// Create an animation-origin priority (overrides normal author).
    pub fn animation(source_order: u32) -> Self {
        Self {
            important: false,
            origin: CascadeOrigin::Animation,
            layer_order: 0,
            specificity: Specificity::ZERO,
            source_order,
            is_inline: false,
        }
    }

    /// Create a transition-origin priority (highest normal priority).
    pub fn transition(source_order: u32) -> Self {
        Self {
            important: false,
            origin: CascadeOrigin::Transition,
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
            .then(if self.important {
                // CSS Cascading Level 5 §6.4.4: For !important declarations,
                // earlier layers (lower layer_order) win — reverse the order.
                other.layer_order.cmp(&self.layer_order)
            } else {
                // Normal declarations: later layers win — ascending order.
                self.layer_order.cmp(&other.layer_order)
            })
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
            // Check for !important: first via the PropertySet flag (set by the
            // parser for structured types like Number, Color, Length, etc.),
            // then via text-based detection for inline-style fallback values.
            let (actual_val, text_important) = strip_important(val);
            let is_important = text_important || properties.is_important(key);
            let actual_priority = if is_important {
                CascadePriority {
                    important: true,
                    ..priority
                }
            } else {
                priority
            };

            // A shorthand whose value still contains `var()` (or `env()`)
            // CANNOT be expanded here: the substitution is unknown at cascade
            // collection time, so the shorthand parser cannot classify the
            // unresolved token (e.g. `background: var(--accent)` looks like
            // neither a color nor an image and would expand to NOTHING, silently
            // dropping the fill). Per CSS Variables L1 §3, such a declaration is
            // a "pending-substitution value": keep the shorthand intact so the
            // var is resolved and the value re-parsed into longhands at apply
            // time (`apply_single_property`), matching `background-color: var()`.
            let value_has_var = actual_val
                .as_string()
                .map(|text| text.contains("var(") || text.contains("env("))
                .unwrap_or(false);

            // Try shorthand expansion first (unless deferred for var/env above).
            if let Some(expanded) = if value_has_var {
                None
            } else {
                crate::shorthand::expand_shorthand(key, &actual_val)
            } {
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
    ///
    /// Sorts by (property name, priority) so same-property declarations are
    /// grouped, then takes the last (highest-priority) entry per group.
    /// Moves strings and values out of the declarations vec instead of cloning.
    ///
    /// CSS Cascading Level 5: `revert` and `revert-layer` keywords are resolved
    /// within the cascade. `revert` falls back to the winning value from a
    /// lower origin class (Author → User → UA). `revert-layer` falls back
    /// within the same origin to a previous `@layer`.
    pub fn resolve(&mut self) -> Vec<(String, liquide_theme_css::value::PropertyValue)> {
        if self.declarations.is_empty() {
            return Vec::new();
        }

        // Sort by property name first (grouping), then by priority within
        // each group so the last entry in each run is the winner.
        self.declarations.sort_by(|a, b| {
            a.property
                .cmp(&b.property)
                .then(a.priority.cmp(&b.priority))
        });

        let len = self.declarations.len();

        // First pass: determine the winning index for each property group,
        // resolving `revert` and `revert-layer` by falling back within the group.
        let mut winners: Vec<usize> = Vec::new();
        let mut i = 0;
        while i < len {
            let mut j = i + 1;
            while j < len && self.declarations[j].property == self.declarations[i].property {
                j += 1;
            }
            let winner_idx = resolve_winner_in_group(&self.declarations, i, j);
            winners.push(winner_idx);
            i = j;
        }

        // Second pass: apply winning declarations in cascade-priority order
        // rather than property-name order. This preserves shorthand reset
        // behavior such as `all`, which must run after lower-priority
        // longhands and before higher-priority overrides.
        winners.sort_by(|left, right| {
            self.declarations[*left]
                .priority
                .cmp(&self.declarations[*right].priority)
        });

        // Third pass: extract values from winning declarations.
        let mut result = Vec::with_capacity(winners.len());
        for winner_idx in winners {
            let decl = &mut self.declarations[winner_idx];
            let property = std::mem::take(&mut decl.property);
            let value = std::mem::replace(
                &mut decl.value,
                liquide_theme_css::value::PropertyValue::Keyword(String::new()),
            );
            result.push((property, value));
        }
        result
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

    /// Resolve the cascade per origin: for each property, return the winning
    /// value at each cascade origin class (UA, User, Author).
    ///
    /// This is a read-only operation (clones declarations internally) and is
    /// useful for inspecting what each origin contributes before revert
    /// resolution collapses them.
    pub fn resolve_per_origin(
        &self,
    ) -> std::collections::HashMap<
        String,
        Vec<(CascadeOrigin, liquide_theme_css::value::PropertyValue)>,
    > {
        let mut sorted = self.declarations.clone();
        sorted.sort_by(|a, b| {
            a.property
                .cmp(&b.property)
                .then(a.priority.cmp(&b.priority))
        });

        let mut result = std::collections::HashMap::new();
        let len = sorted.len();
        let mut i = 0;
        while i < len {
            let mut j = i + 1;
            while j < len && sorted[j].property == sorted[i].property {
                j += 1;
            }

            let property = sorted[i].property.clone();
            let mut origin_winners: Vec<(CascadeOrigin, liquide_theme_css::value::PropertyValue)> =
                Vec::new();

            // Representative origins for each class
            for target_class in 0u8..=2 {
                let mut best: Option<usize> = None;
                for k in i..j {
                    if cascade_origin_class(sorted[k].priority.origin) == target_class {
                        best = Some(k);
                    }
                }
                if let Some(b) = best {
                    let origin = match target_class {
                        0 => CascadeOrigin::UserAgent,
                        1 => CascadeOrigin::User,
                        _ => CascadeOrigin::Author,
                    };
                    origin_winners.push((origin, sorted[b].value.clone()));
                }
            }

            result.insert(property, origin_winners);
            i = j;
        }

        result
    }
}

impl Default for CascadeMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Classify a [`CascadeOrigin`] into one of the three CSS origin classes:
/// UA (0), User (1), Author (2). Used for `revert` resolution.
fn cascade_origin_class(origin: CascadeOrigin) -> u8 {
    match origin {
        CascadeOrigin::UserAgent => 0,
        CascadeOrigin::User => 1,
        // Author, AuthorInline, Animation, and Transition all belong to the author origin class.
        CascadeOrigin::Author
        | CascadeOrigin::AuthorInline
        | CascadeOrigin::Animation
        | CascadeOrigin::Transition => 2,
    }
}

/// Given a sorted group of declarations for a single property (indices `[start, end)`),
/// determine the winning index after resolving `revert` and `revert-layer` keywords.
///
/// Declarations are sorted by ascending cascade priority, so the last entry is the
/// initial winner. If it is `revert`, we fall back to the best declaration from a
/// lower origin class; if `revert-layer`, we fall back within the same origin to a
/// lower (for normal) or higher (for `!important`) layer.
fn resolve_winner_in_group(declarations: &[CascadeDeclaration], start: usize, end: usize) -> usize {
    let mut idx = end - 1;
    // Guard against infinite loops from malformed/chained reverts.
    let max_iterations = end - start;
    let mut iterations = 0;

    loop {
        if iterations >= max_iterations {
            return idx;
        }
        iterations += 1;

        let is_revert = match &declarations[idx].value {
            liquide_theme_css::value::PropertyValue::Keyword(kw) => match kw.as_str() {
                "revert" => Some(true),
                "revert-layer" => Some(false),
                _ => None,
            },
            _ => None,
        };

        match is_revert {
            Some(true) => {
                // `revert`: roll back to a lower origin class.
                let winner_class = cascade_origin_class(declarations[idx].priority.origin);
                let mut found = false;
                for k in (start..idx).rev() {
                    if cascade_origin_class(declarations[k].priority.origin) < winner_class {
                        idx = k;
                        found = true;
                        break;
                    }
                }
                if !found {
                    // No lower origin — keyword stays; apply_single_property
                    // handles it with `unset` semantics (initial/inherit).
                    return idx;
                }
                // Continue: the fallback itself might be revert/revert-layer.
            }
            Some(false) => {
                // `revert-layer`: roll back within the same origin to a previous layer.
                let winner = &declarations[idx].priority;
                let winner_origin = winner.origin;
                let winner_layer = winner.layer_order;
                let is_important = winner.important;
                let mut found = false;
                for k in (start..idx).rev() {
                    let p = &declarations[k].priority;
                    if cascade_origin_class(p.origin) == cascade_origin_class(winner_origin) {
                        let layer_ok = if is_important {
                            // For !important, earlier layers (lower order) win,
                            // so "previous" layer = higher layer_order.
                            p.layer_order > winner_layer
                        } else {
                            // For normal, later layers win,
                            // so "previous" layer = lower layer_order.
                            p.layer_order < winner_layer
                        };
                        if layer_ok {
                            idx = k;
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    // No lower layer within the same origin. Per CSS Cascade 5,
                    // `revert-layer` then continues by falling back across
                    // lower origins, just like `revert`.
                    let winner_class = cascade_origin_class(winner_origin);
                    for k in (start..idx).rev() {
                        if cascade_origin_class(declarations[k].priority.origin) < winner_class {
                            idx = k;
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    return idx;
                }
            }
            None => return idx,
        }
    }
}

/// Strip `!important` annotation from a value if present.
/// Returns (cleaned value, is_important).
///
/// Only allocates when the value actually contains `!important`;
/// non-important values (the vast majority) are cloned once in the caller
/// when pushed into `CascadeDeclaration`.
fn strip_important(
    value: &liquide_theme_css::value::PropertyValue,
) -> (liquide_theme_css::value::PropertyValue, bool) {
    match value {
        liquide_theme_css::value::PropertyValue::Keyword(kw) => {
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
        liquide_theme_css::value::PropertyValue::String(s) => {
            if let Some(stripped) = s.strip_suffix("!important") {
                let cleaned = stripped.trim();
                if cleaned.is_empty() {
                    return (value.clone(), false);
                }
                return (
                    liquide_theme_css::value::PropertyValue::String(cleaned.to_string()),
                    true,
                );
            }
        }
        _ => {}
    }
    // Not important — return a clone. The caller needs an owned value for
    // CascadeDeclaration anyway, so this clone is unavoidable.
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
            Specificity {
                id: 1,
                class: 0,
                type_sel: 0,
            },
            100,
        );
        let important_author = CascadePriority::author_important(Specificity::ZERO, 0);
        assert!(normal_author < important_author);
    }

    #[test]
    fn specificity_within_origin() {
        let low = CascadePriority::author(
            Specificity {
                id: 0,
                class: 1,
                type_sel: 0,
            },
            0,
        );
        let high = CascadePriority::author(
            Specificity {
                id: 1,
                class: 0,
                type_sel: 0,
            },
            0,
        );
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
                Specificity {
                    id: 0,
                    class: 1,
                    type_sel: 0,
                },
                1,
            ),
        });
        let resolved = map.resolve();
        let color = resolved.iter().find(|(k, _)| k == "color").unwrap();
        assert!(matches!(&color.1, PropertyValue::Keyword(kw) if kw == "blue"));
    }

    #[test]
    fn important_number_beats_higher_specificity_normal() {
        // A Number(100.0) with !important (via PropertySet flag) should beat
        // Number(200.0) at higher specificity without !important.
        let mut props_important = liquide_theme_css::property::PropertySet::new();
        props_important.insert("opacity".into(), PropertyValue::Number(100.0));
        props_important.mark_important("opacity");

        let mut props_normal = liquide_theme_css::property::PropertySet::new();
        props_normal.insert("opacity".into(), PropertyValue::Number(200.0));

        let mut map = CascadeMap::new();
        // Important at low specificity
        map.add_properties(
            &props_important,
            CascadePriority::author(Specificity::ZERO, 0),
        );
        // Normal at high specificity
        map.add_properties(
            &props_normal,
            CascadePriority::author(
                Specificity {
                    id: 1,
                    class: 0,
                    type_sel: 0,
                },
                1,
            ),
        );

        let resolved = map.resolve();
        let opacity = resolved.iter().find(|(k, _)| k == "opacity").unwrap();
        // The important value (100.0) should win despite lower specificity
        assert!(
            matches!(&opacity.1, PropertyValue::Number(n) if (*n - 100.0).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn important_color_via_property_set_flag() {
        use liquide_theme_css::value::Color;

        let mut props_important = liquide_theme_css::property::PropertySet::new();
        props_important.insert(
            "background-color".into(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );
        props_important.mark_important("background-color");

        let mut props_normal = liquide_theme_css::property::PropertySet::new();
        props_normal.insert(
            "background-color".into(),
            PropertyValue::Color(Color::rgb(0, 0, 255)),
        );

        let mut map = CascadeMap::new();
        map.add_properties(
            &props_important,
            CascadePriority::author(Specificity::ZERO, 0),
        );
        map.add_properties(
            &props_normal,
            CascadePriority::author(
                Specificity {
                    id: 1,
                    class: 0,
                    type_sel: 0,
                },
                1,
            ),
        );

        let resolved = map.resolve();
        let bg = resolved
            .iter()
            .find(|(k, _)| k == "background-color")
            .unwrap();
        // Important red should win over normal blue at higher specificity
        assert!(matches!(&bg.1, PropertyValue::Color(c) if c.r == 255 && c.b == 0));
    }

    #[test]
    fn revert_falls_back_to_user_origin() {
        // Author says `revert` → should fall back to User value.
        let mut map = CascadeMap::new();
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("ua-red".into()),
            priority: CascadePriority::ua(0),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("user-green".into()),
            priority: CascadePriority::user(Specificity::ZERO, 1),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("revert".into()),
            priority: CascadePriority::author(Specificity::ZERO, 2),
        });
        let resolved = map.resolve();
        let color = resolved.iter().find(|(k, _)| k == "color").unwrap();
        assert!(
            matches!(&color.1, PropertyValue::Keyword(kw) if kw == "user-green"),
            "revert from Author should fall back to User value, got {:?}",
            color.1
        );
    }

    #[test]
    fn revert_falls_back_to_ua_when_no_user() {
        // Author says `revert`, no User declarations → fall back to UA.
        let mut map = CascadeMap::new();
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("ua-default".into()),
            priority: CascadePriority::ua(0),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("revert".into()),
            priority: CascadePriority::author(Specificity::ZERO, 1),
        });
        let resolved = map.resolve();
        let color = resolved.iter().find(|(k, _)| k == "color").unwrap();
        assert!(
            matches!(&color.1, PropertyValue::Keyword(kw) if kw == "ua-default"),
            "revert from Author with no User should fall back to UA, got {:?}",
            color.1
        );
    }

    #[test]
    fn revert_user_falls_back_to_ua() {
        // User says `revert` → fall back to UA.
        let mut map = CascadeMap::new();
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("ua-blue".into()),
            priority: CascadePriority::ua(0),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("revert".into()),
            priority: CascadePriority::user(Specificity::ZERO, 1),
        });
        let resolved = map.resolve();
        let color = resolved.iter().find(|(k, _)| k == "color").unwrap();
        assert!(
            matches!(&color.1, PropertyValue::Keyword(kw) if kw == "ua-blue"),
            "revert from User should fall back to UA, got {:?}",
            color.1
        );
    }

    #[test]
    fn revert_layer_falls_back_to_previous_layer() {
        // Author layer 2 says `revert-layer` → fall back to Author layer 1.
        let mut map = CascadeMap::new();
        let mut p1 = CascadePriority::author(Specificity::ZERO, 0);
        p1.layer_order = 1;
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("layer1-blue".into()),
            priority: p1,
        });
        let mut p2 = CascadePriority::author(Specificity::ZERO, 1);
        p2.layer_order = 2;
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("revert-layer".into()),
            priority: p2,
        });
        let resolved = map.resolve();
        let color = resolved.iter().find(|(k, _)| k == "color").unwrap();
        assert!(
            matches!(&color.1, PropertyValue::Keyword(kw) if kw == "layer1-blue"),
            "revert-layer should fall back to previous layer, got {:?}",
            color.1
        );
    }

    #[test]
    fn revert_layer_falls_back_across_lower_origins_when_no_lower_layer_exists() {
        let mut map = CascadeMap::new();
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("ua-red".into()),
            priority: CascadePriority::ua(0),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("user-green".into()),
            priority: CascadePriority::user(Specificity::ZERO, 1),
        });
        let mut author = CascadePriority::author(Specificity::ZERO, 2);
        author.layer_order = 3;
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("revert-layer".into()),
            priority: author,
        });

        let resolved = map.resolve();
        let color = resolved.iter().find(|(key, _)| key == "color").unwrap();
        assert!(
            matches!(&color.1, PropertyValue::Keyword(kw) if kw == "user-green"),
            "revert-layer should fall back across lower origins when no lower layer exists, got {:?}",
            color.1
        );
    }

    #[test]
    fn chained_revert_resolves() {
        // Author `revert` → User `revert` → UA value.
        let mut map = CascadeMap::new();
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("ua-final".into()),
            priority: CascadePriority::ua(0),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("revert".into()),
            priority: CascadePriority::user(Specificity::ZERO, 1),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("revert".into()),
            priority: CascadePriority::author(Specificity::ZERO, 2),
        });
        let resolved = map.resolve();
        let color = resolved.iter().find(|(k, _)| k == "color").unwrap();
        assert!(
            matches!(&color.1, PropertyValue::Keyword(kw) if kw == "ua-final"),
            "chained revert should resolve to UA, got {:?}",
            color.1
        );
    }

    #[test]
    fn resolve_per_origin_returns_all_origins() {
        let mut map = CascadeMap::new();
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("ua-val".into()),
            priority: CascadePriority::ua(0),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("user-val".into()),
            priority: CascadePriority::user(Specificity::ZERO, 1),
        });
        map.add(CascadeDeclaration {
            property: "color".into(),
            value: PropertyValue::Keyword("author-val".into()),
            priority: CascadePriority::author(Specificity::ZERO, 2),
        });
        let per_origin = map.resolve_per_origin();
        let color = per_origin.get("color").expect("color should be present");
        assert_eq!(color.len(), 3, "should have UA, User, and Author entries");
        assert!(
            matches!(&color[0], (CascadeOrigin::UserAgent, PropertyValue::Keyword(kw)) if kw == "ua-val")
        );
        assert!(
            matches!(&color[1], (CascadeOrigin::User, PropertyValue::Keyword(kw)) if kw == "user-val")
        );
        assert!(
            matches!(&color[2], (CascadeOrigin::Author, PropertyValue::Keyword(kw)) if kw == "author-val")
        );
    }
}
