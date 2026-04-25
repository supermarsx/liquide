//! Virtual desktop / workspace management for LiquiDE.
//!
//! This crate provides [`WorkspaceManager`], the central orchestrator for
//! virtual desktops. Windows can be moved between workspaces, and workspaces
//! can be created, destroyed, reordered, and switched with animated
//! transitions.
//!
//! ## Modules
//!
//! - [`workspace`] — Core [`Workspace`] model and [`WorkspaceId`] newtype.
//! - [`manager`] — Create, destroy, switch, reorder workspaces; move windows.
//! - [`layout`] — Workspace geometry for overview grids and slide transitions.
//! - [`policy`] — Workspace, focus, and window placement policies.
//! - [`persistent`] — Session persistence and window placement rules.

pub mod layout;
pub mod manager;
pub mod persistent;
pub mod policy;
pub mod workspace;

pub use layout::{Rect, WorkspaceLayout, overview_grid, transition_offset, workspace_position};
pub use manager::{WorkspaceConfig, WorkspaceCountMode, WorkspaceEvent, WorkspaceManager};
pub use persistent::{WindowRule, WindowRuleEngine, WindowRuleResult, WorkspaceSnapshot};
pub use policy::{
    FocusPolicy, WindowPlacementPolicy, WindowRect, WorkspacePolicy, cascade_position,
    center_position, smart_placement,
};
pub use workspace::{Workspace, WorkspaceId};
