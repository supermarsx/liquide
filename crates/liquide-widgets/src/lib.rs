//! # liquide-widgets
//!
//! Reusable, CSS-styled, interactive widget toolkit rendered through the DOM+CSS
//! pipeline — the t98 P7 "all GUI elements" answer. It **generalizes** the
//! proven chrome authoring pattern (`Component` -> `TemplateNode` ->
//! `TemplateRenderer`, keyed reconciliation + pseudo-state patching) from
//! [`liquide_components`] into a shared widget infrastructure.
//!
//! This crate is **S0 (shared infrastructure)**: it provides the foundation that
//! every individual widget (Groups A-D) will build on, plus ONE reference widget
//! ([`ReferenceBox`]) that validates the infrastructure end-to-end. No
//! individual widget families ship here yet.
//!
//! ## The two halves of a widget
//!
//! - **Appearance** — a [`Component`] emitting a `<lq-*>` [`TemplateNode`]
//!   subtree styled purely in CSS (`assets/themes/widgets.css` + per-widget
//!   sections), with `pseudo_if(...)` for `:hover`/`:active`/`:focus`/
//!   `:checked`/`:disabled`.
//! - **Behavior** — a [`WidgetBehavior`]: owns runtime state, consumes
//!   [`DomEvent`]s + keyboard, emits [`WidgetOutcome`]s; the [`WidgetHost`]
//!   re-renders it through [`TemplateRenderer`] so changes reconcile into the
//!   live DOM.
//!
//! ## The anti-constant guard
//!
//! All interaction reads hit geometry through [`LayoutQuery`] — the laid-out CSS
//! box — never a hardcoded constant. This is the structural defense against the
//! menu-hit-test-mismatch bug class. The S0 gallery harness drives the REAL
//! pipeline + real event dispatcher so a constant-based widget cannot pass.
//!
//! [`Component`]: liquide_components::template::Component
//! [`TemplateNode`]: liquide_components::template::TemplateNode
//! [`TemplateRenderer`]: liquide_components::template::TemplateRenderer
//! [`DomEvent`]: liquide_hit_test::event::DomEvent

pub mod behavior;
pub mod focus;
pub mod host;
pub mod layout_query;
pub mod reference;

// The real-pipeline gallery test harness (depends on dev-deps; test-only).
#[cfg(test)]
mod gallery;

// End-to-end infra-validation tests for the reference widget.
#[cfg(test)]
mod gallery_tests;

// ── Public API surface ────────────────────────────────────────────────────

pub use behavior::{KeyInput, WidgetBehavior, WidgetId, WidgetKind, WidgetOutcome};
pub use focus::{FocusRing, FOCUSABLE_ATTR};
pub use host::{WidgetAction, WidgetHost};
pub use layout_query::LayoutQuery;
pub use reference::ReferenceBox;

// Re-export the generalized authoring substrate so widget authors use ONE path:
// the same Component / TemplateNode / TemplateRenderer the chrome uses.
pub use liquide_components::template::{Component, TemplateNode, TemplateRenderer};
