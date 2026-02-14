//! # liquide-hit-test
//!
//! CSS-aware hit testing against the layout tree, and DOM event dispatch
//! with capture/bubble propagation, :hover/:focus state management.

pub mod dispatch;
pub mod engine;
pub mod event;

pub use dispatch::EventDispatcher;
pub use engine::{HitTestEngine, HitTestResult};
pub use event::{DomEvent, DomEventKind, Propagation};
