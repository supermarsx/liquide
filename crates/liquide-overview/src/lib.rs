//! Overview / expose / activities mode for LiquiDE.
//!
//! When activated, all windows scale down and arrange in a grid for quick
//! selection — similar to GNOME Shell's Activities view or macOS Mission Control.

mod animation;
pub mod expose;
mod gestures;
mod layout;
pub mod peek;
mod search;
mod state;
pub mod switcher;
pub mod taskbar_preview;
pub mod thumbnail;

pub use animation::{AnimatedSlot, OverviewAnimator, OverviewPhase, ease_out_cubic};
pub use expose::{
    ExposeConfig, ExposeKey, ExposeManager, ExposeSlot, ExposeState, ExposeWindow,
    compute_expose_layout, select_at_point,
};
pub use gestures::{Corner, HotCornerDetector, OverviewGesture};
pub use layout::{
    LayoutConfig, OverviewRect, OverviewSlot, WindowInfo, compute_overview_layout,
    compute_workspace_strip,
};
pub use peek::{PeekMode, PeekState};
pub use search::OverviewSearch;
pub use state::{OverviewAction, OverviewKey, OverviewState};
pub use switcher::{
    AppGroup, SwitcherAction, SwitcherKey, SwitcherLayout, SwitcherMode, SwitcherSlot,
    SwitcherState, WindowEntry, group_by_app, sort_mru,
};
pub use taskbar_preview::{PreviewConfig, PreviewLayout, PreviewState, TaskbarPreview};
pub use thumbnail::{
    Thumbnail, ThumbnailConfig, ThumbnailId, ThumbnailQuality, ThumbnailRegistry,
    compute_thumbnail_size, downscale_bilinear,
};
