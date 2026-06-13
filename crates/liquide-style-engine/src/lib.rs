//! # liquide-style-engine
//!
//! CSS style computation for the LiquiDE rendering pipeline.
//!
//! Takes a [`liquide_dom::Document`] + stylesheets and computes a
//! [`ComputedStyle`] per element using cascade, specificity, inheritance,
//! CSS variables, and media queries.

pub mod cascade;
pub mod computed;
pub mod dimension;
pub mod engine;
pub mod impact;
pub mod inheritance;
pub mod rules;
pub mod selector;
pub mod shadow_dom;
pub mod shorthand;
pub mod specificity;
pub mod style_map;
pub mod transition;
pub mod value_resolve;

pub use cascade::{CascadeMap, CascadeOrigin, CascadePriority};
pub use computed::ComputedStyle;
pub use dimension::Dimension;
pub use engine::EnvironmentValues;
pub use engine::RestyleResult;
pub use engine::StyleEngine;
pub use impact::{
    StyleChangeImpact, StyleDiffSummary, StylePropertyChange, classify_style_property,
    conservative_style_impact,
};
pub use rules::{ElementRule, RuleEngine, Severity, ValidationReport, Violation};
pub use selector::{Combinator, ComplexSelector, CompoundSelector, PseudoClassSelector};
pub use shorthand::expand_shorthand;
pub use specificity::Specificity;
pub use style_map::{PseudoKind, StyleMap};
