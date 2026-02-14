//! Native shell components with CSS+HTML templating.
//!
//! This crate provides a zero-overhead template engine and a collection of
//! native shell components (dock, status bar, launcher, notifications, menus)
//! that render directly to DOM nodes without intermediate allocations.
//!
//! ## Architecture
//!
//! - **Template Engine** — Builder-pattern `TemplateNode` + `Component` trait 
//!   + `TemplateRenderer` with keyed reconciliation
//! - **Components** — Dock, StatusBar, Launcher, Notifications, Context/Session/App menus
//!
//! ## Example
//!
//! ```ignore
//! use liquide_components::{Component, TemplateNode, TemplateRenderer};
//! use liquide_components::dock::{DockComponent, DockItem};
//!
//! let items = vec![
//!     DockItem {
//!         app_id: "files".into(),
//!         label: "Files".into(),
//!         icon: "folder".into(),
//!         is_active: true,
//!         is_running: true,
//!         is_pinned: true,
//!     },
//! ];
//!
//! let component = DockComponent::new(items);
//! let doc = liquide_dom::Document::new();
//! 
//! // Apply to DOM
//! TemplateRenderer::apply(&component, &doc);
//! ```

pub mod template;
pub mod types;
pub mod dock;
pub mod statusbar;
pub mod launcher;
pub mod notifications;
pub mod menus;

// Re-export key types
pub use template::{Component, TemplateNode, TemplateRenderer};
pub use types::*;
