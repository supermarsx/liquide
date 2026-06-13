//! # liquide-dom — Document Object Model for LiquiDE
//!
//! A lightweight DOM implementation for the LiquiDE desktop environment.
//! Every desktop element (dock, statusbar, windows, notifications) is
//! represented as a DOM node styled with CSS.
//!
//! This is **not** a web browser DOM — there is no JavaScript, no `innerHTML`,
//! no `window` object. It's a structured element tree that connects:
//!
//! 1. **CSS selectors** — type/class/id/pseudo matching
//! 2. **Layout** — the box tree is derived from this tree
//! 3. **Painting** — display list is generated per node
//! 4. **Hit testing** — input events are dispatched to nodes
//! 5. **Accessibility** — the a11y tree mirrors this tree
//!
//! ## Key differences from web DOM
//!
//! - No prototype chain or JS bindings
//! - Custom element tags are first-class (no registration ceremony)
//! - Pseudo-states are set directly (no CSS hover tracking built in)
//! - Mutation observers are synchronous Rust traits
//! - `Send` — the tree can be shared across threads with appropriate locking

pub mod attrs;
pub mod class_list;
pub mod dirty;
pub mod document;
pub mod events;
pub mod html_parser;
pub mod node;
pub mod pseudo;
pub mod tag;
pub mod template;
pub mod template_registry;
pub mod visitor;

pub use attrs::AttributeMap;
pub use class_list::ClassList;
pub use dirty::DirtyFlags;
pub use document::Document;
pub use events::{
    Event, EventListener, EventPhase, EventTargetMap, ListenerOptions, dispatch_event,
};
pub use node::{Node, NodeData, NodeId, PseudoType};
pub use pseudo::PseudoStateFlags;
pub use tag::Tag;
pub use template_registry::escape_html;
pub use visitor::{MutationObserver, Visitor};
