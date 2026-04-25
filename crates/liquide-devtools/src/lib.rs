//! LiquiDE Developer Tools
//!
//! Provides live component reload, element tree inspection, style viewing,
//! layout box visualization, and an integrated dev-tools panel for the
//! LiquiDE desktop compositor.
//!
//! # Architecture
//!
//! The dev-tools system consists of several cooperating modules:
//!
//! - **`live_reload`** — Watches template HTML and component CSS files for changes,
//!   triggering automatic re-render of affected components.
//! - **`inspector`** — Element tree browser with expand/collapse, search, and
//!   hover-to-highlight.
//! - **`style_panel`** — Computed style viewer for the selected element, showing
//!   all resolved CSS properties grouped by category.
//! - **`layout_overlay`** — Visual overlay that draws margin/padding/border/content
//!   boxes for the selected element directly on the compositor output.
//! - **`element_picker`** — Click-to-select element tool with hover highlighting.
//! - **`devtools_panel`** — The top-level dev-tools UI that composes all sub-panels
//!   into a docked or floating window.
//! - **`dom_serializer`** — Serializes the live DOM tree to JSON for external tools.
//! - **`mutation_log`** — Records DOM mutations via `MutationObserver` for debugging.

pub mod console;
pub mod context_menu;
pub mod devtools_panel;
pub mod dom_serializer;
pub mod element_picker;
pub mod inspector;
pub mod layout_overlay;
pub mod live_reload;
pub mod mutation_log;
pub mod scene_graph;
pub mod style_editor;
pub mod style_panel;

pub use console::{ConsoleAction, DebugConsole};
pub use context_menu::{ContextAction, ContextMenu};
pub use devtools_panel::{
    DevToolsConfig, DevToolsPanel, DevToolsTab, DockPosition, FrameSnapshot, SideTab,
};
pub use dom_serializer::DomSerializer;
pub use element_picker::ElementPicker;
pub use inspector::ElementTreeInspector;
pub use layout_overlay::LayoutOverlay;
pub use live_reload::{LiveReloadWatcher, ReloadEvent};
pub use mutation_log::{MutationLog, MutationRecord};
pub use scene_graph::SceneGraphDebugger;
pub use style_editor::StyleEditor;
pub use style_panel::StyleInspector;

// Re-export TemplateNode so consumers can build devtools templates without
// depending on liquide-components directly.
pub use liquide_components::TemplateNode;
