//! Overview / expose / activities mode for LiquiDE.
//!
//! When activated, all windows scale down and arrange in a grid for quick
//! selection — similar to GNOME Shell's Activities view or macOS Mission Control.

mod layout;
mod animation;
mod search;
mod state;
mod gestures;
pub mod switcher;
pub mod thumbnail;
pub mod taskbar_preview;
pub mod expose;
pub mod peek;

pub use layout::{
    compute_overview_layout, compute_workspace_strip, LayoutConfig, OverviewRect, OverviewSlot,
    WindowInfo,
};
pub use animation::{ease_out_cubic, AnimatedSlot, OverviewAnimator, OverviewPhase};
pub use search::OverviewSearch;
pub use state::{OverviewAction, OverviewKey, OverviewState};
pub use gestures::{Corner, HotCornerDetector, OverviewGesture};
pub use switcher::{
    group_by_app, sort_mru, AppGroup, SwitcherAction, SwitcherKey, SwitcherLayout, SwitcherMode,
    SwitcherSlot, SwitcherState, WindowEntry,
};
pub use thumbnail::{
    compute_thumbnail_size, downscale_bilinear, Thumbnail, ThumbnailConfig, ThumbnailId,
    ThumbnailQuality, ThumbnailRegistry,
};
pub use taskbar_preview::{
    PreviewConfig, PreviewLayout, PreviewState, TaskbarPreview,
};
pub use expose::{
    compute_expose_layout, select_at_point, ExposeConfig, ExposeKey, ExposeManager, ExposeSlot,
    ExposeState, ExposeWindow,
};
pub use peek::{PeekMode, PeekState};
