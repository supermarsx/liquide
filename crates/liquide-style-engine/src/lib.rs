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
pub mod inheritance;
pub mod rules;
pub mod selector;
pub mod shorthand;
pub mod specificity;
pub mod style_map;
pub mod value_resolve;

pub use cascade::{CascadeMap, CascadeOrigin, CascadePriority};
pub use computed::ComputedStyle;
pub use dimension::Dimension;
pub use engine::StyleEngine;
pub use rules::{ElementRule, RuleEngine, Severity, ValidationReport, Violation};
pub use selector::{Combinator, ComplexSelector, CompoundSelector, PseudoClassSelector};
pub use shorthand::expand_shorthand;
pub use specificity::Specificity;
pub use style_map::{PseudoKind, StyleMap};
