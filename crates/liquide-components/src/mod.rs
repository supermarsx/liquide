//! Shell component implementations using the template engine.
//!
//! Each shell subsystem (dock, status bar, launcher, notifications, menus)
//! gets a component struct that reads live state and produces a
//! [`TemplateNode`] describing the desired DOM tree.
//!
//! These components are **views** — purely functional transformations from
//! state → DOM template.  They don't own any mutable state; the shell
//! subsystem structs (`ShellDock`, `ShellStatusBar`, etc.) own the data.

pub mod dock;
pub mod launcher;
pub mod notifications;
pub mod statusbar;
pub mod menus;
