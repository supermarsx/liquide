//! Window grouping, rules, smart placement, stacking, and focus policy
//! for the LiquiDE desktop environment.
//!
//! This crate provides:
//! - **WindowGroup / GroupManager**: Grouping related windows (e.g., by app)
//! - **TabGroup / TabBarLayout / TabDragState**: Tabbed window interface
//! - **RuleEngine**: Glob-based window matching rules with configurable actions
//! - **PlacementStrategy**: Smart, cascade, center, under-mouse, first-available placement
//! - **StackingOrder**: Z-order management across Desktop/Below/Normal/Above/Notification/Overlay/Fullscreen layers
//! - **FocusPolicy / FocusGuard**: Focus stealing prevention (Strict/Moderate/Lenient)
//! - **GroupEvent**: Events emitted by group operations

pub mod focus;
pub mod group;
pub mod grouping;
pub mod manager;
pub mod placement;
pub mod policy;
pub mod rules;
pub mod stacking;
pub mod tabs;

#[cfg(test)]
mod tests;

// Re-exports for convenience.
pub use focus::{
    CurrentFocus, FocusDecision, FocusGuard, FocusPolicy, FocusReason, FocusRequest,
    should_allow_focus_steal,
};
pub use group::{GroupId, WindowGroup, WindowId};
pub use grouping::{GroupEvent, GroupEventLog};
pub use manager::GroupManager;
pub use placement::{
    PlacementConfig, PlacementStrategy, Rect, Strut, StrutEdge, cascade_place, center_place,
    first_available_place, place_window, smart_place, under_mouse_place, work_area,
};
pub use policy::{AutoGroupPolicy, GroupMinimizePolicy};
pub use rules::{
    RuleAction, RuleEngine, TilePosition, WindowInfo, WindowMatcher, WindowRule, WindowType,
    glob_match,
};
pub use stacking::{StackLayer, StackingOrder};
pub use tabs::{TabBarLayout, TabDragState, TabGroup, TabGroupId, TabRect};
